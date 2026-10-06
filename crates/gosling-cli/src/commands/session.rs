use crate::session::message_to_markdown;
use anyhow::{Context, Result};

use cliclack::{confirm, multiselect, select};
use etcetera::home_dir;
use gosling::acp::custom_requests::{
    CompactionEffectDto, CompactionHistoryPurgeMode, CompactionRevisionDto, CompactionTriggerDto,
    DeleteCompactionRevisionRequest, GetCompactionRevisionRequest, ListCompactionRevisionsRequest,
    PurgeCompactionHistoryRequest, SetCompactionRevisionPinnedRequest,
};
#[cfg(feature = "nostr")]
use gosling::config::Config;
#[cfg(feature = "nostr")]
use gosling::session::nostr_share;
use gosling::session::{
    generate_diagnostics, installation_secret_redactor, redact_session_export, DiagnosticsLevel,
    Session, SessionManager, SessionType,
};
use gosling::utils::safe_truncate;
use gosling::workspace::WorkspaceNames;
use gosling_providers::secret_redaction::SecretRedactor;
use regex::Regex;
use std::borrow::Cow;
use std::fmt::Write as FmtWrite;
use std::fs;
use std::io::{self, IsTerminal, Write};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;

const TRUNCATED_DESC_LENGTH: usize = 60;

/// Session names can come from untrusted transcripts (foreign importers derive them from the
/// first user message), so control characters are shown escaped instead of reaching the terminal
/// where ESC/BEL sequences could retitle, recolor or clear it.
fn terminal_safe(text: &str) -> Cow<'_, str> {
    if !text.chars().any(char::is_control) {
        return Cow::Borrowed(text);
    }
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_control() {
            escaped.extend(c.escape_debug());
        } else {
            escaped.push(c);
        }
    }
    Cow::Owned(escaped)
}

fn display_path_with_tilde(path: &Path) -> String {
    #[cfg(not(target_os = "windows"))]
    if let Ok(home) = home_dir() {
        if let Ok(stripped) = path.strip_prefix(&home) {
            return format!("~/{}", stripped.display());
        }
    }
    path.display().to_string()
}

async fn remove_sessions(
    session_manager: &SessionManager,
    sessions: Vec<Session>,
    skip_confirmation: bool,
) -> Result<()> {
    if !skip_confirmation && !io::stdin().is_terminal() {
        anyhow::bail!(
            "Session removal requires an interactive terminal. Re-run with --yes to remove the matched sessions."
        );
    }

    println!("The following sessions will be removed:");
    for session in &sessions {
        println!("- {} {}", session.id, terminal_safe(&session.name));
    }

    let should_delete = skip_confirmation
        || confirm("Are you sure you want to delete these sessions?")
            .initial_value(false)
            .interact()?;

    if should_delete {
        for session in sessions {
            session_manager.delete_session(&session.id).await?;
            println!("Session `{}` removed.", session.id);
        }
    } else {
        println!("Skipping deletion of the sessions.");
    }

    Ok(())
}

fn prompt_interactive_session_removal(sessions: &[Session]) -> Result<Vec<Session>> {
    if sessions.is_empty() {
        println!("No sessions to delete.");
        return Ok(vec![]);
    }

    let mut selector = multiselect(
        "Select sessions to delete (use spacebar, Enter to confirm, Ctrl+C to cancel):",
    );

    for (id, label) in session_picker_items(sessions) {
        selector = selector.item(id, label, "");
    }

    let selected_ids: Vec<String> = selector.interact()?;

    let selected_sessions: Vec<Session> = sessions
        .iter()
        .filter(|s| selected_ids.contains(&s.id))
        .cloned()
        .collect();

    Ok(selected_sessions)
}

pub async fn handle_session_remove(
    session_id: Option<String>,
    name: Option<String>,
    regex_string: Option<String>,
    skip_confirmation: bool,
) -> Result<()> {
    let session_manager = SessionManager::instance();

    let matched_sessions: Vec<Session>;

    if let Some(id_val) = session_id {
        match session_manager.get_session(&id_val, false).await {
            Ok(session) => matched_sessions = vec![session],
            Err(_) => return Err(anyhow::anyhow!("Session ID '{}' not found.", id_val)),
        }
    } else if let Some(name_val) = name {
        let all_sessions = session_manager.list_all_sessions().await?;
        if let Some(session) = all_sessions.into_iter().find(|s| s.name == name_val) {
            matched_sessions = vec![session];
        } else {
            return Err(anyhow::anyhow!(
                "Session with name '{}' not found.",
                name_val
            ));
        }
    } else if let Some(regex_val) = regex_string {
        let session_regex = Regex::new(&regex_val)
            .with_context(|| format!("Invalid regex pattern '{}'", regex_val))?;

        let visible_sessions = session_manager.list_sessions().await?;
        matched_sessions = visible_sessions
            .into_iter()
            .filter(|session| session_regex.is_match(&session.id))
            .collect();

        if matched_sessions.is_empty() {
            println!(
                "Regex string '{}' does not match any session IDs (--regex matches IDs, not names)",
                regex_val
            );
            return Ok(());
        }
    } else {
        let visible_sessions = session_manager.list_sessions().await?;
        if visible_sessions.is_empty() {
            return Err(anyhow::anyhow!("No sessions found."));
        }
        ensure_session_picker_terminal("--session-id <ID>, --name <NAME> or --regex <PATTERN>")?;
        matched_sessions = prompt_interactive_session_removal(&visible_sessions)?;
    }

    if matched_sessions.is_empty() {
        return Ok(());
    }

    remove_sessions(&session_manager, matched_sessions, skip_confirmation).await
}

