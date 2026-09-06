use crate::session::{
    DeepResearchState, ExtensionState, SessionArtifact, SessionArtifactProvenance,
    SessionArtifactRelation, SessionManager,
};
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

const MAX_RESEARCH_ARTIFACTS: usize = 2_000;
const MAX_RESEARCH_DELIVERABLE_BYTES: u64 = 100 * 1024 * 1024;
const ARTIFACT_PAGE_SIZE: usize = 200;
const MAX_LIBRARY_SCAN_ENTRIES: usize = 5_000;
const MAX_LIBRARY_SCAN_DEPTH: usize = 6;
const HASH_BUFFER_SIZE: usize = 64 * 1024;

pub(super) async fn verify_deep_research_completion(
    session_manager: &SessionManager,
    session_id: &str,
    assistant_text: &str,
    run_started_at: DateTime<Utc>,
    current_assistant_message_ids: &HashSet<String>,
) -> Result<()> {
    let session = session_manager.get_session(session_id, false).await?;
    let Some(state) = DeepResearchState::from_extension_data(&session.extension_data) else {
        return Ok(());
    };

    let mut artifacts = Vec::new();
    let mut cursor = None;
    loop {
        let page = session_manager
            .list_session_artifacts(session_id, cursor.as_deref(), ARTIFACT_PAGE_SIZE)
            .await?;
        if page.total_count > MAX_RESEARCH_ARTIFACTS {
            bail!("the session has too many artifacts to verify safely");
        }
        artifacts.extend(page.artifacts);
        let Some(next_cursor) = page.next_cursor else {
            break;
        };
        cursor = Some(next_cursor);
    }

    let assistant_text = assistant_text.to_string();
    let current_assistant_message_ids = current_assistant_message_ids.clone();
    tokio::task::spawn_blocking(move || {
        verify_artifact_pairs(
            &state,
            &artifacts,
            &assistant_text,
            run_started_at,
            &current_assistant_message_ids,
        )
    })
    .await
    .context("completion verifier stopped unexpectedly")??;
    Ok(())
}

fn verify_artifact_pairs(
    state: &DeepResearchState,
    artifacts: &[SessionArtifact],
    _assistant_text: &str,
    run_started_at: DateTime<Utc>,
    current_assistant_message_ids: &HashSet<String>,
) -> Result<()> {
    let library_root = std::fs::canonicalize(&state.library_path)
        .context("the Research Library is unavailable")?;
    if !library_root.is_dir() {
        bail!("the Research Library is unavailable");
    }
    let output_roots = state
        .output_paths
        .iter()
        .map(std::fs::canonicalize)
        .collect::<std::io::Result<Vec<_>>>()
        .context("a workspace output folder is unavailable")?;
    if output_roots.is_empty() || output_roots.iter().any(|root| !root.is_dir()) {
        bail!("a workspace output folder is unavailable");
    }

    let current_deliverables = artifacts.iter().filter(|artifact| {
        artifact_can_complete_research(artifact)
            && is_deliverable(Path::new(&artifact.resolved_path))
            && artifact_belongs_to_current_run(
                artifact,
                run_started_at,
                current_assistant_message_ids,
            )
    });
    let (output_files, mut library_files) =
        collect_report_files(current_deliverables, &library_root, &output_roots);

    if output_files.is_empty() && library_files.is_empty() {
        let prior_deliverables = artifacts.iter().filter(|artifact| {
            artifact_can_complete_research(artifact)
                && is_deliverable(Path::new(&artifact.resolved_path))
        });
        let (prior_outputs, mut prior_library) =
            collect_report_files(prior_deliverables, &library_root, &output_roots);
        if prior_library.is_empty() {
            // Same blind spot as below: a copy made with `cp` is absent from the
            // inventory. Whether a verified pair already exists is a question
            // about the files, so no run window applies here.
            prior_library = discover_library_copies(&library_root, &prior_outputs, None);
        }
        if has_identical_report_pair(&prior_outputs, &prior_library)? {
            return Ok(());
        }
        bail!("Deep Research has no verified report and the current turn did not produce one");
    }

    if output_files.is_empty() {
        bail!("the current Deep Research turn did not produce a report in Session Outputs");
    }
    if library_files.is_empty() {
        // The archive copy is normally made with `cp`, so the file never passes
        // through a structured file tool and never reaches the artifact
        // inventory. Requiring the tool would mean re-emitting an entire report
        // through the model just to copy it. Placement, recency, and byte
        // identity are still verified below against the real files on disk, so
        // this discovers the copy rather than trusting that it was made.
        library_files = discover_library_copies(&library_root, &output_files, Some(run_started_at));
    }
    if library_files.is_empty() {
        bail!("the current Deep Research turn did not produce a Research Library copy");
    }

    for output in &output_files {
        let Some(name) = output.file_name() else {
            bail!("a reported Session Output has no filename");
        };
        let matching_library_files = library_files
            .iter()
            .filter(|candidate| candidate.file_name() == Some(name))
            .collect::<Vec<_>>();
        if matching_library_files.is_empty() {
            bail!(
                "the Research Library has no reported copy named {}",
                name.to_string_lossy()
            );
        }
        let output_hash = sha256_file(output)?;
        let mut found_identical_copy = false;
        for candidate in matching_library_files {
            if sha256_file(candidate)? == output_hash {
                found_identical_copy = true;
                break;
            }
        }
        if !found_identical_copy {
            bail!(
                "the Session Output and Research Library copy named {} are not identical",
                name.to_string_lossy()
            );
        }
    }

    Ok(())
}

