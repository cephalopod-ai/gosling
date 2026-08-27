use crate::session::{
    DeepResearchState, ExtensionState, SessionArtifact, SessionArtifactRelation, SessionManager,
};
use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
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
    tokio::task::spawn_blocking(move || verify_artifact_pairs(&state, &artifacts, &assistant_text))
        .await
        .context("completion verifier stopped unexpectedly")??;
    Ok(())
}

fn verify_artifact_pairs(
    state: &DeepResearchState,
    artifacts: &[SessionArtifact],
    assistant_text: &str,
) -> Result<()> {
    let library_root = std::fs::canonicalize(&state.library_path)
        .context("the Research Library is unavailable")?;
    if !library_root.is_dir() {
        bail!("the Research Library is unavailable");
    }

    let mut output_files = Vec::new();
    let mut library_files = Vec::new();
    for artifact in artifacts {
        if !matches!(
            artifact.relation,
            SessionArtifactRelation::Created | SessionArtifactRelation::Modified
        ) || !is_deliverable(Path::new(&artifact.resolved_path))
            || !artifact_is_reported(artifact, assistant_text)
        {
            continue;
        }
        let Ok(canonical) = std::fs::canonicalize(&artifact.resolved_path) else {
            continue;
        };
        if !canonical.is_file() {
            continue;
        }
        if canonical.starts_with(&library_root) {
            library_files.push(canonical);
        } else {
            output_files.push(canonical);
        }
    }

    if output_files.is_empty() {
        bail!("the final response did not reference a created report in Session Outputs");
    }
    if library_files.is_empty() {
        bail!("the final response did not reference a created Research Library copy");
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
        if !matching_library_files
            .into_iter()
            .map(|candidate| sha256_file(candidate))
            .collect::<Result<Vec<_>>>()?
            .iter()
            .any(|candidate_hash| candidate_hash == &output_hash)
        {
            bail!(
                "the Session Output and Research Library copy named {} are not identical",
                name.to_string_lossy()
            );
        }
    }

    Ok(())
}

fn artifact_is_reported(artifact: &SessionArtifact, assistant_text: &str) -> bool {
    assistant_text.contains(&artifact.resolved_path)
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
    use crate::session::SessionArtifactProvenance;
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
        };
        let assistant_text = format!("Reports: {} and {}", output.display(), copy.display());

        verify_artifact_pairs(
            &state,
            &[artifact(&output), artifact(&copy)],
            &assistant_text,
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
        };
        let assistant_text = format!("Reports: {} and {}", output.display(), copy.display());

        let error = verify_artifact_pairs(
            &state,
            &[artifact(&output), artifact(&copy)],
            &assistant_text,
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
        };

        let error = verify_artifact_pairs(
            &state,
            &[artifact(&output), artifact(&copy)],
            &output.to_string_lossy(),
        )
        .unwrap_err();

        assert!(error
            .to_string()
            .contains("did not reference a created Research Library copy"));
    }
}