fn write_line_or_broken_pipe_ok<W: Write>(out: &mut W, line: &str) -> Result<bool> {
    match writeln!(out, "{line}") {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(false),
        Err(e) => Err(e.into()),
    }
}

fn session_activity_at(session: &Session) -> chrono::DateTime<chrono::Utc> {
    session.last_message_at.unwrap_or(session.updated_at)
}

fn sort_most_recent_first(sessions: &mut [Session]) {
    sessions.sort_by_key(|s| std::cmp::Reverse(session_activity_at(s)));
}

/// `(session id, label)` pairs for the session pickers, in `session list` order.
fn session_picker_items(sessions: &[Session]) -> Vec<(String, String)> {
    let mut sessions = sessions.to_vec();
    sort_most_recent_first(&mut sessions);
    sessions
        .iter()
        .map(|s| {
            let desc = if s.name.is_empty() {
                Cow::Borrowed("(no name)")
            } else {
                terminal_safe(&s.name)
            };
            let truncated_desc = safe_truncate(&desc, TRUNCATED_DESC_LENGTH);
            let label = format!("{} - {} ({})", session_activity_at(s), truncated_desc, s.id);
            (s.id.clone(), label)
        })
        .collect()
}

/// `session list -w` matches the directory itself and anything inside it, by
/// whole path components: a substring match leaked sibling directories such
/// as `proj-b` for `proj`. The canonical form is also tried so `/tmp/x`
/// still matches sessions recorded under `/private/tmp/x`.
fn working_dir_filter_roots(dir: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(absolute) = std::path::absolute(dir) {
        roots.push(absolute);
    }
    if let Ok(canonical) = dir.canonicalize() {
        if !roots.contains(&canonical) {
            roots.push(canonical);
        }
    }
    roots
}

pub async fn handle_session_list(
    format: String,
    ascending: bool,
    working_dir: Option<PathBuf>,
    limit: Option<usize>,
) -> Result<()> {
    let session_manager = SessionManager::instance();
    let mut sessions = session_manager.list_sessions_with_messages().await?;

    if let Some(ref dir) = working_dir {
        let roots = working_dir_filter_roots(dir);
        sessions.retain(|s| roots.iter().any(|root| s.working_dir.starts_with(root)));
    }

    if ascending {
        sessions.sort_by_key(session_activity_at);
    } else {
        sort_most_recent_first(&mut sessions);
    }

    if let Some(n) = limit {
        sessions.truncate(n);
    }

    let stdout = io::stdout();
    let mut out = stdout.lock();

    match format.as_str() {
        "json" => {
            // An unreadable workspace store must not hide sessions, so their
            // stored names are shown instead.
            let workspace_names =
                WorkspaceNames::load(&session_manager.data_dir()).unwrap_or_default();
            for session in &mut sessions {
                workspace_names.apply(session);
            }
            let payload = serde_json::to_string(&sessions)?;
            if !write_line_or_broken_pipe_ok(&mut out, &payload)? {
                return Ok(());
            }
        }
        _ => {
            if sessions.is_empty() {
                if !write_line_or_broken_pipe_ok(&mut out, "No sessions found")? {
                    return Ok(());
                }
                return Ok(());
            }

            if !write_line_or_broken_pipe_ok(&mut out, "Available sessions:")? {
                return Ok(());
            }

            for session in sessions {
                let output = format!(
                    "{} - {} - {} - {}",
                    session.id,
                    terminal_safe(&session.name),
                    session_activity_at(&session),
                    display_path_with_tilde(&session.working_dir)
                );
                if !write_line_or_broken_pipe_ok(&mut out, &output)? {
                    return Ok(());
                }
            }
        }
    }
    Ok(())
}

