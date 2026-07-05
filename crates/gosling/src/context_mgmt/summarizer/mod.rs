//! Local-LLM summarization + fact-extraction worker (`GOSLING_SUMMARIZER`).
//!
//! Slice 1's [`super::packet::ContextManager`] collapses older conversation
//! and long tool output into a naive front-truncation stub whenever the
//! packet is over budget. This module adds a worker that runs asynchronously
//! after a packet is built, replaces that stub with a faithful digest, and
//! extracts durable facts into `memories.jsonl` — the first producer for the
//! [`super::memory::FileMemorySource`] read seam.
//!
//! Nothing here ever blocks a provider call or surfaces an error to the
//! user: `off` never runs the worker, and `shadow`/`on` swallow every
//! failure (endpoint down, timeout, malformed JSON) and fall back to the
//! deterministic truncation stub already in the packet.

pub mod schema;
pub mod worker;
pub mod writer;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::{Mutex, OnceLock, PoisonError};

use tracing::{debug, warn};

use crate::config::Config;
use crate::conversation::message::Message;
use crate::session::{
    SessionManager, SessionSummary, SessionSummaryFact, SessionSummaryStatus,
    MAX_SESSION_MESSAGE_PAGE_LIMIT,
};

pub use schema::{ExtractedFact, FactType, WorkerResponse};
pub use worker::SummarizerConfig;

use gosling_providers::base::{durable_memory_file_for, Provider};

use super::block::ContextSlot;

/// Runtime mode for the summarizer worker, controlled by `GOSLING_SUMMARIZER`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SummarizerMode {
    /// The worker never runs: no endpoint call, no cache writes, no file I/O.
    #[default]
    Off,
    /// The worker runs and its output is logged, but the packet keeps using
    /// the deterministic truncation stub and nothing is written to
    /// `memories.jsonl`.
    Shadow,
    /// The worker runs; its digest replaces the truncation stub on the next
    /// turn's packet, and extracted facts are appended to `memories.jsonl`.
    On,
}

impl SummarizerMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            SummarizerMode::Off => "off",
            SummarizerMode::Shadow => "shadow",
            SummarizerMode::On => "on",
        }
    }
}

/// Reads `GOSLING_SUMMARIZER` (env var or config), defaulting to `off`.
pub fn summarizer_mode() -> SummarizerMode {
    summarizer_mode_from(Config::global())
}

/// [`summarizer_mode`] against an explicit [`Config`]. `get_param` reads the
/// env var first, then the settings file, so an explicit
/// `GOSLING_SUMMARIZER` env var wins over the value chosen in the config
/// surface, which in turn wins over the built-in `off` default.
pub fn summarizer_mode_from(config: &Config) -> SummarizerMode {
    let raw = config
        .get_param::<String>("GOSLING_SUMMARIZER")
        .unwrap_or_else(|_| "off".to_string());

    match raw.trim().to_lowercase().as_str() {
        "shadow" => SummarizerMode::Shadow,
        "on" => SummarizerMode::On,
        _ => SummarizerMode::Off,
    }
}

const DEFAULT_MODEL: &str = "qwen2.5-coder:3b";
const DEFAULT_TIMEOUT_MS: u64 = 4_000;

impl SummarizerConfig {
    /// Resolves the summarizer's endpoint/model/timeout from config. `None`
    /// when `GOSLING_SUMMARIZER_ENDPOINT` isn't set — the worker has nowhere
    /// to call, so the caller should skip it entirely rather than fail.
    pub fn from_config() -> Option<Self> {
        Self::from_config_with(Config::global())
    }

    /// [`from_config`](Self::from_config) against an explicit [`Config`]. Each
    /// field follows the same env-over-settings-over-default precedence as
    /// [`summarizer_mode_from`].
    pub fn from_config_with(config: &Config) -> Option<Self> {
        let endpoint = config
            .get_param::<String>("GOSLING_SUMMARIZER_ENDPOINT")
            .ok()
            .filter(|s: &String| !s.is_empty())?;
        let model = config
            .get_param::<String>("GOSLING_SUMMARIZER_MODEL")
            .unwrap_or_else(|_| DEFAULT_MODEL.to_string());
        let timeout_ms = config
            .get_param::<u64>("GOSLING_SUMMARIZER_TIMEOUT_MS")
            .unwrap_or(DEFAULT_TIMEOUT_MS);
        Some(Self {
            endpoint,
            model,
            timeout_ms,
        })
    }
}