fn collect_report_files<'a>(
    artifacts: impl IntoIterator<Item = &'a SessionArtifact>,
    library_root: &Path,
    output_roots: &[PathBuf],
) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut output_files = Vec::new();
    let mut library_files = Vec::new();
    for artifact in artifacts {
        let Ok(canonical) = std::fs::canonicalize(&artifact.resolved_path) else {
            continue;
        };
        if !canonical.is_file() {
            continue;
        }
        if canonical.starts_with(library_root) {
            library_files.push(canonical);
        } else if output_roots.iter().any(|root| canonical.starts_with(root)) {
            output_files.push(canonical);
        }
    }
    (output_files, library_files)
}

/// Files under the Research Library whose name matches a reported Session
/// Output, optionally narrowed to those written since `written_since`. The
/// caller still compares contents, so a same-named unrelated file cannot
/// satisfy the gate.
fn discover_library_copies(
    library_root: &Path,
    output_files: &[PathBuf],
    written_since: Option<DateTime<Utc>>,
) -> Vec<PathBuf> {
    let wanted: HashSet<&std::ffi::OsStr> = output_files
        .iter()
        .filter_map(|path| path.file_name())
        .collect();
    if wanted.is_empty() {
        return Vec::new();
    }

    let mut found = Vec::new();
    let mut directories = vec![(library_root.to_path_buf(), 0usize)];
    let mut visited = 0usize;
    while let Some((directory, depth)) = directories.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            visited += 1;
            if visited > MAX_LIBRARY_SCAN_ENTRIES {
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
                if depth < MAX_LIBRARY_SCAN_DEPTH {
                    directories.push((path, depth + 1));
                }
                continue;
            }
            if !path.file_name().is_some_and(|name| wanted.contains(name)) {
                continue;
            }
            let in_window = written_since.is_none_or(|since| {
                metadata
                    .modified()
                    .map(|modified| DateTime::<Utc>::from(modified) >= since)
                    .unwrap_or(false)
            });
            if in_window {
                found.push(path);
            }
        }
    }
    found
}