pub async fn handle_session_export(
    session_id: String,
    output_path: Option<PathBuf>,
    format: String,
    nostr: bool,
    #[cfg_attr(not(feature = "nostr"), allow(unused_variables))] relays: Vec<String>,
    redact: bool,
) -> Result<()> {
    let session_manager = SessionManager::instance();
    if format == "json-pages" {
        if nostr || !relays.is_empty() {
            anyhow::bail!("Paged acquisition export does not support Nostr sharing");
        }
        let directory =
            output_path.ok_or_else(|| anyhow::anyhow!("json-pages requires --output directory"))?;
        let snapshot = session_manager
            .export_session_snapshot_for_bundle(&session_id)
            .await?;
        let snapshot = if redact {
            let redactor = installation_secret_redactor().await;
            serde_json::to_string_pretty(&redact_session_export(
                serde_json::from_str(&snapshot)?,
                &redactor,
            ))?
        } else {
            snapshot
        };
        let bundle = gosling::session::export_bundle::write_bundle(
            &directory,
            &session_id,
            &snapshot,
            redact,
        )?;
        println!("{}", serde_json::to_string(&bundle)?);
        return Ok(());
    }
    let session = match session_manager.get_session(&session_id, true).await {
        Ok(session) => session,
        Err(e) => {
            return Err(anyhow::anyhow!(
                "Session '{}' not found or failed to read: {}",
                session_id,
                e
            ));
        }
    };

    let redactor = if redact {
        Some(installation_secret_redactor().await)
    } else {
        None
    };
    let output =
        serialize_session_export(&session_manager, &session, &format, redactor.as_ref()).await?;

    #[cfg(feature = "nostr")]
    if nostr {
        if format != "json" {
            return Err(anyhow::anyhow!(
                "Nostr session sharing only supports --format json"
            ));
        }
        if output_path.is_some() {
            return Err(anyhow::anyhow!(
                "Nostr session sharing cannot be combined with --output"
            ));
        }

        let relays = nostr_share::resolve_relays(relays, Config::global());
        let share = nostr_share::publish_session_json(&output, relays).await?;
        println!("Session published to Nostr relays:");
        for relay in &share.relays {
            println!("- {}", relay);
        }
        println!("\nShare link:");
        println!("{}", share.deeplink);
        return Ok(());
    }
    #[cfg(not(feature = "nostr"))]
    if nostr {
        return Err(anyhow::anyhow!("gosling was not built with nostr support"));
    }

    if let Some(output_path) = output_path {
        // An export is the full conversation plus workspace and credential
        // identifiers. It was written with `fs::write`, i.e. world-readable
        // 0644, while the diagnostics bundle beside it already used 0o600 and
        // warned about its contents. Match that. (IOP-GOS-003)
        write_owner_only_output_file(&output_path, output.as_bytes()).with_context(|| {
            format!("Failed to write to output file: {}", output_path.display())
        })?;
        println!("Session exported to {}", output_path.display());
        if redact {
            println!(
                "Stored secrets and credential-shaped text were replaced with [REDACTED]. \
                 Review before sharing."
            );
        } else {
            println!(
                "This file contains the full conversation and may include secrets \
                 pasted into the session. Review before sharing."
            );
        }
    } else {
        println!("{}", output);
    }

    Ok(())
}

pub async fn handle_context_history_list(
    session_id: String,
    limit: usize,
    before_generation: Option<u64>,
    include_expired: bool,
    format: String,
) -> Result<()> {
    let response = SessionManager::instance()
        .list_compaction_history(ListCompactionRevisionsRequest {
            session_id,
            before_generation,
            limit: Some(limit),
            include_expired,
        })
        .await?;
    match format.as_str() {
        "json" => println!("{}", serde_json::to_string_pretty(&response)?),
        "text" => {
            if response.revisions.is_empty() {
                println!("No context history snapshots found.");
            } else {
                for revision in response.revisions {
                    let state = if revision.pinned_at.is_some() {
                        "pinned"
                    } else if revision.expired {
                        "expired"
                    } else {
                        "retained"
                    };
                    println!(
                        "#{}  {}  {}  {} → {} tokens  {}",
                        revision.generation,
                        revision.created_at,
                        state,
                        revision.estimated_tokens_before,
                        revision.estimated_tokens_after,
                        revision.resolved_model
                    );
                }
                if let Some(next) = response.next_before_generation {
                    println!("More history is available; continue with --before {next}.");
                }
            }
            if response.purged_count > 0 {
                println!(
                    "{} snapshot(s) were removed by retention or an explicit action.",
                    response.purged_count
                );
            }
        }
        _ => anyhow::bail!("Unsupported format: {format}"),
    }
    Ok(())
}

pub async fn handle_context_history_show(
    session_id: String,
    generation: u64,
    format: String,
) -> Result<()> {
    let revision = SessionManager::instance()
        .get_compaction_history_revision(GetCompactionRevisionRequest {
            session_id,
            generation,
        })
        .await?
        .revision;
    match format.as_str() {
        "json" => println!("{}", serde_json::to_string_pretty(&revision)?),
        "markdown" => print!("{}", render_context_history_markdown(&[revision], 0)),
        _ => anyhow::bail!("Unsupported format: {format}"),
    }
    Ok(())
}

