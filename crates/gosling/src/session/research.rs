//! Deep Research deliverable detection shared by the reply loop and the ACP
//! completion gate.

use crate::session::artifacts::{SessionArtifact, SessionArtifactRelation};
use crate::session::{DeepResearchState, SessionManager};
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const MAX_SCAN_ENTRIES: usize = 5_000;
const MAX_SCAN_DEPTH: usize = 6;
const ARTIFACT_PAGE_SIZE: usize = 200;
pub const MAX_RESEARCH_ARTIFACTS: usize = 2_000;

/// The hidden instruction sent once when a research turn is about to end
/// without a report in Session Outputs.
pub const RESEARCH_DELIVERABLE_NUDGE: &str = "Your turn is about to end, but this Deep Research \
session has no report in Session Outputs yet. If you are waiting on the user for something \
specific, say so in one line and stop. Otherwise write the report now: save it under a workspace \
output folder, then make the identical Research Library copy, and reference both exact paths in \
your final message.";

pub fn is_deliverable(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some(
            "csv"
                | "doc"
                | "docx"
                | "html"
                | "json"
                | "md"
                | "pdf"
                | "rtf"
                | "tsv"
                | "txt"
                | "yaml"
                | "yml"
        )
    )
}

/// Bounded walk of `root` for regular files accepted by `keep`; hidden entries
/// and symbolic links are skipped, as in the Research Library listing.
pub fn walk_files(
    root: &Path,
    mut keep: impl FnMut(&Path, &std::fs::Metadata) -> bool,
) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut directories = vec![(root.to_path_buf(), 0usize)];
    let mut visited = 0usize;
    while let Some((directory, depth)) = directories.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            visited += 1;
            if visited > MAX_SCAN_ENTRIES {
                return found;
            }
            let path = entry.path();
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with('.'))
            {
                continue;
            }
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            if metadata.file_type().is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                if depth < MAX_SCAN_DEPTH {
                    directories.push((path, depth + 1));
                }
                continue;
            }
            if keep(&path, &metadata) {
                found.push(path);
            }
        }
    }
    found
}

pub fn written_since(metadata: &std::fs::Metadata, since: DateTime<Utc>) -> bool {
    metadata
        .modified()
        .map(|modified| DateTime::<Utc>::from(modified) >= since)
        .unwrap_or(false)
}

/// Deliverable files under `roots` written during the run and named in the
/// final assistant text. A file the model wrote with a shell heredoc never
/// reaches the artifact inventory, but the research contract makes it name
/// the path in its final message, which is enough to find it deterministically
/// without mistaking another session's output for this turn's.
pub fn deliverables_written_since(
    roots: &[PathBuf],
    since: DateTime<Utc>,
    final_text: &str,
) -> Vec<PathBuf> {
    if final_text.trim().is_empty() {
        return Vec::new();
    }
    let mut found = Vec::new();
    for root in roots {
        found.extend(walk_files(root, |path, metadata| {
            is_deliverable(path)
                && written_since(metadata, since)
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| final_text.contains(name))
        }));
    }
    found
}

pub fn canonical_dirs(paths: &[String]) -> Vec<PathBuf> {
    paths
        .iter()
        .filter_map(|path| std::fs::canonicalize(path).ok())
        .filter(|path| path.is_dir())
        .collect()
}

pub async fn list_all_artifacts(
    session_manager: &SessionManager,
    session_id: &str,
) -> Result<Vec<SessionArtifact>> {
    let mut artifacts = Vec::new();
    let mut cursor = None;
    loop {
        let page = session_manager
            .list_session_artifacts(session_id, cursor.as_deref(), ARTIFACT_PAGE_SIZE)
            .await?;
        if page.total_count > MAX_RESEARCH_ARTIFACTS {
            anyhow::bail!("the session has too many artifacts to verify safely");
        }
        artifacts.extend(page.artifacts);
        let Some(next_cursor) = page.next_cursor else {
            break;
        };
        cursor = Some(next_cursor);
    }
    Ok(artifacts)
}

/// Whether the turn that started at `since` has produced a report under one
/// of the session's workspace output roots, by inventory or on disk.
pub async fn turn_wrote_output_deliverable(
    session_manager: &SessionManager,
    session_id: &str,
    state: &DeepResearchState,
    since: DateTime<Utc>,
    final_text: &str,
) -> bool {
    let output_roots = canonical_dirs(&state.output_paths);
    if output_roots.is_empty() {
        return true;
    }
    let inventory = match list_all_artifacts(session_manager, session_id).await {
        Ok(artifacts) => artifacts,
        Err(_) => return true,
    };
    let seen: HashSet<PathBuf> = inventory
        .iter()
        .filter(|artifact| {
            matches!(
                artifact.relation,
                SessionArtifactRelation::Created | SessionArtifactRelation::Modified
            ) && artifact.last_seen_at >= since
                && is_deliverable(Path::new(&artifact.resolved_path))
        })
        .filter_map(|artifact| std::fs::canonicalize(&artifact.resolved_path).ok())
        .filter(|path| output_roots.iter().any(|root| path.starts_with(root)))
        .collect();
    if !seen.is_empty() {
        return true;
    }
    !deliverables_written_since(&output_roots, since, final_text).is_empty()
}

/// A folder name derived from the session title for archive copies that have
/// no topic folder of their own.
pub fn topic_folder_name(session_name: &str) -> String {
    let mut name = String::new();
    let mut last_space = true;
    for character in session_name.chars() {
        let keep = character.is_alphanumeric() || matches!(character, '-' | '_' | '.');
        if keep {
            name.push(character);
            last_space = false;
        } else if !last_space {
            name.push(' ');
            last_space = true;
        }
        if name.chars().count() >= 80 {
            break;
        }
    }
    let name = name.trim().trim_end_matches('.').to_string();
    if name.is_empty() {
        "Deep Research".to_string()
    } else {
        name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topic_folder_names_are_filesystem_safe_and_bounded() {
        assert_eq!(
            topic_folder_name("Neural Architecture: Research / Reconciliation"),
            "Neural Architecture Research Reconciliation"
        );
        assert_eq!(topic_folder_name("   "), "Deep Research");
        assert_eq!(topic_folder_name("trailing..."), "trailing");
        assert!(topic_folder_name(&"x".repeat(200)).chars().count() <= 80);
    }

    #[test]
    fn deliverables_written_since_requires_recency_and_a_mention() {
        let root = tempfile::tempdir().unwrap();
        let outputs = root.path().join("outputs");
        std::fs::create_dir_all(outputs.join("topic")).unwrap();
        let report = outputs.join("topic").join("report.md");
        let scratch = outputs.join("topic").join("scratch.md");
        std::fs::write(&report, "r").unwrap();
        std::fs::write(&scratch, "s").unwrap();
        let since = Utc::now() - chrono::Duration::seconds(60);

        let found = deliverables_written_since(
            std::slice::from_ref(&outputs),
            since,
            "Wrote topic/report.md to Session Outputs.",
        );
        assert_eq!(found, vec![report.clone()]);
        assert!(deliverables_written_since(std::slice::from_ref(&outputs), since, "").is_empty());
        assert!(deliverables_written_since(
            &[outputs],
            Utc::now() + chrono::Duration::seconds(60),
            "report.md"
        )
        .is_empty());
    }
}