/// A block of older conversation / long tool output that the deterministic
/// packet builder rendered as a truncation-stub summary because no cached
/// digest was available for it yet. Carries what the worker needs to
/// produce a better one.
#[derive(Debug, Clone)]
pub struct PendingSummary {
    pub slot: ContextSlot,
    pub cache_key: u64,
    pub text: String,
    pub message_count: usize,
}

/// Where a run's digest and extracted facts should land, decided by whether
/// the current provider drives a self-managing backend (see
/// [`gosling_providers::base::Provider::manages_own_context`]).
#[derive(Debug, Clone)]
pub enum SummarizerTarget {
    /// Raw API provider: Gosling assembles the prompt. The digest replaces
    /// the packet's truncation stub on the next turn, and extracted facts
    /// append to `memories.jsonl`.
    ContextPacket,
    /// Self-managing CLI/agent backend (Claude Code, Codex, Gemini CLI, …):
    /// no packet takeover, so the digest is not cached. Extracted facts are
    /// routed to the backend's own durable file — the seam that survives its
    /// internal compaction — instead of `memories.jsonl` or the prompt.
    DurableFile { path: PathBuf, label: String },
}

/// A worker-produced digest cached in place of the naive truncation stub.
#[derive(Debug, Clone)]
pub struct CachedDigest {
    pub summary: String,
}

static DIGEST_CACHE: OnceLock<Mutex<HashMap<u64, CachedDigest>>> = OnceLock::new();

fn digest_cache() -> &'static Mutex<HashMap<u64, CachedDigest>> {
    DIGEST_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Content-addressed key for a block of rendered conversation text, used to
/// look up (and later store) a cached digest for it. Two calls with the same
/// rendered text — e.g. the same older messages, still unsummarized on a
/// later turn — resolve to the same key.
pub fn cache_key_for(text: &str) -> u64 {
    let hash = blake3::hash(text.as_bytes());
    u64::from_le_bytes(
        hash.as_bytes()[..8]
            .try_into()
            .expect("blake3 digest is at least 8 bytes"),
    )
}

/// Looks up a cached digest for `key`. Always misses until [`run_pending`]
/// has populated it in `on` mode — in particular, this is always empty when
/// the summarizer is `off`, which is what keeps `off` a hard no-op for the
/// deterministic packet builder in [`super::packet`].
pub fn cached_digest(key: u64) -> Option<CachedDigest> {
    digest_cache()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&key)
        .cloned()
}

fn store_digest(key: u64, digest: CachedDigest) {
    digest_cache()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(key, digest);
}

#[cfg(test)]
pub fn clear_cache_for_test() {
    digest_cache()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();
}

/// Test-only seam for [`super::packet`]'s tests to populate the digest cache
/// directly, exercising "the cache already has a digest" without spinning up
/// a mock endpoint and running the full async worker.
#[cfg(test)]
pub fn store_digest_for_test(key: u64, summary: String) {
    store_digest(key, CachedDigest { summary });
}

/// Runs the summarizer worker for each pending block and, depending on
/// `mode`, updates the digest cache and/or appends extracted facts to
/// `memories.jsonl`.
///
/// Picks the [`SummarizerTarget`] for the current provider. A self-managing
/// backend (Claude Code, Codex/ACP, Gemini CLI — anything whose
/// [`Provider::manages_own_context`] is true) routes extracted facts to its
/// durable file (`CLAUDE.md` / `AGENTS.md`) under `working_dir` and takes no
/// packet handoff; every other provider uses the packet digest cache plus
/// `memories.jsonl`.
pub fn target_for_provider(
    provider: &dyn Provider,
    working_dir: &std::path::Path,
) -> SummarizerTarget {
    if provider.manages_own_context() {
        let label = durable_memory_file_for(provider.get_name());
        SummarizerTarget::DurableFile {
            path: working_dir.join(label),
            label: label.to_string(),
        }
    } else {
        SummarizerTarget::ContextPacket
    }
}