pub async fn handle_context_history_export(
    session_id: String,
    generation: Option<u64>,
    output_path: Option<PathBuf>,
    format: String,
    acknowledged: bool,
) -> Result<()> {
    if !confirm_context_history_action(
        acknowledged,
        "Context History contains model-generated summaries that may include sensitive session details. Export it?",
    )? {
        println!("Export cancelled.");
        return Ok(());
    }
    let manager = SessionManager::instance();
    let (revisions, purged_count) = if let Some(generation) = generation {
        (
            vec![
                manager
                    .get_compaction_history_revision(GetCompactionRevisionRequest {
                        session_id: session_id.clone(),
                        generation,
                    })
                    .await?
                    .revision,
            ],
            manager
                .compaction_history_stats(Some(&session_id))
                .await?
                .purged_count,
        )
    } else {
        load_all_context_history(&manager, &session_id).await?
    };
    let output = match format.as_str() {
        "json" => serde_json::to_string_pretty(&serde_json::json!({
            "schemaVersion": 1,
            "warning": "This export contains sensitive model-generated context summaries.",
            "sessionId": session_id,
            "exportedAt": chrono::Utc::now().to_rfc3339(),
            "purgedCount": purged_count,
            "revisions": revisions,
        }))?,
        "markdown" => render_context_history_markdown(&revisions, purged_count),
        _ => anyhow::bail!("Unsupported format: {format}"),
    };
    if let Some(path) = output_path {
        write_owner_only_output_file(&path, output.as_bytes()).with_context(|| {
            format!("Failed to write Context History export: {}", path.display())
        })?;
        println!("Context History exported to {}", path.display());
        println!("The file is owner-readable only. Review it before sharing.");
    } else {
        println!("{output}");
    }
    Ok(())
}

pub async fn handle_context_history_pin(
    session_id: String,
    generation: u64,
    pinned: bool,
) -> Result<()> {
    SessionManager::instance()
        .set_compaction_history_pinned(SetCompactionRevisionPinnedRequest {
            session_id,
            generation,
            pinned,
        })
        .await?;
    println!(
        "Context History snapshot #{generation} {}.",
        if pinned { "pinned" } else { "unpinned" }
    );
    Ok(())
}

pub async fn handle_context_history_delete(
    session_id: String,
    generation: u64,
    confirmed: bool,
) -> Result<()> {
    if !confirm_context_history_action(
        confirmed,
        &format!("Permanently delete Context History snapshot #{generation}?"),
    )? {
        println!("Deletion cancelled.");
        return Ok(());
    }
    SessionManager::instance()
        .delete_compaction_history_revision(DeleteCompactionRevisionRequest {
            session_id,
            generation,
        })
        .await?;
    println!("Context History snapshot #{generation} deleted.");
    Ok(())
}

pub async fn handle_context_history_prune(
    session_id: String,
    all_unpinned: bool,
    confirmed: bool,
) -> Result<()> {
    let description = if all_unpinned {
        "Delete every unpinned Context History snapshot for this session?"
    } else {
        "Delete expired Context History snapshots for this session now?"
    };
    if !confirm_context_history_action(confirmed, description)? {
        println!("Prune cancelled.");
        return Ok(());
    }
    let result = SessionManager::instance()
        .purge_compaction_history(PurgeCompactionHistoryRequest {
            session_id,
            mode: if all_unpinned {
                CompactionHistoryPurgeMode::AllUnpinned
            } else {
                CompactionHistoryPurgeMode::Expired
            },
        })
        .await?;
    println!(
        "Deleted {} snapshot(s), reclaiming {} logical byte(s). {} snapshot(s) remain.",
        result.deleted_count, result.deleted_bytes, result.remaining.revision_count
    );
    Ok(())
}

async fn load_all_context_history(
    manager: &SessionManager,
    session_id: &str,
) -> Result<(Vec<CompactionRevisionDto>, u64)> {
    let mut revisions = Vec::new();
    let mut before_generation = None;
    loop {
        let page = manager
            .list_compaction_history(ListCompactionRevisionsRequest {
                session_id: session_id.to_string(),
                before_generation,
                limit: Some(200),
                include_expired: true,
            })
            .await?;
        let purged_count = page.purged_count;
        for item in page.revisions {
            revisions.push(
                manager
                    .get_compaction_history_revision(GetCompactionRevisionRequest {
                        session_id: session_id.to_string(),
                        generation: item.generation,
                    })
                    .await?
                    .revision,
            );
        }
        let Some(next) = page.next_before_generation else {
            return Ok((revisions, purged_count));
        };
        before_generation = Some(next);
    }
}

fn confirm_context_history_action(confirmed: bool, prompt: &str) -> Result<bool> {
    if confirmed {
        return Ok(true);
    }
    if !io::stdin().is_terminal() {
        anyhow::bail!(
            "This action requires an interactive terminal. Re-run with --yes to confirm."
        );
    }
    Ok(confirm(prompt).initial_value(false).interact()?)
}

