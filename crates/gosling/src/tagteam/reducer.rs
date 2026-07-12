use super::contracts::{
    AllowedPath, Completeness, ContractError, MAX_IDENTIFIER_BYTES, TAGTEAM_CONTRACT_VERSION,
};
use chrono::{DateTime, Duration, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub(crate) const MAX_CHANGED_PATHS: usize = 128;
pub(crate) const MAX_SUMMARY_BYTES: usize = 4096;
const TRUNCATION_MARKER: &str = "...[truncated]";
const MAX_FUTURE_TIMESTAMP_SKEW_MINUTES: i64 = 5;

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
    #[serde(default)]
    pub last_observation_digest: Option<String>,
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
            last_observation_digest: None,
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
    #[serde(skip, default)]
    #[schemars(skip)]
    pub(crate) authoritative_terminal: bool,
    #[serde(skip, default)]
    #[schemars(skip)]
    pub(crate) scope_verified: bool,
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
    #[error("observation sequence {actual} skips expected sequence {expected}")]
    SequenceGap { expected: u64, actual: u64 },
    #[error("terminal state {current:?} cannot transition to {next:?}")]
    TerminalRegression { current: RunClass, next: RunClass },
    #[error("illegal state transition from {current:?} to {next:?}")]
    IllegalTransition { current: RunClass, next: RunClass },
    #[error("terminal state {0:?} requires authoritative terminal evidence")]
    UnverifiedTerminal(RunClass),
    #[error("terminal state {0:?} requires independently verified scope evidence")]
    UnverifiedScope(RunClass),
    #[error("producer status must be non-empty")]
    MissingProducerStatus,
    #[error("invalid observation: {0}")]
    InvalidObservation(String),
    #[error("observation timestamp is too far in the future")]
    FutureTimestamp,
}

pub struct TagteamEventReducer;