/// Intended to be spawned (e.g. via `tokio::spawn`) right after a packet is
/// built, so it never sits on the critical path to the provider call: by the
/// time it completes, the current turn's packet has already gone out with
/// the deterministic truncation stub, and only the *next* turn benefits from
/// whatever this call caches.
///
/// `target` decides where the output lands. For a raw API provider
/// ([`SummarizerTarget::ContextPacket`]) the digest is cached for next
/// turn's packet and facts append to `memories.jsonl`. For a self-managing
/// backend ([`SummarizerTarget::DurableFile`]) the digest is *not* cached —
/// the backend owns the prompt and would re-compact any packet away — and
/// facts are routed to its durable file instead.
pub async fn run_pending(
    mode: SummarizerMode,
    session_id: &str,
    pending: Vec<PendingSummary>,
    target: SummarizerTarget,
) {
    if mode == SummarizerMode::Off || pending.is_empty() {
        return;
    }
    let Some(config) = SummarizerConfig::from_config() else {
        return;
    };

    for item in pending {
        let Some(response) = worker::summarize(&item.text, &config).await else {
            debug!(
                target: "gosling::context_mgmt::summarizer",
                slot = ?item.slot,
                "summarizer worker produced no usable digest; deterministic fallback stays in place"
            );
            continue;
        };

        match mode {
            SummarizerMode::Off => unreachable!("returned above"),
            SummarizerMode::Shadow => {
                debug!(
                    target: "gosling::context_mgmt::summarizer",
                    slot = ?item.slot,
                    summary = %response.summary,
                    fact_count = response.facts.len(),
                    target = ?target,
                    "summarizer shadow mode: would replace summary and write memories"
                );
            }
            SummarizerMode::On => match &target {
                SummarizerTarget::ContextPacket => {
                    store_digest(
                        item.cache_key,
                        CachedDigest {
                            summary: response.summary.clone(),
                        },
                    );

                    if !response.facts.is_empty() {
                        let created_at = chrono::Utc::now().to_rfc3339();
                        let records =
                            writer::records_for_facts(&response.facts, session_id, &created_at);
                        let path = super::memory::memories_file_path();
                        if let Err(e) = writer::append_memories(&path, &records) {
                            warn!(
                                "Failed to append summarizer memories to {}: {}",
                                path.display(),
                                e
                            );
                        }
                    }
                }
                SummarizerTarget::DurableFile { path, label } => {
                    // Self-managing backend: no packet takeover, so the digest
                    // is deliberately not cached. Only the write-path applies,
                    // routed to the backend's durable file.
                    if !response.facts.is_empty() {
                        let created_at = chrono::Utc::now().to_rfc3339();
                        if let Err(e) = writer::append_facts_to_durable_file(
                            path,
                            label,
                            &response.facts,
                            session_id,
                            &created_at,
                        ) {
                            warn!(
                                "Failed to append summarizer facts to {}: {}",
                                path.display(),
                                e
                            );
                        }
                    }
                }
            },
        }
    }
}

pub fn spawn_session_rollup(
    mode: SummarizerMode,
    session_manager: Arc<SessionManager>,
    session_id: String,
    tail_limit: usize,
) {
    if mode == SummarizerMode::Off {
        return;
    }

    tokio::spawn(async move {
        run_session_rollup(mode, session_manager, &session_id, tail_limit).await;
    });
}