fn render_context_history_markdown(
    revisions: &[CompactionRevisionDto],
    purged_count: u64,
) -> String {
    let mut output = String::from(
        "# Context History\n\n> This export contains sensitive model-generated context summaries. Review it before sharing.\n\n",
    );
    if purged_count > 0 {
        let _ = writeln!(
            output,
            "{purged_count} snapshot(s) were removed; generation gaps are intentional.\n"
        );
    }
    for revision in revisions {
        let trigger = match revision.trigger {
            CompactionTriggerDto::Manual => "manual",
            CompactionTriggerDto::AutomaticThreshold => "automatic threshold",
            CompactionTriggerDto::OverflowRecovery => "overflow recovery",
        };
        let effect = match revision.effect {
            CompactionEffectDto::Durable => "durable",
            CompactionEffectDto::Temporary => "temporary",
        };
        let retention = if revision.pinned_at.is_some() {
            "pinned".to_string()
        } else if revision.expired {
            "expired".to_string()
        } else {
            revision.expires_at.as_ref().map_or_else(
                || "no time expiry".to_string(),
                |date| format!("expires {date}"),
            )
        };
        let _ = writeln!(
            output,
            "## Snapshot #{}\n\n- Created: {}\n- Trigger: {}\n- Effect: {}\n- Model: {}\n- Conversation estimate: {} → {} tokens (system prompt and tools not included)\n- Source messages: {}\n- Retention: {}\n- Summary hash: `{}`\n\n{}\n",
            revision.generation,
            revision.created_at,
            trigger,
            effect,
            revision.resolved_model,
            revision.estimated_tokens_before,
            revision.estimated_tokens_after,
            revision.source_message_count,
            retention,
            revision.summary_hash,
            revision.summary
        );
    }
    output
}

/// With a redactor, the export is prepared the way diagnostics bundles are:
/// secrets are replaced with `[REDACTED]` (GSL-PT-20260927-D06, C15).
async fn serialize_session_export(
    session_manager: &SessionManager,
    session: &Session,
    format: &str,
    redactor: Option<&SecretRedactor>,
) -> Result<String> {
    match format {
        // Native JSON is a transactional core export, not merely Session
        // serialization: it also carries plan_history_v1 and future native
        // adjuncts that must round-trip with the transcript.
        "json" => {
            let exported = session_manager.export_session(&session.id).await?;
            match redactor {
                Some(redactor) => Ok(serde_json::to_string_pretty(&redact_session_export(
                    serde_json::from_str(&exported)?,
                    redactor,
                ))?),
                None => Ok(exported),
            }
        }
        "yaml" => match redactor {
            Some(redactor) => Ok(serde_yaml::to_string(&redact_session_export(
                serde_json::to_value(session)?,
                redactor,
            ))?),
            None => Ok(serde_yaml::to_string(session)?),
        },
        "markdown" => {
            let conversation = session
                .conversation
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Session has no messages"))?;
            let markdown =
                export_session_to_markdown(conversation.messages().to_vec(), &session.name);
            Ok(match redactor {
                Some(redactor) => redactor.redact(&markdown),
                None => markdown,
            })
        }
        _ => Err(anyhow::anyhow!("Unsupported format: {format}")),
    }
}

/// Format detection falls back to the native gosling format, so a bare JSON
/// error means no importer recognised the file; foreign importers attach their
/// own format name to parse errors.
fn import_file_error(input: &str, error: anyhow::Error) -> anyhow::Error {
    if error.chain().count() == 1 && error.is::<serde_json::Error>() {
        anyhow::anyhow!(
            "Could not import {input}: not a gosling, Claude Code, Codex or Pi session file ({error})"
        )
    } else {
        anyhow::anyhow!("Could not import {input}: {error:#}")
    }
}

pub async fn handle_session_import(
    input: String,
    nostr: bool,
    working_dir: Option<PathBuf>,
) -> Result<()> {
    let is_nostr = nostr || gosling::session::is_session_share_deeplink(&input);
    let json = if is_nostr {
        #[cfg(feature = "nostr")]
        {
            nostr_share::import_session_json_from_deeplink(&input).await?
        }
        #[cfg(not(feature = "nostr"))]
        return Err(anyhow::anyhow!("gosling was not built with nostr support"));
    } else {
        String::new()
    };

    let working_dir = working_dir.unwrap_or(std::env::current_dir()?);
    let working_dir = gosling::session::import_formats::validate_import_working_dir(&working_dir)?;

    let session_manager = SessionManager::instance();
    let result = if is_nostr {
        let format = gosling::session::import_formats::detect_format(&json);
        println!("Detected format: {}", format.label());
        session_manager
            .import_session(
                &json,
                Some(SessionType::User),
                working_dir,
                gosling::session::import_formats::SessionImportTransport::Nostr,
            )
            .await?
    } else {
        session_manager
            .import_session_file(Path::new(&input), Some(SessionType::User), working_dir)
            .await
            .map_err(|error| import_file_error(&input, error))?
    };
    match result {
        gosling::session::session_manager::SessionImportOutcome::Imported(session) => {
            println!(
                "Imported session working directory: {}",
                session.working_dir.display()
            );
            println!("Session imported:");
            println!("{} - {}", session.id, terminal_safe(&session.name));
            println!(
                "Mode: {}{}. Change the mode with /mode after resuming.",
                session.gosling_mode,
                if session.restrict_tools_to_working_dirs {
                    ", tools restricted to its working directory"
                } else {
                    ""
                }
            );
        }
        gosling::session::session_manager::SessionImportOutcome::AlreadyImported(session) => {
            println!("Session already imported from this exact source:");
            println!("{} - {}", session.id, terminal_safe(&session.name));
        }
        gosling::session::session_manager::SessionImportOutcome::SourceChanged(session) => {
            println!(
                "Source file changed after its first import; skipped to prevent duplicate transcript history:"
            );
            println!("{} - {}", session.id, terminal_safe(&session.name));
        }
    }

    Ok(())
}

