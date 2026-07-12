use super::contracts::{Completeness, TAGTEAM_CONTRACT_VERSION};
use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const MAX_CHANGED_PATHS: usize = 128;
const MAX_SUMMARY_BYTES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RunClass {
    Configured,
    Validating,
    Ready,
    Starting,
    Running,
    Waiting,
    Implementing,
    Reviewing,
    Testing,
    Recovering,
    Passed,
    Degraded,
    Blocked,
    Failed,
    Quarantined,
    Cancelled,
}

impl RunClass {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Passed
                | Self::Degraded
                | Self::Blocked
                | Self::Failed
                | Self::Quarantined
                | Self::Cancelled
        )
    }

    fn is_pre_start(self) -> bool {
        matches!(
            self,
            Self::Configured | Self::Validating | Self::Ready | Self::Starting
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DiffSummary {
    pub files_changed: u32,
    pub additions: u32,
    pub deletions: u32,
    pub changed_paths: Vec<String>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TagteamRunSnapshot {
    pub schema_version: u32,
    pub run_id: String,
    pub last_sequence: u64,
    pub class: RunClass,
    pub producer_status: String,
    pub phase: Option<String>,
    pub role: Option<String>,
    pub reason_code: Option<String>,
    pub summary: Option<String>,
    pub diff: Option<DiffSummary>,
    pub open_findings: u32,
    pub completeness: Completeness,
    pub updated_at: DateTime<Utc>,
}

impl TagteamRunSnapshot {
    pub fn configured(run_id: impl Into<String>, updated_at: DateTime<Utc>) -> Self {
        Self {
            schema_version: TAGTEAM_CONTRACT_VERSION,
            run_id: run_id.into(),
            last_sequence: 0,
            class: RunClass::Configured,
            producer_status: "configured".to_string(),
            phase: None,
            role: None,
            reason_code: None,
            summary: None,
            diff: None,
            open_findings: 0,
            completeness: Completeness::Partial,
            updated_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TagteamObservation {
    pub schema_version: u32,
    pub run_id: String,
    pub sequence: u64,
    pub observed_at: DateTime<Utc>,
    pub class: RunClass,
    pub producer_status: String,
    pub phase: Option<String>,
    pub role: Option<String>,
    pub reason_code: Option<String>,
    pub summary: Option<String>,
    pub diff: Option<DiffSummary>,
    pub open_findings: Option<u32>,
    pub completeness: Completeness,
    pub authoritative_terminal: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReduceOutcome {
    Applied,
    Duplicate,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ReducerError {
    #[error("observation schema version {actual} is unsupported; expected {expected}")]
    UnsupportedVersion { actual: u32, expected: u32 },
    #[error("observation run {actual:?} does not match snapshot run {expected:?}")]
    RunMismatch { actual: String, expected: String },
    #[error("observation sequence {actual} is older than applied sequence {current}")]
    StaleSequence { actual: u64, current: u64 },
    #[error("observation sequence {0} reuses the current sequence with different content")]
    SequenceConflict(u64),
    #[error("terminal state {current:?} cannot transition to {next:?}")]
    TerminalRegression { current: RunClass, next: RunClass },
    #[error("illegal state transition from {current:?} to {next:?}")]
    IllegalTransition { current: RunClass, next: RunClass },
    #[error("terminal state {0:?} requires authoritative terminal evidence")]
    UnverifiedTerminal(RunClass),
    #[error("producer status must be non-empty")]
    MissingProducerStatus,
}

pub struct TagteamEventReducer;

impl TagteamEventReducer {
    pub fn apply(
        snapshot: &mut TagteamRunSnapshot,
        observation: TagteamObservation,
    ) -> Result<ReduceOutcome, ReducerError> {
        if observation.schema_version != TAGTEAM_CONTRACT_VERSION {
            return Err(ReducerError::UnsupportedVersion {
                actual: observation.schema_version,
                expected: TAGTEAM_CONTRACT_VERSION,
            });
        }
        if observation.run_id != snapshot.run_id {
            return Err(ReducerError::RunMismatch {
                actual: observation.run_id,
                expected: snapshot.run_id.clone(),
            });
        }
        if observation.sequence == snapshot.last_sequence {
            return if observation_matches_snapshot(snapshot, &observation) {
                Ok(ReduceOutcome::Duplicate)
            } else {
                Err(ReducerError::SequenceConflict(observation.sequence))
            };
        }
        if observation.sequence < snapshot.last_sequence {
            return Err(ReducerError::StaleSequence {
                actual: observation.sequence,
                current: snapshot.last_sequence,
            });
        }
        if snapshot.class.is_terminal() && snapshot.class != observation.class {
            return Err(ReducerError::TerminalRegression {
                current: snapshot.class,
                next: observation.class,
            });
        }
        if observation.class.is_terminal() && !observation.authoritative_terminal {
            return Err(ReducerError::UnverifiedTerminal(observation.class));
        }
        if observation.producer_status.trim().is_empty() {
            return Err(ReducerError::MissingProducerStatus);
        }
        if !legal_transition(snapshot.class, observation.class) {
            return Err(ReducerError::IllegalTransition {
                current: snapshot.class,
                next: observation.class,
            });
        }

        snapshot.last_sequence = observation.sequence;
        snapshot.class = observation.class;
        snapshot.producer_status = truncate(observation.producer_status, MAX_SUMMARY_BYTES);
        snapshot.phase = observation
            .phase
            .map(|value| truncate(value, MAX_SUMMARY_BYTES));
        snapshot.role = observation
            .role
            .map(|value| truncate(value, MAX_SUMMARY_BYTES));
        snapshot.reason_code = observation
            .reason_code
            .map(|value| truncate(value, MAX_SUMMARY_BYTES));
        snapshot.summary = observation
            .summary
            .map(|value| truncate(value, MAX_SUMMARY_BYTES));
        snapshot.diff = observation.diff.map(bound_diff);
        if let Some(open_findings) = observation.open_findings {
            snapshot.open_findings = open_findings;
        }
        snapshot.completeness = observation.completeness;
        snapshot.updated_at = observation.observed_at;
        Ok(ReduceOutcome::Applied)
    }
}

fn legal_transition(current: RunClass, next: RunClass) -> bool {
    if next.is_terminal() {
        return true;
    }
    if current == next {
        return true;
    }
    match current {
        RunClass::Configured => matches!(
            next,
            RunClass::Validating | RunClass::Ready | RunClass::Starting | RunClass::Running
        ),
        RunClass::Validating => {
            matches!(
                next,
                RunClass::Ready | RunClass::Starting | RunClass::Running
            )
        }
        RunClass::Ready => matches!(next, RunClass::Starting | RunClass::Running),
        RunClass::Starting => !next.is_pre_start(),
        _ => !next.is_pre_start(),
    }
}

fn observation_matches_snapshot(
    snapshot: &TagteamRunSnapshot,
    observation: &TagteamObservation,
) -> bool {
    snapshot.class == observation.class
        && snapshot.producer_status
            == truncate(observation.producer_status.clone(), MAX_SUMMARY_BYTES)
        && snapshot.phase
            == observation
                .phase
                .clone()
                .map(|value| truncate(value, MAX_SUMMARY_BYTES))
        && snapshot.role
            == observation
                .role
                .clone()
                .map(|value| truncate(value, MAX_SUMMARY_BYTES))
        && snapshot.reason_code
            == observation
                .reason_code
                .clone()
                .map(|value| truncate(value, MAX_SUMMARY_BYTES))
        && snapshot.summary
            == observation
                .summary
                .clone()
                .map(|value| truncate(value, MAX_SUMMARY_BYTES))
        && snapshot.diff == observation.diff.clone().map(bound_diff)
        && observation
            .open_findings
            .is_none_or(|count| count == snapshot.open_findings)
        && snapshot.completeness == observation.completeness
}

fn bound_diff(mut diff: DiffSummary) -> DiffSummary {
    if diff.changed_paths.len() > MAX_CHANGED_PATHS {
        diff.changed_paths.truncate(MAX_CHANGED_PATHS);
        diff.truncated = true;
    }
    diff.changed_paths = diff
        .changed_paths
        .into_iter()
        .map(|path| truncate(path, MAX_SUMMARY_BYTES))
        .collect();
    diff
}

fn truncate(mut value: String, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value;
    }
    let mut boundary = max_bytes;
    while !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    value.truncate(boundary);
    value.push_str("...[truncated]");
    value
}

pub fn render_deterministic_status(snapshot: &TagteamRunSnapshot) -> String {
    let mut lines = vec![
        format!(
            "run={} status={}",
            snapshot.run_id, snapshot.producer_status
        ),
        format!(
            "state={:?} sequence={}",
            snapshot.class, snapshot.last_sequence
        ),
    ];
    if let Some(phase) = &snapshot.phase {
        lines.push(format!("phase={phase}"));
    }
    if let Some(role) = &snapshot.role {
        lines.push(format!("role={role}"));
    }
    if let Some(diff) = &snapshot.diff {
        lines.push(format!(
            "diff=files:{} additions:{} deletions:{}{}",
            diff.files_changed,
            diff.additions,
            diff.deletions,
            if diff.truncated { " truncated" } else { "" }
        ));
    }
    lines.push(format!("open_findings={}", snapshot.open_findings));
    if let Some(reason) = &snapshot.reason_code {
        lines.push(format!("reason={reason}"));
    }
    if let Some(summary) = &snapshot.summary {
        lines.push(format!("summary={summary}"));
    }
    if snapshot.completeness == Completeness::Partial {
        lines.push("completeness=partial; do not infer missing state".to_string());
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(sequence: u64, class: RunClass) -> TagteamObservation {
        TagteamObservation {
            schema_version: TAGTEAM_CONTRACT_VERSION,
            run_id: "run-1".to_string(),
            sequence,
            observed_at: DateTime::from_timestamp(sequence as i64, 0).unwrap(),
            class,
            producer_status: format!("{class:?}").to_lowercase(),
            phase: None,
            role: None,
            reason_code: None,
            summary: None,
            diff: None,
            open_findings: None,
            completeness: Completeness::Partial,
            authoritative_terminal: class.is_terminal(),
        }
    }

    #[test]
    fn replay_is_deterministic_and_duplicate_safe() {
        let start = DateTime::from_timestamp(0, 0).unwrap();
        let mut first = TagteamRunSnapshot::configured("run-1", start);
        let mut second = first.clone();
        for event in [
            observation(1, RunClass::Validating),
            observation(2, RunClass::Running),
            observation(3, RunClass::Passed),
        ] {
            TagteamEventReducer::apply(&mut first, event.clone()).unwrap();
            TagteamEventReducer::apply(&mut second, event).unwrap();
        }
        assert_eq!(first, second);
        assert_eq!(
            render_deterministic_status(&first),
            render_deterministic_status(&second)
        );
        assert_eq!(
            TagteamEventReducer::apply(&mut first, observation(3, RunClass::Passed)),
            Ok(ReduceOutcome::Duplicate)
        );
    }

    #[test]
    fn terminal_state_cannot_regress() {
        let start = DateTime::from_timestamp(0, 0).unwrap();
        let mut snapshot = TagteamRunSnapshot::configured("run-1", start);
        TagteamEventReducer::apply(&mut snapshot, observation(1, RunClass::Passed)).unwrap();
        assert!(matches!(
            TagteamEventReducer::apply(&mut snapshot, observation(2, RunClass::Running)),
            Err(ReducerError::TerminalRegression { .. })
        ));
    }

    #[test]
    fn duplicate_sequence_with_different_content_is_rejected() {
        let start = DateTime::from_timestamp(0, 0).unwrap();
        let mut snapshot = TagteamRunSnapshot::configured("run-1", start);
        TagteamEventReducer::apply(&mut snapshot, observation(1, RunClass::Running)).unwrap();
        let mut conflicting = observation(1, RunClass::Running);
        conflicting.summary = Some("different payload".to_string());
        assert_eq!(
            TagteamEventReducer::apply(&mut snapshot, conflicting),
            Err(ReducerError::SequenceConflict(1))
        );
    }

    #[test]
    fn bounded_observation_replay_is_duplicate_safe() {
        let start = DateTime::from_timestamp(0, 0).unwrap();
        let mut snapshot = TagteamRunSnapshot::configured("run-1", start);
        let mut event = observation(1, RunClass::Running);
        event.producer_status = "s".repeat(MAX_SUMMARY_BYTES + 1);
        event.summary = Some("x".repeat(MAX_SUMMARY_BYTES + 1));
        event.diff = Some(DiffSummary {
            files_changed: (MAX_CHANGED_PATHS + 1) as u32,
            additions: 1,
            deletions: 0,
            changed_paths: (0..=MAX_CHANGED_PATHS)
                .map(|index| format!("src/file-{index}.rs"))
                .collect(),
            truncated: false,
        });

        TagteamEventReducer::apply(&mut snapshot, event.clone()).unwrap();

        assert_eq!(
            TagteamEventReducer::apply(&mut snapshot, event),
            Ok(ReduceOutcome::Duplicate)
        );
        assert!(snapshot.diff.as_ref().unwrap().truncated);
    }

    #[test]
    fn authoritative_terminal_state_can_be_refined_without_regression() {
        let start = DateTime::from_timestamp(0, 0).unwrap();
        let mut snapshot = TagteamRunSnapshot::configured("run-1", start);
        TagteamEventReducer::apply(&mut snapshot, observation(1, RunClass::Passed)).unwrap();
        let mut refinement = observation(2, RunClass::Passed);
        refinement.summary = Some("final artifact persisted".to_string());
        refinement.completeness = Completeness::Complete;
        assert_eq!(
            TagteamEventReducer::apply(&mut snapshot, refinement),
            Ok(ReduceOutcome::Applied)
        );
        assert_eq!(
            snapshot.summary.as_deref(),
            Some("final artifact persisted")
        );
    }

    #[test]
    fn unverified_success_is_rejected() {
        let start = DateTime::from_timestamp(0, 0).unwrap();
        let mut snapshot = TagteamRunSnapshot::configured("run-1", start);
        let mut event = observation(1, RunClass::Passed);
        event.authoritative_terminal = false;
        assert_eq!(
            TagteamEventReducer::apply(&mut snapshot, event),
            Err(ReducerError::UnverifiedTerminal(RunClass::Passed))
        );
    }

    #[test]
    fn report_marks_partial_evidence() {
        let snapshot =
            TagteamRunSnapshot::configured("run-1", DateTime::from_timestamp(0, 0).unwrap());
        assert!(render_deterministic_status(&snapshot).contains("do not infer missing state"));
    }
}