async fn run_session_rollup(
    mode: SummarizerMode,
    session_manager: Arc<SessionManager>,
    session_id: &str,
    tail_limit: usize,
) {
    if mode == SummarizerMode::Off {
        return;
    }
    let Some(config) = SummarizerConfig::from_config() else {
        return;
    };

    let session = match session_manager.get_session(session_id, false).await {
        Ok(session) => session,
        Err(error) => {
            debug!(session_id, %error, "session summary rollup skipped: session unavailable");
            return;
        }
    };
    let tail = match session_manager
        .get_session_tail_page(session_id, tail_limit)
        .await
    {
        Ok(page) => page,
        Err(error) => {
            warn!("Failed to load session summary tail boundary: {error}");
            return;
        }
    };
    let Some(before_row_id) = tail.oldest_row_id else {
        return;
    };
    if tail.total_count <= tail.messages.len() {
        refresh_stale_summary_status(&session_manager, session_id).await;
        return;
    }

    let mut summary = match session_manager.get_session_summary(session_id).await {
        Ok(summary) => summary,
        Err(error) => {
            warn!("Failed to load existing session summary: {error}");
            None
        }
    };
    let mut after_row_id = summary
        .as_ref()
        .map(|summary| summary.covered_through_row_id)
        .unwrap_or(0);

    loop {
        let rows = match session_manager
            .get_session_message_rows_between(
                session_id,
                after_row_id,
                before_row_id,
                MAX_SESSION_MESSAGE_PAGE_LIMIT,
            )
            .await
        {
            Ok(rows) => rows,
            Err(error) => {
                warn!("Failed to load session summary chunk: {error}");
                return;
            }
        };
        if rows.is_empty() {
            refresh_stale_summary_status(&session_manager, session_id).await;
            return;
        }

        let Some((end_row_id, end_message)) = rows.last() else {
            return;
        };
        let start_row_id = rows.first().map(|(row_id, _)| *row_id);
        let source_hash = source_hash_for_summary_chunk(summary.as_ref(), &rows);
        if summary
            .as_ref()
            .is_some_and(|existing| existing.source_hash == source_hash)
        {
            after_row_id = *end_row_id;
            continue;
        }

        let prompt = rolling_summary_prompt(summary.as_ref(), &rows);
        let Some(response) = worker::summarize(&prompt, &config).await else {
            persist_failed_session_summary(
                &session_manager,
                session_id,
                summary.as_ref(),
                &config.model,
                "summarizer worker produced no usable response",
            )
            .await;
            return;
        };

        if mode == SummarizerMode::Shadow {
            debug!(
                target: "gosling::context_mgmt::summarizer",
                session_id,
                summary = %response.summary,
                fact_count = response.facts.len(),
                "session summary shadow mode: would persist rolling summary"
            );
            return;
        }

        let updated_at = chrono::Utc::now();
        let covered_message_count = summary
            .as_ref()
            .map(|summary| summary.covered_message_count)
            .unwrap_or(0)
            + rows.len();
        let next_summary = SessionSummary {
            session_id: session_id.to_string(),
            summary: response.summary.clone(),
            covered_through_row_id: *end_row_id,
            covered_through_timestamp: end_message.created,
            covered_message_count,
            source_hash,
            summarizer_model: Some(config.model.clone()),
            status: SessionSummaryStatus::Current,
            error: None,
            updated_at,
        };
        let facts = response
            .facts
            .iter()
            .map(|fact| SessionSummaryFact {
                id: 0,
                session_id: session_id.to_string(),
                project_id: session.project_id.clone(),
                working_dir: session.working_dir.to_string_lossy().to_string(),
                scope: "session".to_string(),
                fact_type: fact.fact_type.as_str().to_string(),
                content: fact.content.clone(),
                confidence: fact.confidence,
                source_start_row_id: start_row_id,
                source_end_row_id: Some(*end_row_id),
                created_at: updated_at,
            })
            .collect::<Vec<_>>();
        if let Err(error) = session_manager.upsert_session_summary(&next_summary).await {
            warn!("Failed to persist session summary: {error}");
            return;
        }
        if let Err(error) = session_manager
            .replace_session_summary_facts(session_id, &facts)
            .await
        {
            warn!("Failed to persist session summary facts: {error}");
            return;
        }

        after_row_id = *end_row_id;
        summary = Some(next_summary);
    }
}

fn rolling_summary_prompt(summary: Option<&SessionSummary>, rows: &[(i64, Message)]) -> String {
    let mut rendered = String::new();
    rendered.push_str("Merge the existing rolling session summary with the new message chunk.\n");
    rendered.push_str(
        "Return a complete replacement summary and durable facts for the merged session state.\n\n",
    );
    rendered.push_str("Existing summary:\n");
    rendered.push_str(
        summary
            .and_then(|summary| {
                let summary = summary.summary.as_str();
                (!summary.trim().is_empty()).then_some(summary)
            })
            .unwrap_or("(none)"),
    );
    rendered.push_str("\n\nNew messages:\n");
    for (row_id, message) in rows {
        let role = match message.role {
            rmcp::model::Role::User => "user",
            rmcp::model::Role::Assistant => "assistant",
        };
        rendered.push_str(&format!(
            "\n[row {row_id} {role}]\n{}\n",
            message.as_concat_text()
        ));
    }
    rendered
}