pub async fn handle_diagnostics(session_id: &str, output_path: Option<PathBuf>) -> Result<()> {
    let session_manager = SessionManager::instance();
    if let Err(error) = session_manager.get_session(session_id, false).await {
        return Err(anyhow::anyhow!(
            "Session '{}' not found or failed to read: {}",
            session_id,
            error
        ));
    }

    println!(
        "Generating diagnostics report for session '{}'...",
        session_id
    );

    let diagnostics_report =
        generate_diagnostics(&session_manager, session_id, DiagnosticsLevel::Full)
            .await
            .with_context(|| {
                format!(
                    "Failed to generate diagnostics report for session '{}'",
                    session_id
                )
            })?;
    let diagnostics_data = serde_json::to_vec_pretty(&diagnostics_report)
        .context("Failed to serialize diagnostics report")?;

    let output_file = if let Some(path) = output_path {
        path.clone()
    } else {
        PathBuf::from(format!("diagnostics_{}.json", session_id))
    };

    write_owner_only_output_file(&output_file, &diagnostics_data).context(format!(
        "Failed to create output file: {}",
        output_file.display()
    ))?;

    println!("Diagnostics report saved to: {}", output_file.display());
    println!("This report may contain prompts, configuration, and logs. Review it before sharing.");

    Ok(())
}

/// Writes to a temp file beside `path` and renames it into place, so an
/// interrupted write never leaves a truncated file and a symlink at `path`
/// is refused rather than followed. An existing file that is not writable is
/// still rejected, as it was when the file was opened in place.
fn write_owner_only_output_file(path: &Path, contents: &[u8]) -> io::Result<()> {
    if let Ok(metadata) = fs::symlink_metadata(path) {
        if metadata.file_type().is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "refusing to write through a symbolic link",
            ));
        }
        fs::OpenOptions::new().write(true).open(path)?;
    }
    let parent = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    #[cfg(unix)]
    temp.as_file()
        .set_permissions(fs::Permissions::from_mode(0o600))?;
    temp.write_all(contents)?;
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|error| error.error)?;
    Ok(())
}

#[cfg(test)]
mod session_export_tests {
    use super::*;
    use gosling::config::GoslingMode;
    use gosling::conversation::message::Message;
    use gosling::session::{NewPlanRevision, SessionType};

    struct ExportTestProvider;

    #[async_trait::async_trait]
    impl gosling::providers::base::Provider for ExportTestProvider {
        fn get_name(&self) -> &str {
            "export-test"
        }

        async fn stream(
            &self,
            _model_config: &gosling_providers::model::ModelConfig,
            _system: &str,
            _messages: &[Message],
            _tools: &[rmcp::model::Tool],
        ) -> Result<gosling_providers::base::MessageStream, gosling_providers::errors::ProviderError>
        {
            unreachable!("session export does not run inference")
        }
    }

    #[tokio::test]
    async fn native_json_export_includes_plan_history_from_the_core_export() {
        let temp = tempfile::tempdir().unwrap();
        let manager = SessionManager::new(temp.path().to_path_buf());
        let session = manager
            .create_session(
                temp.path().to_path_buf(),
                "plan export".to_string(),
                SessionType::User,
                GoslingMode::Approve,
            )
            .await
            .unwrap();
        let started = manager
            .plans()
            .start_or_resume(
                &session.id,
                &ExportTestProvider,
                Some("planner-model".to_string()),
                None,
            )
            .await
            .unwrap();
        manager
            .plans()
            .update_revision(
                &session.id,
                NewPlanRevision {
                    content_markdown: "# Persist me".to_string(),
                    expected_generation: started.plan.generation,
                    expected_parent_revision_id: None,
                    planner_provider: Some("export-test".to_string()),
                    planner_model: Some("planner-model".to_string()),
                },
            )
            .await
            .unwrap();
        let loaded = manager.get_session(&session.id, true).await.unwrap();

        let json = serialize_session_export(&manager, &loaded, "json", None)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert!(value.get("plan_history_v1").is_some());
        assert_eq!(
            value["plan_history_v1"]["plans"][0]["revisions"][0]["contentMarkdown"],
            "# Persist me"
        );
        assert!(!serialize_session_export(&manager, &loaded, "yaml", None)
            .await
            .unwrap()
            .contains("plan_history_v1"));
    }

