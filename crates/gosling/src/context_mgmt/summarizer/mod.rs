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
use std::sync::{Mutex, OnceLock, PoisonError};

use tracing::{debug, warn};

use crate::config::Config;

pub use schema::{ExtractedFact, FactType, WorkerResponse};
pub use worker::SummarizerConfig;

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
    let raw = Config::global()
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
        let config = Config::global();
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
/// Intended to be spawned (e.g. via `tokio::spawn`) right after a packet is
/// built, so it never sits on the critical path to the provider call: by the
/// time it completes, the current turn's packet has already gone out with
/// the deterministic truncation stub, and only the *next* turn benefits from
/// whatever this call caches.
pub async fn run_pending(mode: SummarizerMode, session_id: &str, pending: Vec<PendingSummary>) {
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
                    "summarizer shadow mode: would replace summary and write memories"
                );
            }
            SummarizerMode::On => {
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
        }
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

        run_pending(SummarizerMode::Off, "session-off", pending).await;

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

        run_pending(SummarizerMode::Shadow, "session-shadow", pending).await;

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

        run_pending(SummarizerMode::On, "session-on", pending).await;

        let digest = cached_digest(key).expect("on mode should cache the worker's digest");
        assert_eq!(digest.summary, "on-mode digest preserving gosling.toml");

        let contents = std::fs::read_to_string(&memory_path).unwrap();
        assert!(contents.contains("config lives in gosling.toml"));
        assert!(contents.contains("session-on"));
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

        run_pending(SummarizerMode::On, "session-empty-facts", pending).await;

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

        run_pending(SummarizerMode::On, "session-malformed", pending).await;

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

        run_pending(SummarizerMode::On, "session-timeout", pending).await;

        assert!(cached_digest(key).is_none());
    }
}
