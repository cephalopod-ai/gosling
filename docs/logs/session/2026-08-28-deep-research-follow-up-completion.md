# Deep Research follow-up completion repair

Date: 2026-08-28

## Task

Repair repeated Deep Research failures where a conversational follow-up was
rejected because its final response did not reference a newly created report,
even though the session already contained a verified report in Session Outputs
and the Research Library.

## Root cause and evidence

- The completion verifier scanned the session-wide artifact ledger after every
  Deep Research turn and unconditionally required report paths in the current
  final response.
- Session `20260828_51` already contained byte-identical Output and Research
  Library copies of
  `BO_for_Distributed_Sensing_and_Cooperative_Intercept_v8.md`. Both copies are
  70,698 bytes with SHA-256
  `83f73ec355f7d14d89cad037c9d6620d802a88ed608c283f1770feeca2504c03`.
- The failed 07:27 and 07:43 follow-ups were exploratory answers, not report
  production turns. The latter also exposed an assistant-message artifact
  false positive for the numeric sequence `0.50, 0.75, 0.88, 0.81…`.

## Changes

- Capture the start time and persisted assistant message IDs for each ACP
  prompt run.
- Scope completion verification to deliverable artifacts observed during that
  run, sourced from its assistant messages, or explicitly named in its final
  response.
- Allow follow-up turns that do not produce a current deliverable while
  the session already has a verified Output/Research Library pair. Initial
  research still must create a paired report. Configured-root containment,
  allowed deliverable types, artifact and byte limits, matching filenames, and
  byte-identical checks remain active whenever a current report is produced.
- Added a regression for a follow-up with a prior report and the observed
  numeric artifact false positive.

## Validation

- `cargo test -p gosling research_completion::tests -- --nocapture`: pass,
  7/7 after the final patch.
- `cargo clippy -p gosling --all-targets -- -D warnings`: pass.
- `cargo fmt --all` and `git diff --check`: pass.
- `just package-ui` under the Hermit Node 24.10.0/pnpm 10.30.3 toolchain:
  pass.
- Packaged code signature: pass. Packaged sidecar SHA-256 matches
  `target/release/gosling` at
  `28f33129762b642365e81f4f646757318f7c115eb46b2b65030cb7eceafe3e6b`.
- Installed `/Applications/Gosling.app` version 1.1.0 has the same sidecar hash.
  Its final relaunch is partially validated: process sampling shows macOS
  Keychain is awaiting authorization before the backend starts.

## Recovery and remaining live check

- Previous application backup:
  `/Users/eric/.local/share/gosling/install-backups/Gosling-before-deep-research-fix-20260828-075147.app`
- The local GUI-control service could not capture the app because macOS
  ScreenCaptureKit returned `-3811`. Installation and process startup are
  verified only up to the Keychain authorization gate; clicking that system
  dialog and observing a new follow-up turn remain partial live-UI validation,
  not claimed passes.
