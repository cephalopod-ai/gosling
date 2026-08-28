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
    assistant_text: &str,
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
                assistant_text,
                run_started_at,
                current_assistant_message_ids,
            )
    });
    let (output_files, library_files) =
        collect_report_files(current_deliverables, &library_root, &output_roots);

    if output_files.is_empty() && library_files.is_empty() {
        let prior_deliverables = artifacts.iter().filter(|artifact| {
            artifact_can_complete_research(artifact)
                && is_deliverable(Path::new(&artifact.resolved_path))
        });
        let (prior_outputs, prior_library) =
            collect_report_files(prior_deliverables, &library_root, &output_roots);
        if has_identical_report_pair(&prior_outputs, &prior_library)? {
            return Ok(());
        }
        bail!("Deep Research has no verified report and the current turn did not produce one");
    }

    if output_files.is_empty() {
        bail!("the current Deep Research turn did not produce a report in Session Outputs");
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
    assistant_text: &str,
    run_started_at: DateTime<Utc>,
    current_assistant_message_ids: &HashSet<String>,
) -> bool {
    artifact.last_seen_at >= run_started_at
        || artifact
            .source_id
            .as_ref()
            .is_some_and(|source_id| current_assistant_message_ids.contains(source_id))
        || assistant_text.contains(&artifact.resolved_path)
        || assistant_text.contains(&artifact.display_path)
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

        let mut output_artifact = artifact(&output);
        output_artifact.last_seen_at = Utc::now() - chrono::Duration::hours(1);
        let mut library_artifact = artifact(&copy);
        library_artifact.last_seen_at = Utc::now() - chrono::Duration::hours(1);
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
        fs::write(&output, "prior report").unwrap();
        fs::write(&copy, "prior report").unwrap();
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
        let mut numeric_reference = artifact(Path::new("0.50, 0.75, 0.88, 0.81…"));
        numeric_reference.relation = SessionArtifactRelation::Referenced;
        numeric_reference.provenance = SessionArtifactProvenance::AssistantMessage;
        numeric_reference.source_id = Some("current-assistant".into());

        verify_artifact_pairs(
            &state,
            &[output_artifact, library_artifact, numeric_reference],
            "Here is a direct answer to the follow-up question.",
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