    // GSL-PT-20260927-D07: `-w proj` also listed `proj-b ünï space`, `-w proj/`
    // listed nothing, and the match ignored case.
    #[test]
    fn working_dir_filter_matches_whole_path_components() {
        let root = tempfile::tempdir().unwrap();
        let proj = root.path().join("proj");
        let sibling = root.path().join("proj-b ünï space");
        std::fs::create_dir_all(proj.join("nested")).unwrap();
        std::fs::create_dir_all(&sibling).unwrap();
        let matches = |filter: &Path, session_dir: &Path| {
            let session_dir = session_dir.canonicalize().unwrap();
            working_dir_filter_roots(filter)
                .iter()
                .any(|root| session_dir.starts_with(root))
        };

        let with_slash = PathBuf::from(format!("{}/", proj.display()));
        for filter in [proj.as_path(), with_slash.as_path()] {
            assert!(matches(filter, &proj), "{}", filter.display());
            assert!(
                matches(filter, &proj.join("nested")),
                "{}",
                filter.display()
            );
            assert!(!matches(filter, &sibling), "{}", filter.display());
        }
        let upper = root.path().join("PROJ");
        if !upper.exists() {
            assert!(!matches(&upper, &proj));
        }
    }

    #[test]
    fn markdown_export_leaves_out_messages_hidden_from_the_user() {
        let messages = vec![
            Message::user()
                .with_text("# Gosling session checkpoint {\"trigger\": \"model_change\"}")
                .agent_only(),
            Message::user().with_text("Say HELLO"),
            Message::assistant().with_text("HELLO"),
        ];

        let markdown = export_session_to_markdown(messages, &"switch".to_string());

        assert!(!markdown.contains("Gosling session checkpoint"));
        assert!(markdown.contains("*Total messages: 2*"));
        assert!(markdown.contains("Say HELLO"));
        assert!(markdown.contains("HELLO"));
    }

    #[test]
    fn session_pickers_list_the_most_recent_session_first_like_session_list() {
        let at = |hour: u32| {
            chrono::DateTime::parse_from_rfc3339(&format!("2026-09-27T{hour:02}:00:00Z"))
                .unwrap()
                .with_timezone(&chrono::Utc)
        };
        let session = |id: &str, updated: u32, last_message: Option<u32>| Session {
            id: id.to_string(),
            name: format!("name-{id}"),
            updated_at: at(updated),
            last_message_at: last_message.map(at),
            ..Session::default()
        };
        let sessions = vec![
            session("old", 1, None),
            session("chatted", 2, Some(9)),
            session("renamed", 8, None),
            session("tie-a", 5, None),
            session("tie-b", 5, None),
        ];

        let ids = |items: Vec<(String, String)>| -> Vec<String> {
            items.into_iter().map(|(id, _)| id).collect()
        };
        let picked = ids(session_picker_items(&sessions));
        assert_eq!(picked, ["chatted", "renamed", "tie-a", "tie-b", "old"]);
        assert_eq!(ids(session_picker_items(&sessions)), picked);

        let mut listed = sessions.clone();
        sort_most_recent_first(&mut listed);
        let listed: Vec<String> = listed.into_iter().map(|s| s.id).collect();
        assert_eq!(listed, picked);

        let (_, label) = &session_picker_items(&sessions)[0];
        assert_eq!(label, "2026-09-27 09:00:00 UTC - name-chatted (chatted)");
    }

    #[test]
    fn terminal_safe_escapes_c0_c1_and_del_but_keeps_unicode_text() {
        assert_eq!(
            terminal_safe("a\u{1b}[31mb\u{7}c\td\u{9b}2Je\u{7f}"),
            r"a\u{1b}[31mb\u{7}c\td\u{9b}2Je\u{7f}"
        );
        assert!(matches!(
            terminal_safe("café \"quoted\" \\ 🪿"),
            Cow::Borrowed("café \"quoted\" \\ 🪿")
        ));
    }
}

#[cfg(all(test, unix))]
mod diagnostics_output_tests {
    use super::*;

    #[test]
    fn diagnostics_output_is_owner_only_even_when_overwriting() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("diagnostics.json");
        fs::write(&path, "old").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();