fn source_hash_for_summary_chunk(
    summary: Option<&SessionSummary>,
    rows: &[(i64, Message)],
) -> String {
    let mut hasher = blake3::Hasher::new();
    if let Some(summary) = summary {
        hasher.update(summary.source_hash.as_bytes());
        hasher.update(summary.summary.as_bytes());
        hasher.update(&summary.covered_through_row_id.to_le_bytes());
    }
    for (row_id, message) in rows {
        hasher.update(&row_id.to_le_bytes());
        hasher.update(message.as_concat_text().as_bytes());
    }
    hasher.finalize().to_hex().to_string()
}

async fn refresh_stale_summary_status(session_manager: &SessionManager, session_id: &str) {
    let Ok(Some(mut summary)) = session_manager.get_session_summary(session_id).await else {
        return;
    };
    summary.status = SessionSummaryStatus::Current;
    summary.error = None;
    summary.updated_at = chrono::Utc::now();
    if let Err(error) = session_manager.upsert_session_summary(&summary).await {
        warn!("Failed to refresh session summary status: {error}");
    }
}

async fn persist_failed_session_summary(
    session_manager: &SessionManager,
    session_id: &str,
    existing: Option<&SessionSummary>,
    model: &str,
    error: &str,
) {
    let summary = existing.cloned().unwrap_or_else(|| SessionSummary {
        session_id: session_id.to_string(),
        summary: String::new(),
        covered_through_row_id: 0,
        covered_through_timestamp: 0,
        covered_message_count: 0,
        source_hash: String::new(),
        summarizer_model: Some(model.to_string()),
        status: SessionSummaryStatus::Failed,
        error: Some(error.to_string()),
        updated_at: chrono::Utc::now(),
    });
    let mut failed = summary;
    failed.status = SessionSummaryStatus::Failed;
    failed.error = Some(error.to_string());
    failed.summarizer_model = Some(model.to_string());
    failed.updated_at = chrono::Utc::now();
    if let Err(persist_error) = session_manager.upsert_session_summary(&failed).await {
        warn!("Failed to persist failed session summary state: {persist_error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn defaults_to_off() {
        let _guard = env_lock::lock_env([("GOSLING_SUMMARIZER", None::<&str>)]);
        assert_eq!(summarizer_mode(), SummarizerMode::Off);
    }

    #[test]
    fn reads_shadow_and_on() {
        {
            let _guard = env_lock::lock_env([("GOSLING_SUMMARIZER", Some("shadow"))]);
            assert_eq!(summarizer_mode(), SummarizerMode::Shadow);
        }
        let _guard = env_lock::lock_env([("GOSLING_SUMMARIZER", Some("ON"))]);
        assert_eq!(summarizer_mode(), SummarizerMode::On);
    }

    #[test]
    fn settings_file_values_are_honored_and_env_overrides_them() {
        use crate::config::Config;
        use tempfile::NamedTempFile;

        let config_file = NamedTempFile::new().unwrap();
        let secrets_file = NamedTempFile::new().unwrap();
        let config =
            Config::new_with_file_secrets(config_file.path(), secrets_file.path()).unwrap();

        config.set_param("GOSLING_SUMMARIZER", "shadow").unwrap();
        config
            .set_param("GOSLING_SUMMARIZER_ENDPOINT", "http://localhost:11434/v1")
            .unwrap();
        config
            .set_param("GOSLING_SUMMARIZER_MODEL", "qwen2.5-coder:3b")
            .unwrap();
        config
            .set_param("GOSLING_SUMMARIZER_TIMEOUT_MS", 9000u64)
            .unwrap();

        // Values chosen in the config surface (as `gosling configure` or the
        // desktop settings UI would write them) are honored when no env var
        // is set.
        {
            let _guard = env_lock::lock_env([
                ("GOSLING_SUMMARIZER", None::<&str>),
                ("GOSLING_SUMMARIZER_ENDPOINT", None),
                ("GOSLING_SUMMARIZER_MODEL", None),
                ("GOSLING_SUMMARIZER_TIMEOUT_MS", None),
            ]);
            assert_eq!(summarizer_mode_from(&config), SummarizerMode::Shadow);
            let resolved = SummarizerConfig::from_config_with(&config)
                .expect("endpoint set in the config surface should resolve a config");
            assert_eq!(resolved.endpoint, "http://localhost:11434/v1");
            assert_eq!(resolved.model, "qwen2.5-coder:3b");
            assert_eq!(resolved.timeout_ms, 9000);
        }

        // An explicit env var wins over the settings value; fields without an
        // env override still come from the settings file.
        {
            let _override = env_lock::lock_env([
                ("GOSLING_SUMMARIZER", Some("on")),
                ("GOSLING_SUMMARIZER_ENDPOINT", None),
                ("GOSLING_SUMMARIZER_MODEL", Some("llama3.2:1b")),
                ("GOSLING_SUMMARIZER_TIMEOUT_MS", None),
            ]);
            assert_eq!(summarizer_mode_from(&config), SummarizerMode::On);
            let overridden = SummarizerConfig::from_config_with(&config).unwrap();
            assert_eq!(overridden.model, "llama3.2:1b");
            assert_eq!(overridden.endpoint, "http://localhost:11434/v1");
        }
    }

    #[test]
    fn cache_key_is_stable_and_content_addressed() {
        let a = cache_key_for("older turn one\nolder reply one");
        let b = cache_key_for("older turn one\nolder reply one");
        let c = cache_key_for("a completely different block of text");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    fn chat_completion(content: &str) -> serde_json::Value {
        serde_json::json!({
            "choices": [{
                "message": { "role": "assistant", "content": content },
            }],
        })
    }

    async fn mock_server_replying(content: &str) -> MockServer {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(chat_completion(content)))
            .mount(&server)
            .await;
        server
    }

    #[tokio::test]
    async fn off_mode_never_calls_the_endpoint_or_touches_the_cache() {
        let dir = tempfile::tempdir().unwrap();
        let memory_path = dir.path().join("memories.jsonl");

        // A live, working endpoint is configured — proving `off` doesn't
        // call it (not just that there was nowhere to call). `.expect(0)`
        // makes the server itself assert zero requests on drop.
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(chat_completion(
                r#"{"summary": "should never be used", "facts": [{"content": "should never be written", "type": "fact", "confidence": 0.9}]}"#,
            )))
            .expect(0)
            .mount(&server)
            .await;

        let _guard = env_lock::lock_env([
            ("GOSLING_SUMMARIZER_ENDPOINT", Some(server.uri().as_str())),
            ("GOSLING_SUMMARIZER_MODEL", None),
            ("GOSLING_SUMMARIZER_TIMEOUT_MS", None),
            (
                "GOSLING_MEMORY_FILE",
                Some(memory_path.to_string_lossy().as_ref()),
            ),
        ]);

        let key = cache_key_for("off_mode_never_calls_the_endpoint_or_touches_the_cache");
        let pending = vec![PendingSummary {
            slot: ContextSlot::OlderConversationSummary,
            cache_key: key,
            text: "some older conversation".to_string(),
            message_count: 3,
        }];

        run_pending(
            SummarizerMode::Off,
            "session-off",
            pending,
            SummarizerTarget::ContextPacket,
        )
        .await;

        assert!(cached_digest(key).is_none());
        assert!(
            !memory_path.exists(),
            "off mode must not write to memories.jsonl"
        );
        // `server`'s Drop impl verifies the `.expect(0)` mock above received
        // no requests, i.e. `off` mode never called out even though a
        // working endpoint was configured.
    }

    #[tokio::test]
    async fn shadow_mode_does_not_populate_the_cache_or_write_memories() {
        let dir = tempfile::tempdir().unwrap();
        let memory_path = dir.path().join("memories.jsonl");
        let server = mock_server_replying(
            r#"{"summary": "shadow digest", "facts": [{"content": "shadow fact", "type": "fact", "confidence": 0.5}]}"#,
        )
        .await;

        let _guard = env_lock::lock_env([
            ("GOSLING_SUMMARIZER_ENDPOINT", Some(server.uri().as_str())),
            ("GOSLING_SUMMARIZER_MODEL", None),
            ("GOSLING_SUMMARIZER_TIMEOUT_MS", None),
            (
                "GOSLING_MEMORY_FILE",
                Some(memory_path.to_string_lossy().as_ref()),
            ),
        ]);

        let key = cache_key_for("shadow_mode_does_not_populate_the_cache_or_write_memories");
        let pending = vec![PendingSummary {
            slot: ContextSlot::OlderConversationSummary,
            cache_key: key,
            text: "some older conversation about shadow mode".to_string(),
            message_count: 4,
        }];

        run_pending(
            SummarizerMode::Shadow,
            "session-shadow",
            pending,
            SummarizerTarget::ContextPacket,
        )
        .await;

        assert!(
            cached_digest(key).is_none(),
            "shadow mode must not replace the packet's summary text"
        );
        assert!(
            !memory_path.exists(),
            "shadow mode must not write to memories.jsonl"
        );
    }

    #[tokio::test]
    async fn on_mode_populates_the_cache_and_writes_memories() {
        let dir = tempfile::tempdir().unwrap();
        let memory_path = dir.path().join("memories.jsonl");
        let server = mock_server_replying(
            r#"{"summary": "on-mode digest preserving gosling.toml", "facts": [{"content": "config lives in gosling.toml", "type": "fact", "confidence": 0.9}]}"#,
        )
        .await;

        let _guard = env_lock::lock_env([
            ("GOSLING_SUMMARIZER_ENDPOINT", Some(server.uri().as_str())),
            ("GOSLING_SUMMARIZER_MODEL", None),
            ("GOSLING_SUMMARIZER_TIMEOUT_MS", None),
            (
                "GOSLING_MEMORY_FILE",
                Some(memory_path.to_string_lossy().as_ref()),
            ),
        ]);

        let key = cache_key_for("on_mode_populates_the_cache_and_writes_memories");
        let pending = vec![PendingSummary {
            slot: ContextSlot::OlderConversationSummary,
            cache_key: key,
            text: "some older conversation about gosling.toml".to_string(),
            message_count: 5,
        }];

        run_pending(
            SummarizerMode::On,
            "session-on",
            pending,
            SummarizerTarget::ContextPacket,
        )
        .await;

        let digest = cached_digest(key).expect("on mode should cache the worker's digest");
        assert_eq!(digest.summary, "on-mode digest preserving gosling.toml");

        let contents = std::fs::read_to_string(&memory_path).unwrap();
        assert!(contents.contains("config lives in gosling.toml"));
        assert!(contents.contains("session-on"));
    }

    #[tokio::test]
    async fn on_mode_self_managing_backend_routes_facts_to_durable_file_and_skips_cache() {
        clear_cache_for_test();
        let dir = tempfile::tempdir().unwrap();
        let memory_path = dir.path().join("memories.jsonl");
        let durable_path = dir.path().join("AGENTS.md");
        let server = mock_server_replying(
            r#"{"summary": "on-mode digest", "facts": [{"content": "prefers AGENTS.md convention", "type": "preference", "confidence": 0.9}]}"#,
        )
        .await;

        let _guard = env_lock::lock_env([
            ("GOSLING_SUMMARIZER_ENDPOINT", Some(server.uri().as_str())),
            ("GOSLING_SUMMARIZER_MODEL", None),
            ("GOSLING_SUMMARIZER_TIMEOUT_MS", None),
            (
                "GOSLING_MEMORY_FILE",
                Some(memory_path.to_string_lossy().as_ref()),
            ),
        ]);

        let key = cache_key_for(
            "on_mode_self_managing_backend_routes_facts_to_durable_file_and_skips_cache",
        );
        let pending = vec![PendingSummary {
            slot: ContextSlot::OlderConversationSummary,
            cache_key: key,
            text: "some older conversation on a self-managing backend".to_string(),
            message_count: 5,
        }];

        run_pending(
            SummarizerMode::On,
            "session-selfmanaging",
            pending,
            SummarizerTarget::DurableFile {
                path: durable_path.clone(),
                label: "AGENTS.md".to_string(),
            },
        )
        .await;

        assert!(
            cached_digest(key).is_none(),
            "a self-managing backend takes no packet digest handoff"
        );
        assert!(
            !memory_path.exists(),
            "facts route to the durable file, not memories.jsonl"
        );
        let durable = std::fs::read_to_string(&durable_path).unwrap();
        assert!(durable.contains("## Gosling extracted memory"));
        assert!(durable.contains("prefers AGENTS.md convention"));
        assert!(durable.contains("session-selfmanaging"));
    }

    #[tokio::test]
    async fn empty_facts_is_summary_only_and_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let memory_path = dir.path().join("memories.jsonl");
        let server =
            mock_server_replying(r#"{"summary": "nothing durable here", "facts": []}"#).await;

        let _guard = env_lock::lock_env([
            ("GOSLING_SUMMARIZER_ENDPOINT", Some(server.uri().as_str())),
            ("GOSLING_SUMMARIZER_MODEL", None),
            ("GOSLING_SUMMARIZER_TIMEOUT_MS", None),
            (
                "GOSLING_MEMORY_FILE",
                Some(memory_path.to_string_lossy().as_ref()),
            ),
        ]);

        let key = cache_key_for("empty_facts_is_summary_only_and_writes_nothing");
        let pending = vec![PendingSummary {
            slot: ContextSlot::OlderConversationSummary,
            cache_key: key,
            text: "some older conversation with nothing durable".to_string(),
            message_count: 2,
        }];

        run_pending(
            SummarizerMode::On,
            "session-empty-facts",
            pending,
            SummarizerTarget::ContextPacket,
        )
        .await;

        let digest = cached_digest(key).expect("summary should still be cached");
        assert_eq!(digest.summary, "nothing durable here");
        assert!(
            !memory_path.exists(),
            "no facts extracted should mean no file write at all"
        );
    }

    #[tokio::test]
    async fn malformed_endpoint_response_falls_back_without_panicking() {
        let server = mock_server_replying("not json at all").await;

        let _guard = env_lock::lock_env([
            ("GOSLING_SUMMARIZER_ENDPOINT", Some(server.uri().as_str())),
            ("GOSLING_SUMMARIZER_MODEL", None),
            ("GOSLING_SUMMARIZER_TIMEOUT_MS", None),
            ("GOSLING_MEMORY_FILE", None),
        ]);

        let key = cache_key_for("malformed_endpoint_response_falls_back_without_panicking");
        let pending = vec![PendingSummary {
            slot: ContextSlot::OlderConversationSummary,
            cache_key: key,
            text: "some older conversation".to_string(),
            message_count: 3,
        }];

        run_pending(
            SummarizerMode::On,
            "session-malformed",
            pending,
            SummarizerTarget::ContextPacket,
        )
        .await;

        assert!(
            cached_digest(key).is_none(),
            "malformed JSON must not be cached; the truncation stub stays the fallback"
        );
    }

    #[tokio::test]
    async fn timeout_falls_back_without_panicking() {
        use std::time::Duration;

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(chat_completion(r#"{"summary": "too slow", "facts": []}"#))
                    .set_delay(Duration::from_millis(500)),
            )
            .mount(&server)
            .await;

        let _guard = env_lock::lock_env([
            ("GOSLING_SUMMARIZER_ENDPOINT", Some(server.uri().as_str())),
            ("GOSLING_SUMMARIZER_MODEL", None),
            ("GOSLING_SUMMARIZER_TIMEOUT_MS", Some("50")),
            ("GOSLING_MEMORY_FILE", None),
        ]);

        let key = cache_key_for("timeout_falls_back_without_panicking");
        let pending = vec![PendingSummary {
            slot: ContextSlot::OlderConversationSummary,
            cache_key: key,
            text: "some older conversation".to_string(),
            message_count: 3,
        }];

        run_pending(
            SummarizerMode::On,
            "session-timeout",
            pending,
            SummarizerTarget::ContextPacket,
        )
        .await;

        assert!(cached_digest(key).is_none());
    }
}
