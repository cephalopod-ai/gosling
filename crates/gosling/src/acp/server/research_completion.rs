use crate::session::artifacts::DiscoveredArtifact;
use crate::session::research;
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

const MAX_RESEARCH_DELIVERABLE_BYTES: u64 = 100 * 1024 * 1024;
const MAX_LIBRARY_SCAN_ENTRIES: usize = 5_000;
const MAX_LIBRARY_SCAN_DEPTH: usize = 6;
const HASH_BUFFER_SIZE: usize = 64 * 1024;

/// Verifies the dual-destination contract for a Deep Research turn and, when
/// the model produced a report on only one side, makes the other copy itself.
/// Returns operator-facing notes describing anything it did.
pub(super) async fn verify_deep_research_completion(
    session_manager: &SessionManager,
    session_id: &str,
    assistant_text: &str,
    run_started_at: DateTime<Utc>,
    current_assistant_message_ids: &HashSet<String>,
) -> Result<Vec<String>> {
    let session = session_manager.get_session(session_id, false).await?;
    let Some(state) = DeepResearchState::from_extension_data(&session.extension_data) else {
        return Ok(Vec::new());
    };

    let mut artifacts = research::list_all_artifacts(session_manager, session_id).await?;

    let closeout = {
        let state = state.clone();
        let artifacts = artifacts.clone();
        let assistant_text = assistant_text.to_string();
        let current_assistant_message_ids = current_assistant_message_ids.clone();
        let session_name = session.name.clone();
        tokio::task::spawn_blocking(move || {
            close_out_deliverables(
                &state,
                &artifacts,
                &assistant_text,
                run_started_at,
                &current_assistant_message_ids,
                &session_name,
            )
        })
        .await
        .context("completion closeout stopped unexpectedly")??
    };

    if !closeout.register.is_empty() {
        let working_dir = session.working_dir.to_string_lossy().into_owned();
        let discovered: Vec<DiscoveredArtifact> = closeout
            .register
            .iter()
            .map(|path| DiscoveredArtifact {
                display_path: path.to_string_lossy().into_owned(),
                resolved_path: path.to_string_lossy().into_owned(),
                base_working_dir: working_dir.clone(),
                workspace_id: session.workspace_id.clone(),
                mime_type: None,
                relation: SessionArtifactRelation::Created,
                provenance: SessionArtifactProvenance::BuiltInTool,
                source_id: None,
            })
            .collect();
        session_manager
            .upsert_session_artifacts(session_id, &discovered)
            .await?;
        artifacts = research::list_all_artifacts(session_manager, session_id).await?;
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
    Ok(closeout.notes)
}

#[derive(Debug, Default)]
struct Closeout {
    /// Files to add to the artifact inventory: reports the model wrote outside
    /// the structured file tools, plus the copies made here.
    register: Vec<PathBuf>,
    notes: Vec<String>,
}

/// Finds this turn's deliverables on both sides, including ones written with a
/// shell command, and mirrors whichever side is missing. The verifier still
/// judges the result afterwards; this only removes the failure modes where the
/// model did the research but not the bookkeeping.
fn close_out_deliverables(
    state: &DeepResearchState,
    artifacts: &[SessionArtifact],
    assistant_text: &str,
    run_started_at: DateTime<Utc>,
    current_assistant_message_ids: &HashSet<String>,
    session_name: &str,
) -> Result<Closeout> {
    let library_root = std::fs::canonicalize(&state.library_path)
        .context("the Research Library is unavailable")?;
    let output_roots = research::canonical_dirs(&state.output_paths);
    if output_roots.is_empty() {
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
    let (mut outputs, mut library) =
        collect_report_files(current_deliverables, &library_root, &output_roots);
    let mut closeout = Closeout::default();

    if outputs.is_empty() {
        outputs =
            research::deliverables_written_since(&output_roots, run_started_at, assistant_text);
        closeout.register.extend(outputs.iter().cloned());
    }
    if library.is_empty() {
        library = if outputs.is_empty() {
            research::deliverables_written_since(
                std::slice::from_ref(&library_root),
                run_started_at,
                assistant_text,
            )
        } else {
            discover_library_copies(&library_root, &outputs, Some(run_started_at))
        };
        closeout.register.extend(library.iter().cloned());
    }

    if !outputs.is_empty() {
        for output in &outputs {
            let Some(name) = output.file_name() else {
                continue;
            };
            if library.iter().any(|copy| copy.file_name() == Some(name)) {
                continue;
            }
            let topic = output_roots
                .iter()
                .find_map(|root| output.strip_prefix(root).ok())
                .and_then(|relative| relative.parent())
                .filter(|parent| !parent.as_os_str().is_empty())
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from(research::topic_folder_name(session_name)));
            let destination = mirror_file(output, &library_root.join(topic), name)?;
            closeout.notes.push(format!(
                "Archived to Research Library: {}",
                destination.display()
            ));
            closeout.register.push(destination.clone());
            library.push(destination);
        }
    } else if !library.is_empty() {
        for copy in &library {
            let Some(name) = copy.file_name() else {
                continue;
            };
            let relative_dir = copy
                .strip_prefix(&library_root)
                .ok()
                .and_then(|relative| relative.parent())
                .map(Path::to_path_buf)
                .unwrap_or_default();
            let destination = mirror_file(copy, &output_roots[0].join(relative_dir), name)?;
            closeout.notes.push(format!(
                "Copied to Session Outputs: {}",
                destination.display()
            ));
            closeout.register.push(destination);
        }
    }

    Ok(closeout)
}

/// Copies `source` into `directory` as `name`, never replacing a different
/// file of that name: an existing identical copy is reused and a different one
/// is kept, with the new copy taking a dated name.
fn mirror_file(source: &Path, directory: &Path, name: &std::ffi::OsStr) -> Result<PathBuf> {
    std::fs::create_dir_all(directory)
        .with_context(|| format!("cannot create {}", directory.display()))?;
    let mut destination = directory.join(name);
    if destination.exists() {
        if sha256_file(&destination)? == sha256_file(&source.to_path_buf())? {
            return Ok(destination);
        }
        let stem = Path::new(name)
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        let extension = Path::new(name)
            .extension()
            .map(|extension| format!(".{}", extension.to_string_lossy()))
            .unwrap_or_default();
        destination = directory.join(format!(
            "{stem} ({}){extension}",
            Utc::now().format("%Y%m%d-%H%M%S")
        ));
    }
    std::fs::copy(source, &destination).with_context(|| {
        format!(
            "cannot copy {} to {}",
            source.display(),
            destination.display()
        )
    })?;
    Ok(destination)
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
        bail!(
            "this turn ended without a research report. Reply with \"write the report now\" to continue; the session and its evidence are intact"
        );
    }

    if output_files.is_empty() {
        bail!("this turn ended without a report in Session Outputs. Reply with \"write the report now\" to continue");
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
        bail!("the Research Library copy of this turn's report could not be made or verified");
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
    research::is_deliverable(path)
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

    /// The model wrote the report to Outputs and stopped. The closeout makes
    /// the archive copy itself, in the report's own topic folder, and reports
    /// it, so the turn completes without anyone re-prompting.
    #[test]
    fn closeout_archives_an_outputs_only_report_into_the_library() {
        let root = tempfile::tempdir().unwrap();
        let root_path = std::fs::canonicalize(root.path()).unwrap();
        let outputs = root_path.join("outputs");
        let library = root_path.join("library");
        fs::create_dir_all(outputs.join("retry-study")).unwrap();
        fs::create_dir_all(&library).unwrap();
        let report = outputs.join("retry-study").join("report.md");
        fs::write(&report, "final report").unwrap();
        let state = DeepResearchState {
            library_path: library.to_string_lossy().into_owned(),
            output_paths: vec![outputs.to_string_lossy().into_owned()],
        };

        let closeout = close_out_deliverables(
            &state,
            &[artifact(&report)],
            "",
            run_started_at(),
            &HashSet::new(),
            "Retry Study",
        )
        .unwrap();

        let copy = library.join("retry-study").join("report.md");
        assert_eq!(fs::read_to_string(&copy).unwrap(), "final report");
        assert_eq!(closeout.register, vec![copy.clone()]);
        assert_eq!(closeout.notes.len(), 1);
        assert!(closeout.notes[0].contains("Archived to Research Library"));

        let mut artifacts = vec![artifact(&report)];
        artifacts.push(artifact(&copy));
        verify_artifact_pairs(&state, &artifacts, "", run_started_at(), &HashSet::new()).unwrap();
    }

    /// A report written with a shell heredoc is not in the inventory; the
    /// closeout finds it because the final message names it, then archives it
    /// under a folder named after the session when it has no topic folder.
    #[test]
    fn closeout_finds_a_shell_written_report_by_its_mention_and_names_the_topic() {
        let root = tempfile::tempdir().unwrap();
        let root_path = std::fs::canonicalize(root.path()).unwrap();
        let outputs = root_path.join("outputs");
        let library = root_path.join("library");
        fs::create_dir_all(&outputs).unwrap();
        fs::create_dir_all(&library).unwrap();
        let report = outputs.join("report.md");
        fs::write(&report, "final report").unwrap();
        let state = DeepResearchState {
            library_path: library.to_string_lossy().into_owned(),
            output_paths: vec![outputs.to_string_lossy().into_owned()],
        };

        let closeout = close_out_deliverables(
            &state,
            &[],
            &format!("Saved the report to {}", report.display()),
            run_started_at(),
            &HashSet::new(),
            "Bounded Retry: A Comparison",
        )
        .unwrap();

        let copy = library.join("Bounded Retry A Comparison").join("report.md");
        assert!(copy.is_file(), "{closeout:?}");
        assert_eq!(
            closeout.register,
            vec![std::fs::canonicalize(&report).unwrap(), copy]
        );
    }

    #[test]
    fn closeout_copies_a_library_only_report_back_to_outputs_without_overwriting() {
        let root = tempfile::tempdir().unwrap();
        let root_path = std::fs::canonicalize(root.path()).unwrap();
        let outputs = root_path.join("outputs");
        let library = root_path.join("library");
        fs::create_dir_all(&outputs).unwrap();
        fs::create_dir_all(library.join("topic")).unwrap();
        let copy = library.join("topic").join("report.md");
        fs::write(&copy, "final report").unwrap();
        fs::create_dir_all(outputs.join("topic")).unwrap();
        fs::write(outputs.join("topic").join("report.md"), "an older draft").unwrap();
        let state = DeepResearchState {
            library_path: library.to_string_lossy().into_owned(),
            output_paths: vec![outputs.to_string_lossy().into_owned()],
        };

        let closeout = close_out_deliverables(
            &state,
            &[artifact(&copy)],
            "",
            run_started_at(),
            &HashSet::new(),
            "Topic",
        )
        .unwrap();

        assert_eq!(
            fs::read_to_string(outputs.join("topic").join("report.md")).unwrap(),
            "an older draft",
            "a different existing file must never be overwritten"
        );
        let dated = closeout
            .register
            .iter()
            .find(|path| path.starts_with(&outputs))
            .expect("a dated copy under Outputs");
        assert!(dated
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("report ("));
        assert_eq!(fs::read_to_string(dated).unwrap(), "final report");
        assert!(closeout.notes[0].contains("Copied to Session Outputs"));
    }

    #[test]
    fn closeout_leaves_a_complete_pair_alone() {
        let root = tempfile::tempdir().unwrap();
        let root_path = std::fs::canonicalize(root.path()).unwrap();
        let outputs = root_path.join("outputs");
        let library = root_path.join("library");
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

        let closeout = close_out_deliverables(
            &state,
            &[artifact(&output), artifact(&copy)],
            "",
            run_started_at(),
            &HashSet::new(),
            "Topic",
        )
        .unwrap();
        assert!(closeout.register.is_empty());
        assert!(closeout.notes.is_empty());
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
            .contains("Research Library copy of this turn's report could not be made or verified"));
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

        assert!(error
            .to_string()
            .contains("ended without a research report"));
    }
}