        write_owner_only_output_file(&path, b"new").unwrap();

        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(fs::read(&path).unwrap(), b"new");
    }

    // GSL-PT-20260912-D-8: export truncated the existing file in place and
    // wrote through a symlink at the destination.
    #[test]
    fn output_replaces_existing_file_atomically_and_refuses_symlinks() {
        use std::os::unix::fs::MetadataExt;

        let dir = tempfile::tempdir().unwrap();
        let existing = dir.path().join("existing.json");
        fs::write(&existing, "OLD").unwrap();
        let old_inode = fs::metadata(&existing).unwrap().ino();

        write_owner_only_output_file(&existing, b"NEW").unwrap();

        assert_eq!(fs::read(&existing).unwrap(), b"NEW");
        assert_ne!(fs::metadata(&existing).unwrap().ino(), old_inode);

        let target = dir.path().join("target.txt");
        fs::write(&target, "TARGET").unwrap();
        let link = dir.path().join("link.md");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        assert!(write_owner_only_output_file(&link, b"NEW").is_err());
        assert_eq!(fs::read(&target).unwrap(), b"TARGET");
        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[test]
    fn output_still_rejects_unwritable_files_and_directories() {
        let dir = tempfile::tempdir().unwrap();
        let read_only = dir.path().join("read-only.json");
        fs::write(&read_only, "OLD").unwrap();
        fs::set_permissions(&read_only, fs::Permissions::from_mode(0o444)).unwrap();

        assert!(write_owner_only_output_file(&read_only, b"NEW").is_err());
        assert_eq!(fs::read(&read_only).unwrap(), b"OLD");

        assert!(write_owner_only_output_file(dir.path(), b"NEW").is_err());
        assert!(write_owner_only_output_file(&dir.path().join("missing/x.json"), b"NEW").is_err());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}

fn export_session_to_markdown(
    messages: Vec<gosling::conversation::message::Message>,
    session_name: &String,
) -> String {
    let mut markdown_output = String::new();

    markdown_output.push_str(&format!("# Session Export: {}\n\n", session_name));

    let messages: Vec<_> = messages
        .into_iter()
        .filter(|message| message.is_user_visible())
        .collect();
    if messages.is_empty() {
        markdown_output.push_str("*(This session has no messages)*\n");
        return markdown_output;
    }

    markdown_output.push_str(&format!("*Total messages: {}*\n\n---\n\n", messages.len()));

    // Track if the last message had tool requests to properly handle tool responses
    let mut skip_next_if_tool_response = false;

    for message in &messages {
        // Check if this is a User message containing only ToolResponses
        let is_only_tool_response = message.role == rmcp::model::Role::User
            && message.content.iter().all(|content| {
                matches!(
                    content,
                    gosling::conversation::message::MessageContent::ToolResponse(_)
                )
            });

        // If the previous message had tool requests and this one is just tool responses,
        // don't create a new User section - we'll attach the responses to the tool calls
        if skip_next_if_tool_response && is_only_tool_response {
            // Export the tool responses without a User heading
            markdown_output.push_str(&message_to_markdown(message, false));
            markdown_output.push_str("\n\n---\n\n");
            skip_next_if_tool_response = false;
            continue;
        }

        // Reset the skip flag - we'll update it below if needed
        skip_next_if_tool_response = false;

        // Output the role prefix except for tool response-only messages
        if !is_only_tool_response {
            let role_prefix = match message.role {
                rmcp::model::Role::User => "### User:\n",
                rmcp::model::Role::Assistant => "### Assistant:\n",
            };
            markdown_output.push_str(role_prefix);
        }

        // Add the message content
        markdown_output.push_str(&message_to_markdown(message, false));
        markdown_output.push_str("\n\n---\n\n");

        // Check if this message has any tool requests, to handle the next message differently
        if message.content.iter().any(|content| {
            matches!(
                content,
                gosling::conversation::message::MessageContent::ToolRequest(_)
            )
        }) {
            skip_next_if_tool_response = true;
        }
    }

    markdown_output
}

/// Session pickers draw on stderr and read keys from the terminal; without one they fail with a
/// bare I/O error ("not connected") that does not say how to pick a session instead.
pub fn ensure_session_picker_terminal(selectors: &str) -> Result<()> {
    if io::stdin().is_terminal() && io::stderr().is_terminal() {
        return Ok(());
    }
    anyhow::bail!(
        "No session was specified and there is no interactive terminal for the session picker. Pass {selectors}."
    )
}

/// Prompt the user to interactively select a session
///
/// Shows a list of available sessions and lets the user select one
pub async fn prompt_interactive_session_selection(
    session_manager: &SessionManager,
    prompt: &str,
) -> Result<String> {
    let sessions = session_manager.list_sessions().await?;

    if sessions.is_empty() {
        return Err(anyhow::anyhow!("No sessions found"));
    }

    let mut selector = select(prompt);
    for (id, label) in session_picker_items(&sessions) {
        selector = selector.item(Some(id), label, "");
    }
    selector = selector.item(None, "Cancel", "");

    selector
        .interact()?
        .ok_or_else(|| crate::signal::PromptCancelled.into())
}