fn has_identical_report_pair(output_files: &[PathBuf], library_files: &[PathBuf]) -> Result<bool> {
    for output in output_files {
        let Some(name) = output.file_name() else {
            continue;
        };
        let output_hash = sha256_file(output)?;
        for candidate in library_files
            .iter()
            .filter(|candidate| candidate.file_name() == Some(name))
        {
            if candidate.metadata()?.len() == output.metadata()?.len()
                && sha256_file(candidate)? == output_hash
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn artifact_can_complete_research(artifact: &SessionArtifact) -> bool {
    matches!(
        artifact.relation,
        SessionArtifactRelation::Created | SessionArtifactRelation::Modified
    ) || matches!(
        (&artifact.relation, &artifact.provenance),
        (
            SessionArtifactRelation::Referenced,
            SessionArtifactProvenance::AssistantMessage
        )
    )
}

fn artifact_belongs_to_current_run(
    artifact: &SessionArtifact,
    run_started_at: DateTime<Utc>,
    current_assistant_message_ids: &HashSet<String>,
) -> bool {
    if artifact.provenance == SessionArtifactProvenance::AssistantMessage {
        artifact.first_seen_at >= run_started_at
    } else {
        artifact.last_seen_at >= run_started_at
            || artifact
                .source_id
                .as_ref()
                .is_some_and(|source_id| current_assistant_message_ids.contains(source_id))
    }
}

fn is_deliverable(path: &Path) -> bool {
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

fn sha256_file(path: &PathBuf) -> Result<[u8; 32]> {
    let mut file = File::open(path)?;
    if file.metadata()?.len() > MAX_RESEARCH_DELIVERABLE_BYTES {
        bail!("a reported research deliverable exceeds the 100 MB verification limit");
    }
    let mut hash = Sha256::new();
    let mut buffer = vec![0; HASH_BUFFER_SIZE];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    Ok(hash.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::fs;
    use std::time::SystemTime;

    fn artifact(path: &Path) -> SessionArtifact {
        SessionArtifact {
            session_id: "research-session".into(),
            display_path: path.to_string_lossy().into_owned(),
            resolved_path: path.to_string_lossy().into_owned(),
            base_working_dir: path.parent().unwrap().to_string_lossy().into_owned(),
            workspace_id: Some("workspace".into()),
            mime_type: Some("text/markdown".into()),
            relation: SessionArtifactRelation::Created,
            provenance: SessionArtifactProvenance::BuiltInTool,
            source_id: Some("tool-call".into()),
            first_seen_at: Utc::now(),
            last_seen_at: Utc::now(),
        }
    }

    fn run_started_at() -> DateTime<Utc> {
        Utc::now() - chrono::Duration::seconds(1)
    }

    #[test]
    fn verifies_reported_identical_output_and_library_copy() {
        let root = tempfile::tempdir().unwrap();
        let outputs = root.path().join("outputs");
        let library = root.path().join("library");
        fs::create_dir_all(&outputs).unwrap();
        fs::create_dir_all(&library).unwrap();
        let output = outputs.join("report.md");
        let copy = library.join("report.md");
        fs::write(&output, "verified report").unwrap();
        fs::write(&copy, "verified report").unwrap();
        let state = DeepResearchState {
            library_path: library.to_string_lossy().into_owned(),
            output_paths: vec![outputs.to_string_lossy().into_owned()],
        };
        let assistant_text = format!("Reports: {} and {}", output.display(), copy.display());

        verify_artifact_pairs(
            &state,
            &[artifact(&output), artifact(&copy)],
            &assistant_text,
            run_started_at(),
            &HashSet::new(),
        )
        .unwrap();
    }

    /// The archive copy is made with `cp`, so it never reaches the artifact
    /// inventory and the gate used to fail a turn whose copies were on disk and
    /// byte-identical. (Observed on session 20260904_7, 2026-09-06.)
    #[test]
    fn accepts_a_library_copy_made_outside_the_structured_file_tools() {
        let root = tempfile::tempdir().unwrap();
        let outputs = root.path().join("outputs");
        let library = root.path().join("library").join("Topic - 2026-09-06");
        fs::create_dir_all(&outputs).unwrap();
        fs::create_dir_all(&library).unwrap();
        let output = outputs.join("SYNTHESIS.md");
        let copy = library.join("SYNTHESIS.md");
        fs::write(&output, "verified report").unwrap();
        fs::write(&copy, "verified report").unwrap();
        let state = DeepResearchState {
            library_path: root.path().join("library").to_string_lossy().into_owned(),
            output_paths: vec![outputs.to_string_lossy().into_owned()],
        };

        // Only the Session Outputs side is in the inventory, exactly as when the
        // copy is made by a shell command.
        verify_artifact_pairs(
            &state,
            &[artifact(&output)],
            "",
            run_started_at(),
            &HashSet::new(),
        )
        .unwrap();
    }

    #[test]
    fn a_discovered_library_copy_that_differs_is_still_rejected() {
        let root = tempfile::tempdir().unwrap();
        let outputs = root.path().join("outputs");
        let library = root.path().join("library");
        fs::create_dir_all(&outputs).unwrap();
        fs::create_dir_all(&library).unwrap();
        let output = outputs.join("SYNTHESIS.md");
        let copy = library.join("SYNTHESIS.md");
        fs::write(&output, "verified report").unwrap();
        fs::write(&copy, "a different report").unwrap();
        let state = DeepResearchState {
            library_path: library.to_string_lossy().into_owned(),
            output_paths: vec![outputs.to_string_lossy().into_owned()],
        };

        let error = verify_artifact_pairs(
            &state,
            &[artifact(&output)],
            "",
            run_started_at(),
            &HashSet::new(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("not identical"), "{error}");
    }

    /// A copy left over from an earlier run does not let a later turn that
    /// wrote nothing to the library pass.
    #[test]
    fn a_library_copy_predating_the_run_is_not_discovered() {
        let root = tempfile::tempdir().unwrap();
        let outputs = root.path().join("outputs");
        let library = root.path().join("library");
        fs::create_dir_all(&outputs).unwrap();
        fs::create_dir_all(&library).unwrap();
        let output = outputs.join("SYNTHESIS.md");
        let copy = library.join("SYNTHESIS.md");
        fs::write(&output, "verified report").unwrap();
        fs::write(&copy, "verified report").unwrap();

        let discovered = discover_library_copies(
            &std::fs::canonicalize(&library).unwrap(),
            &[std::fs::canonicalize(&output).unwrap()],
            Some(Utc::now() + chrono::Duration::seconds(60)),
        );
        assert!(discovered.is_empty(), "{discovered:?}");
    }

    /// A turn that only refreshes the archive copy records no Session Outputs
    /// artifact of its own, so the gate falls back to "is there already a
    /// verified pair?" -- which must also see copies made with `cp`.
    #[test]
    fn accepts_a_prior_pair_whose_library_copy_is_only_on_disk() {
        let root = tempfile::tempdir().unwrap();
        let outputs = root.path().join("outputs");
        let library = root.path().join("library").join("Topic");
        fs::create_dir_all(&outputs).unwrap();
        fs::create_dir_all(&library).unwrap();
        let output = outputs.join("SYNTHESIS.md");
        let copy = library.join("SYNTHESIS.md");
        fs::write(&output, "verified report").unwrap();
        fs::write(&copy, "verified report").unwrap();
        let state = DeepResearchState {
            library_path: root.path().join("library").to_string_lossy().into_owned(),
            output_paths: vec![outputs.to_string_lossy().into_owned()],
        };

        let mut prior = artifact(&output);
        prior.first_seen_at = Utc::now() - chrono::Duration::hours(2);
        prior.last_seen_at = Utc::now() - chrono::Duration::hours(2);

        verify_artifact_pairs(&state, &[prior], "", run_started_at(), &HashSet::new()).unwrap();
    }

    #[test]
    fn verifies_external_provider_reports_discovered_from_persisted_assistant_text() {
        let root = tempfile::tempdir().unwrap();
        let outputs = root.path().join("outputs");
        let library = root.path().join("library");
        fs::create_dir_all(&outputs).unwrap();
        fs::create_dir_all(&library).unwrap();
        let output = outputs.join("report.md");
        let copy = library.join("report.md");
        fs::write(&output, "verified report").unwrap();
        fs::write(&copy, "verified report").unwrap();
        let state = DeepResearchState {
            library_path: library.to_string_lossy().into_owned(),
            output_paths: vec![outputs.to_string_lossy().into_owned()],
        };
        let mut output_artifact = artifact(&output);
        output_artifact.relation = SessionArtifactRelation::Referenced;
        output_artifact.provenance = SessionArtifactProvenance::AssistantMessage;
        output_artifact.source_id = Some("current-assistant".into());
        output_artifact.last_seen_at = Utc::now() - chrono::Duration::hours(1);
        let mut library_artifact = artifact(&copy);
        library_artifact.relation = SessionArtifactRelation::Referenced;
        library_artifact.provenance = SessionArtifactProvenance::AssistantMessage;
        library_artifact.source_id = Some("current-assistant".into());
        library_artifact.last_seen_at = Utc::now() - chrono::Duration::hours(1);
        let current_assistant_message_ids = HashSet::from(["current-assistant".into()]);

        verify_artifact_pairs(
            &state,
            &[output_artifact, library_artifact],
            "",
            run_started_at(),
            &current_assistant_message_ids,
        )
        .unwrap();
    }

    #[test]
    fn rejects_mismatched_research_copy() {
        let root = tempfile::tempdir().unwrap();
        let outputs = root.path().join("outputs");
        let library = root.path().join("library");
        fs::create_dir_all(&outputs).unwrap();
        fs::create_dir_all(&library).unwrap();
        let output = outputs.join("report.md");
        let copy = library.join("report.md");
        fs::write(&output, "final report").unwrap();
        fs::write(&copy, "stale report").unwrap();
        let state = DeepResearchState {
            library_path: library.to_string_lossy().into_owned(),
            output_paths: vec![outputs.to_string_lossy().into_owned()],
        };
        let assistant_text = format!("Reports: {} and {}", output.display(), copy.display());

        let error = verify_artifact_pairs(
            &state,
            &[artifact(&output), artifact(&copy)],
            &assistant_text,
            run_started_at(),
            &HashSet::new(),
        )
        .unwrap_err();

        assert!(error.to_string().contains("are not identical"));
    }

    #[test]
    fn rejects_unreported_research_copy() {
        let root = tempfile::tempdir().unwrap();
        let outputs = root.path().join("outputs");
        let library = root.path().join("library");
        fs::create_dir_all(&outputs).unwrap();
        fs::create_dir_all(&library).unwrap();
        let output = outputs.join("report.md");
        let copy = library.join("report.md");
        fs::write(&output, "final report").unwrap();
        fs::write(&copy, "final report").unwrap();
        let state = DeepResearchState {
            library_path: library.to_string_lossy().into_owned(),
            output_paths: vec![outputs.to_string_lossy().into_owned()],
        };

        let output_artifact = artifact(&output);
        let mut library_artifact = artifact(&copy);
        library_artifact.last_seen_at = Utc::now() - chrono::Duration::hours(1);
        // The copy predates the run on disk too, so neither the inventory nor
        // the filesystem can attribute it to this turn.
        File::options()
            .write(true)
            .open(&copy)
            .unwrap()
            .set_modified(SystemTime::now() - std::time::Duration::from_secs(3600))
            .unwrap();
        let error = verify_artifact_pairs(
            &state,
            &[output_artifact, library_artifact],
            &output.to_string_lossy(),
            run_started_at(),
            &HashSet::new(),
        )
        .unwrap_err();

        assert!(error
            .to_string()
            .contains("did not produce a Research Library copy"));
    }

    #[test]
    fn rejects_a_report_outside_workspace_output_folders() {
        let root = tempfile::tempdir().unwrap();
        let outputs = root.path().join("outputs");
        let scratch = root.path().join("scratch");
        let library = root.path().join("library");
        fs::create_dir_all(&outputs).unwrap();
        fs::create_dir_all(&scratch).unwrap();
        fs::create_dir_all(&library).unwrap();
        let misplaced = scratch.join("report.md");
        let copy = library.join("report.md");
        fs::write(&misplaced, "final report").unwrap();
        fs::write(&copy, "final report").unwrap();
        let state = DeepResearchState {
            library_path: library.to_string_lossy().into_owned(),
            output_paths: vec![outputs.to_string_lossy().into_owned()],
        };
        let assistant_text = format!("Reports: {} and {}", misplaced.display(), copy.display());

        let error = verify_artifact_pairs(
            &state,
            &[artifact(&misplaced), artifact(&copy)],
            &assistant_text,
            run_started_at(),
            &HashSet::new(),
        )
        .unwrap_err();

        assert!(error.to_string().contains("Session Outputs"));
    }

    #[test]
    fn allows_a_follow_up_without_a_current_deliverable() {
        let root = tempfile::tempdir().unwrap();
        let outputs = root.path().join("outputs");
        let library = root.path().join("library");
        fs::create_dir_all(&outputs).unwrap();
        fs::create_dir_all(&library).unwrap();
        let output = outputs.join("prior-report.md");
        let copy = library.join("prior-report.md");
        let rementioned_output = outputs.join("unpaired-report.md");
        fs::write(&output, "prior report").unwrap();
        fs::write(&copy, "prior report").unwrap();
        fs::write(
            &rementioned_output,
            "previously reported without a library copy",
        )
        .unwrap();
        let state = DeepResearchState {
            library_path: library.to_string_lossy().into_owned(),
            output_paths: vec![outputs.to_string_lossy().into_owned()],
        };
        let mut output_artifact = artifact(&output);
        output_artifact.source_id = Some("prior-assistant".into());
        output_artifact.last_seen_at = Utc::now() - chrono::Duration::hours(1);
        let mut library_artifact = artifact(&copy);
        library_artifact.source_id = Some("prior-assistant".into());
        library_artifact.last_seen_at = Utc::now() - chrono::Duration::hours(1);
        let mut rementioned_artifact = artifact(&rementioned_output);
        rementioned_artifact.relation = SessionArtifactRelation::Referenced;
        rementioned_artifact.provenance = SessionArtifactProvenance::AssistantMessage;
        rementioned_artifact.source_id = Some("current-assistant".into());
        rementioned_artifact.first_seen_at = Utc::now() - chrono::Duration::hours(1);
        let mut numeric_reference = artifact(Path::new("0.50, 0.75, 0.88, 0.81…"));
        numeric_reference.relation = SessionArtifactRelation::Referenced;
        numeric_reference.provenance = SessionArtifactProvenance::AssistantMessage;
        numeric_reference.source_id = Some("current-assistant".into());

        verify_artifact_pairs(
            &state,
            &[
                output_artifact,
                library_artifact,
                rementioned_artifact,
                numeric_reference,
            ],
            &format!("The report is at {}.", rementioned_output.display()),
            run_started_at(),
            &HashSet::from(["current-assistant".into()]),
        )
        .unwrap();
    }

    #[test]
    fn rejects_an_initial_turn_without_a_deliverable() {
        let root = tempfile::tempdir().unwrap();
        let outputs = root.path().join("outputs");
        let library = root.path().join("library");
        fs::create_dir_all(&outputs).unwrap();
        fs::create_dir_all(&library).unwrap();
        let state = DeepResearchState {
            library_path: library.to_string_lossy().into_owned(),
            output_paths: vec![outputs.to_string_lossy().into_owned()],
        };

        let error = verify_artifact_pairs(
            &state,
            &[],
            "Here is an answer without a report.",
            run_started_at(),
            &HashSet::from(["current-assistant".into()]),
        )
        .unwrap_err();

        assert!(error.to_string().contains("no verified report"));
    }
}