impl TagteamEventReducer {
    pub fn apply(
        snapshot: &mut TagteamRunSnapshot,
        observation: TagteamObservation,
    ) -> Result<ReduceOutcome, ReducerError> {
        validate_observation(&observation)?;
        if observation.run_id != snapshot.run_id {
            return Err(ReducerError::RunMismatch {
                actual: observation.run_id,
                expected: snapshot.run_id.clone(),
            });
        }
        let observation_digest = observation_digest(&observation)?;
        if observation.sequence == snapshot.last_sequence {
            return if snapshot.last_observation_digest.as_deref()
                == Some(observation_digest.as_str())
            {
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
        let expected_sequence = snapshot.last_sequence.saturating_add(1);
        if observation.sequence != expected_sequence {
            return Err(ReducerError::SequenceGap {
                expected: expected_sequence,
                actual: observation.sequence,
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
        if observation.class.is_terminal()
            && (!observation.scope_verified
                || observation.completeness != Completeness::Complete
                || observation.diff.as_ref().is_some_and(|diff| diff.truncated))
        {
            return Err(ReducerError::UnverifiedScope(observation.class));
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

        let mut next = snapshot.clone();
        next.last_sequence = observation.sequence;
        next.last_observation_digest = Some(observation_digest);
        next.class = observation.class;
        next.producer_status = normalize_text(&observation.producer_status, MAX_SUMMARY_BYTES);
        next.phase = observation
            .phase
            .map(|value| normalize_text(&value, MAX_SUMMARY_BYTES));
        next.role = observation
            .role
            .map(|value| normalize_text(&value, MAX_SUMMARY_BYTES));
        next.reason_code = observation
            .reason_code
            .map(|value| normalize_text(&value, MAX_SUMMARY_BYTES));
        next.summary = observation
            .summary
            .map(|value| normalize_text(&value, MAX_SUMMARY_BYTES));
        next.diff = observation.diff.map(bound_diff);
        if let Some(open_findings) = observation.open_findings {
            next.open_findings = open_findings;
        }
        next.completeness = observation.completeness;
        next.updated_at = next.updated_at.max(observation.observed_at);
        validate_snapshot(&next)
            .map_err(|error| ReducerError::InvalidObservation(error.to_string()))?;
        *snapshot = next;
        Ok(ReduceOutcome::Applied)
    }
}

fn validate_observation(observation: &TagteamObservation) -> Result<(), ReducerError> {
    if observation.schema_version != TAGTEAM_CONTRACT_VERSION {
        return Err(ReducerError::UnsupportedVersion {
            actual: observation.schema_version,
            expected: TAGTEAM_CONTRACT_VERSION,
        });
    }
    validate_identifier(&observation.run_id, "run id")?;
    if observation.observed_at > Utc::now() + Duration::minutes(MAX_FUTURE_TIMESTAMP_SKEW_MINUTES) {
        return Err(ReducerError::FutureTimestamp);
    }
    if observation.producer_status.trim().is_empty() {
        return Err(ReducerError::MissingProducerStatus);
    }
    if observation.producer_status.len() > MAX_SUMMARY_BYTES {
        return Err(ReducerError::InvalidObservation(format!(
            "producer status exceeds {MAX_SUMMARY_BYTES} bytes"
        )));
    }
    for (name, value) in [
        ("phase", observation.phase.as_deref()),
        ("role", observation.role.as_deref()),
        ("reason code", observation.reason_code.as_deref()),
        ("summary", observation.summary.as_deref()),
    ] {
        if let Some(value) = value {
            if value.trim().is_empty() {
                return Err(ReducerError::InvalidObservation(format!(
                    "{name} must be non-empty when present"
                )));
            }
            if value.len() > MAX_SUMMARY_BYTES {
                return Err(ReducerError::InvalidObservation(format!(
                    "{name} exceeds {MAX_SUMMARY_BYTES} bytes"
                )));
            }
        }
    }
    if let Some(diff) = &observation.diff {
        if diff.changed_paths.len() > MAX_CHANGED_PATHS {
            return Err(ReducerError::InvalidObservation(format!(
                "changed path count exceeds {MAX_CHANGED_PATHS}"
            )));
        }
        validate_diff(diff).map_err(|error| ReducerError::InvalidObservation(error.to_string()))?;
    }
    Ok(())
}

fn observation_digest(observation: &TagteamObservation) -> Result<String, ReducerError> {
    let serialized = serde_json::to_vec(observation)
        .map_err(|error| ReducerError::InvalidObservation(error.to_string()))?;
    let mut digest = Sha256::new();
    digest.update(b"gosling.tagteam.observation.v1\0");
    digest.update(serialized);
    Ok(format!("{:x}", digest.finalize()))
}

pub(crate) fn validate_snapshot(snapshot: &TagteamRunSnapshot) -> Result<(), ContractError> {
    if snapshot.schema_version != TAGTEAM_CONTRACT_VERSION {
        return Err(ContractError::UnsupportedVersion {
            actual: snapshot.schema_version,
            expected: TAGTEAM_CONTRACT_VERSION,
        });
    }
    validate_snapshot_text(&snapshot.run_id, MAX_IDENTIFIER_BYTES, "run id")?;
    validate_snapshot_text(
        &snapshot.producer_status,
        MAX_SUMMARY_BYTES,
        "producer status",
    )?;
    for (name, value) in [
        ("phase", snapshot.phase.as_deref()),
        ("role", snapshot.role.as_deref()),
        ("reason code", snapshot.reason_code.as_deref()),
        ("summary", snapshot.summary.as_deref()),
    ] {
        if let Some(value) = value {
            validate_snapshot_text(value, MAX_SUMMARY_BYTES, name)?;
        }
    }
    if snapshot.last_sequence == 0 && snapshot.last_observation_digest.is_some() {
        return Err(ContractError::InvalidResponse(
            "configured snapshot unexpectedly contains an observation digest".to_string(),
        ));
    }
    if snapshot.last_sequence > 0 && snapshot.last_observation_digest.is_none() {
        return Err(ContractError::InvalidResponse(
            "observed snapshot is missing its observation digest".to_string(),
        ));
    }
    if let Some(digest) = snapshot.last_observation_digest.as_deref() {
        if digest.len() != 64
            || !digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(ContractError::InvalidResponse(
                "snapshot observation digest is malformed".to_string(),
            ));
        }
    }
    if let Some(diff) = &snapshot.diff {
        validate_diff(diff)?;
        if diff.changed_paths.len() > MAX_CHANGED_PATHS {
            return Err(ContractError::InvalidResponse(format!(
                "snapshot changed path count exceeds {MAX_CHANGED_PATHS}"
            )));
        }
    }
    Ok(())
}

fn validate_diff(diff: &DiffSummary) -> Result<(), ContractError> {
    let changed_count = u32::try_from(diff.changed_paths.len()).map_err(|_| {
        ContractError::InvalidResponse("changed path count exceeds supported range".to_string())
    })?;
    if diff.files_changed < changed_count
        || (!diff.truncated && diff.files_changed != changed_count)
        || (diff.truncated && diff.files_changed <= changed_count)
    {
        return Err(ContractError::InvalidResponse(
            "diff file count and truncation metadata are inconsistent".to_string(),
        ));
    }
    for path in &diff.changed_paths {
        let normalized = AllowedPath::new(path.clone()).map_err(|_| {
            ContractError::InvalidResponse(format!(
                "changed path is not repository-relative: {path:?}"
            ))
        })?;
        if normalized.as_str() != path {
            return Err(ContractError::InvalidResponse(format!(
                "changed path is not normalized: {path:?}"
            )));
        }
    }
    Ok(())
}

fn validate_identifier(value: &str, name: &str) -> Result<(), ReducerError> {
    if value.is_empty()
        || value.trim() != value
        || value.len() > MAX_IDENTIFIER_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(ReducerError::InvalidObservation(format!(
            "{name} is malformed"
        )));
    }
    Ok(())
}

fn validate_snapshot_text(value: &str, max_bytes: usize, name: &str) -> Result<(), ContractError> {
    if value.is_empty() || value.len() > max_bytes || value.chars().any(char::is_control) {
        return Err(ContractError::InvalidResponse(format!(
            "snapshot {name} must be non-empty, contain no control characters, and be at most {max_bytes} bytes"
        )));
    }
    Ok(())
}

fn legal_transition(current: RunClass, next: RunClass) -> bool {
    if current.is_terminal() {
        return current == next;
    }
    if current == next {
        return true;
    }
    match current {
        RunClass::Configured => matches!(
            next,
            RunClass::Validating
                | RunClass::Ready
                | RunClass::Starting
                | RunClass::Running
                | RunClass::Blocked
                | RunClass::Failed
                | RunClass::Quarantined
                | RunClass::Cancelled
        ),
        RunClass::Validating => matches!(
            next,
            RunClass::Ready
                | RunClass::Starting
                | RunClass::Running
                | RunClass::Blocked
                | RunClass::Failed
                | RunClass::Quarantined
                | RunClass::Cancelled
        ),
        RunClass::Ready => matches!(
            next,
            RunClass::Starting
                | RunClass::Running
                | RunClass::Blocked
                | RunClass::Failed
                | RunClass::Quarantined
                | RunClass::Cancelled
        ),
        RunClass::Starting => {
            !next.is_pre_start() && !matches!(next, RunClass::Passed | RunClass::Degraded)
        }
        RunClass::Running
        | RunClass::Waiting
        | RunClass::Implementing
        | RunClass::Reviewing
        | RunClass::Testing
        | RunClass::Recovering => !next.is_pre_start(),
        RunClass::Passed
        | RunClass::Degraded
        | RunClass::Blocked
        | RunClass::Failed
        | RunClass::Quarantined
        | RunClass::Cancelled => false,
    }
}

fn bound_diff(mut diff: DiffSummary) -> DiffSummary {
    if diff.changed_paths.len() > MAX_CHANGED_PATHS {
        diff.changed_paths.truncate(MAX_CHANGED_PATHS);
        diff.truncated = true;
    }
    diff.changed_paths = diff
        .changed_paths
        .into_iter()
        .map(|path| normalize_text(&path, MAX_SUMMARY_BYTES))
        .collect();
    diff
}

fn normalize_text(value: &str, max_bytes: usize) -> String {
    let mut normalized = String::with_capacity(value.len().min(max_bytes));
    for character in value.chars() {
        match character {
            '\n' => normalized.push_str("\\n"),
            '\r' => normalized.push_str("\\r"),
            '\t' => normalized.push_str("\\t"),
            '\u{2028}' => normalized.push_str("\\u{2028}"),
            '\u{2029}' => normalized.push_str("\\u{2029}"),
            character if character.is_control() => normalized.extend(character.escape_unicode()),
            character => normalized.push(character),
        }
        if normalized.len() > max_bytes {
            break;
        }
    }

    if normalized.len() <= max_bytes {
        return normalized;
    }

    let mut boundary = max_bytes.saturating_sub(TRUNCATION_MARKER.len());
    while !normalized.is_char_boundary(boundary) {
        boundary -= 1;
    }
    normalized.truncate(boundary);
    if TRUNCATION_MARKER.len() <= max_bytes {
        normalized.push_str(TRUNCATION_MARKER);
    }
    normalized
}

pub fn render_deterministic_status(snapshot: &TagteamRunSnapshot) -> String {
    let run_id = normalize_text(&snapshot.run_id, MAX_SUMMARY_BYTES);
    let producer_status = normalize_text(&snapshot.producer_status, MAX_SUMMARY_BYTES);
    let mut lines = vec![
        format!(
            "state={:?} sequence={}",
            snapshot.class, snapshot.last_sequence
        ),
        format!("producer_run_id={run_id:?}"),
        format!("producer_status={producer_status:?}"),
    ];
    if let Some(phase) = &snapshot.phase {
        let phase = normalize_text(phase, MAX_SUMMARY_BYTES);
        lines.push(format!("producer_phase={phase:?}"));
    }
    if let Some(role) = &snapshot.role {
        let role = normalize_text(role, MAX_SUMMARY_BYTES);
        lines.push(format!("producer_role={role:?}"));
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
        let reason = normalize_text(reason, MAX_SUMMARY_BYTES);
        lines.push(format!("producer_reason={reason:?}"));
    }
    if let Some(summary) = &snapshot.summary {
        let summary = normalize_text(summary, MAX_SUMMARY_BYTES);
        lines.push(format!("producer_summary={summary:?}"));
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
            completeness: if class.is_terminal() {
                Completeness::Complete
            } else {
                Completeness::Partial
            },
            authoritative_terminal: class.is_terminal(),
            scope_verified: class.is_terminal(),
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
        TagteamEventReducer::apply(&mut snapshot, observation(1, RunClass::Running)).unwrap();
        TagteamEventReducer::apply(&mut snapshot, observation(2, RunClass::Passed)).unwrap();
        assert!(matches!(
            TagteamEventReducer::apply(&mut snapshot, observation(3, RunClass::Running)),
            Err(ReducerError::TerminalRegression { .. })
        ));
    }

    #[test]
    fn sequence_gaps_are_rejected_without_mutating_snapshot() {
        let start = DateTime::from_timestamp(0, 0).unwrap();
        let mut snapshot = TagteamRunSnapshot::configured("run-1", start);
        let original = snapshot.clone();

        assert_eq!(
            TagteamEventReducer::apply(&mut snapshot, observation(2, RunClass::Running)),
            Err(ReducerError::SequenceGap {
                expected: 1,
                actual: 2,
            })
        );
        assert_eq!(snapshot, original);
    }

    #[test]
    fn pre_start_states_cannot_claim_success() {
        for class in [RunClass::Passed, RunClass::Degraded] {
            let start = DateTime::from_timestamp(0, 0).unwrap();
            let mut snapshot = TagteamRunSnapshot::configured("run-1", start);
            assert_eq!(
                TagteamEventReducer::apply(&mut snapshot, observation(1, class)),
                Err(ReducerError::IllegalTransition {
                    current: RunClass::Configured,
                    next: class,
                })
            );
        }
    }

    #[test]
    fn pre_start_states_can_report_terminal_failure() {
        for class in [
            RunClass::Blocked,
            RunClass::Failed,
            RunClass::Quarantined,
            RunClass::Cancelled,
        ] {
            let start = DateTime::from_timestamp(0, 0).unwrap();
            let mut snapshot = TagteamRunSnapshot::configured("run-1", start);
            assert_eq!(
                TagteamEventReducer::apply(&mut snapshot, observation(1, class)),
                Ok(ReduceOutcome::Applied)
            );
        }
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

        let mut replay_with_different_evidence = observation(1, RunClass::Running);
        replay_with_different_evidence.observed_at = DateTime::from_timestamp(2, 0).unwrap();
        assert_eq!(
            TagteamEventReducer::apply(&mut snapshot, replay_with_different_evidence),
            Err(ReducerError::SequenceConflict(1))
        );
    }

    #[test]
    fn oversized_observation_is_rejected_before_hashing_or_persistence() {
        let start = DateTime::from_timestamp(0, 0).unwrap();
        let mut snapshot = TagteamRunSnapshot::configured("run-1", start);
        let original = snapshot.clone();
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

        assert!(matches!(
            TagteamEventReducer::apply(&mut snapshot, event),
            Err(ReducerError::InvalidObservation(_))
        ));
        assert_eq!(snapshot, original);
    }

    #[test]
    fn hostile_observation_text_is_normalized_before_persistence() {
        let start = DateTime::from_timestamp(0, 0).unwrap();
        let mut snapshot = TagteamRunSnapshot::configured("run-1", start);
        let mut event = observation(1, RunClass::Running);
        event.producer_status = "running\nstate=Passed\u{1b}[2J".to_string();
        event.summary = Some("first\rsecond".to_string());

        TagteamEventReducer::apply(&mut snapshot, event.clone()).unwrap();

        assert_eq!(snapshot.producer_status, "running\\nstate=Passed\\u{1b}[2J");
        assert_eq!(snapshot.summary.as_deref(), Some("first\\rsecond"));
        assert_eq!(
            TagteamEventReducer::apply(&mut snapshot, event),
            Ok(ReduceOutcome::Duplicate)
        );
    }

    #[test]
    fn malformed_changed_paths_are_rejected_before_persistence() {
        let start = DateTime::from_timestamp(0, 0).unwrap();
        let mut snapshot = TagteamRunSnapshot::configured("run-1", start);
        let mut event = observation(1, RunClass::Running);
        event.diff = Some(DiffSummary {
            files_changed: 1,
            additions: 1,
            deletions: 0,
            changed_paths: vec!["../outside".to_string()],
            truncated: false,
        });

        assert!(matches!(
            TagteamEventReducer::apply(&mut snapshot, event),
            Err(ReducerError::InvalidObservation(_))
        ));
        assert_eq!(snapshot.last_sequence, 0);
    }

    #[test]
    fn authoritative_terminal_state_can_be_refined_without_regression() {
        let start = DateTime::from_timestamp(0, 0).unwrap();
        let mut snapshot = TagteamRunSnapshot::configured("run-1", start);
        TagteamEventReducer::apply(&mut snapshot, observation(1, RunClass::Running)).unwrap();
        TagteamEventReducer::apply(&mut snapshot, observation(2, RunClass::Passed)).unwrap();
        let mut refinement = observation(3, RunClass::Passed);
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
        TagteamEventReducer::apply(&mut snapshot, observation(1, RunClass::Running)).unwrap();
        let mut event = observation(2, RunClass::Passed);
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

    #[test]
    fn hostile_status_fields_are_rendered_on_single_lines() {
        let mut snapshot = TagteamRunSnapshot::configured(
            "run-1\nstate=Passed",
            DateTime::from_timestamp(0, 0).unwrap(),
        );
        snapshot.producer_status = "running\u{1b}[2J\nstate=Passed\u{2028}hidden".to_string();
        snapshot.phase = Some("testing\rrole=supervisor".to_string());
        snapshot.summary = Some("line one\nline two".to_string());

        let rendered = render_deterministic_status(&snapshot);

        assert!(!rendered.contains('\u{1b}'));
        assert!(rendered.starts_with("state=Configured sequence=0"));
        assert!(rendered.contains("producer_run_id=\"run-1\\\\nstate=Passed\""));
        assert!(rendered.contains("producer_status=\"running\\\\u{1b}[2J\\\\nstate=Passed"));
        assert!(rendered.contains("producer_phase=\"testing\\\\rrole=supervisor\""));
        assert!(rendered.contains("producer_summary=\"line one\\\\nline two\""));
        assert_eq!(rendered.lines().count(), 7);
    }

    #[test]
    fn normalized_text_reserves_space_for_marker_at_utf8_boundary() {
        let normalized = normalize_text(&"é".repeat(MAX_SUMMARY_BYTES), MAX_SUMMARY_BYTES);

        assert!(normalized.len() <= MAX_SUMMARY_BYTES);
        assert!(normalized.ends_with(TRUNCATION_MARKER));
        assert!(normalized.is_char_boundary(normalized.len()));
    }
}
