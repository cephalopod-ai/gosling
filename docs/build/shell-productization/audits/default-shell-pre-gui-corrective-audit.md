# Default Shell pre-GUI corrective audit

Date: 2026-08-15

Scope: generic Default Shell nonvisual contracts only

Base revision: `1e608283b`; corrective candidate: the revision containing this audit

Decision: **local corrective implementation PASS; DS-7 remains NO-GO for GUI implementation**

## Audit question

Can the generic Default Shell safely support a reduced Gosling/superset-of-workspace-chat GUI
without inventing renderer authority, losing recoverable work, exposing credentials or private
reasoning, or stranding a user in an interaction that the consumer cannot answer?

The audit uses the current repository contracts and executable workflow as authority. Muninn recall
did not produce an authoritative Default Shell usage transcript, so low-trust named-shell artifacts
were not used to widen requirements. DAWES, math, Physics/CST, and every other named shell remain out
of scope.

## Corrective findings and disposition

| Area                 | Prior gap                                                                                               | Corrective result                                                                                                                                                                                                                            |
| -------------------- | ------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Capability closure   | Prompt and domain actions could be declared without their required response channels.                   | Consumer prerequisites and runtime callbacks fail closed unless permission, elicitation, or confirmation responses are declared.                                                                                                             |
| Resume               | ACP history delivered during `loadSession` was discarded while the controller was `resuming`.           | Resume is session-fenced, buffers bounded history, activates the session, then emits explicit `history` updates.                                                                                                                             |
| Discovery and repair | Resume required a caller-known ID and event gaps had no bounded repair path.                            | Standard ACP session listing is limited to 20 active ACP summaries for the accepted directory; main independently filters returned directories. The active-session ledger is limited to 256 updates/48 KiB and reports integrity/truncation. |
| Resume isolation     | A renderer-known session ID could bypass the filtered list and load a session from another directory.   | Main supplies its accepted canonical directory to resume, and the ACP transport rejects mismatched server metadata before `loadSession`. The renderer receives only a safe `SESSION_UNAVAILABLE` recovery.                                   |
| Backend activation   | Direct ACP load could relocate a shell session or retain tools removed by current provisioning.         | Rust requires the requested canonical directory to equal the stored shell-session directory, revalidates provisioning, and rebuilds current extension/skill selection before activation.                                                     |
| User recovery        | Prompt and invoke failures were arbitrary strings with no reliable recovery semantics.                  | `ShellOperationFailure` exposes only a stable code, safe message, retry flag, recovery action, and draft-preservation fact.                                                                                                                  |
| Interaction fidelity | Permission/form summaries omitted decision context; confirmation pending state disagreed between paths. | Main projects bounded tool effect, basename targets, safe field names, supported form schemas, and a safe confirmation mirror. Schema-invalid or secret-shaped forms cancel.                                                                 |
| Privacy              | Agent thought chunks were renderer conversation content.                                                | Private reasoning updates are discarded at the main projection boundary.                                                                                                                                                                     |
| Context              | Session state omitted working directory, title, provider, and model facts.                              | Safe bounded session context is projected for create, resume, and discovery.                                                                                                                                                                 |
| Resource bounds      | Interaction replay IDs had no process-lifetime ceiling.                                                 | New interactions fail closed after 4,096 issued IDs until runtime restart.                                                                                                                                                                   |
| Shutdown lifecycle   | A final event sent after renderer destruction aborted cleanup and left the packaged backend orphaned.   | Event publication drops destroyed/check-send-race deliveries; normal close now completes backend cleanup.                                                                                                                                    |
| Restart cleanup      | A dead child record from failed shutdown survived the next successful shell run.                        | The generic host reconciles its product-local process registry before spawn and fails closed if reconciliation cannot complete.                                                                                                              |
| Test isolation       | Full-workspace acceptance could block while reading the operator's real macOS Keychain.                 | Unit-test `Config` instances always use file-backed secrets. The exact-final workspace passes with the keyring feature enabled and no disable override.                                                                                      |
| Credential lifecycle | A timed-out protected-store lookup could survive and force normal packaged shutdown to `SIGKILL`.       | Electron-managed backends use noninteractive protected-store reads and allow bounded ACP close settling before child termination. A live package exits code 0.                                                                               |
| Interaction lifetime | Streamed tool progress cleared a pending permission or form as though the prompt had terminated.        | Pending interactions now survive progress and clear only on completed, cancelled, or failed session outcomes.                                                                                                                                |

The durable Outputs source of truth already exists under ADR-0013. A later GUI must consume that
inventory and its Electron artifact guard; it must not add a directory scan or an alternate output
authority.

## Requirement-by-requirement re-audit

| Requirement                                                     | Result                                | Evidence or residual                                                                                                                                            |
| --------------------------------------------------------------- | ------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Shell-owned instructions; no generic Gosling prompt inheritance | Pass                                  | Existing DS-1 contract and tests remain unchanged.                                                                                                              |
| Credentials usable but not owned                                | Pass with provider follow-up          | Safe metadata/opaque references remain unchanged. Gemini OAuth currently reports `Internal error`; this is recorded only and was not investigated.              |
| Per-shell local settings only                                   | Pass                                  | Existing strict product-local settings service and narrow IPC remain unchanged.                                                                                 |
| Native working-directory choice                                 | Pass                                  | Discovery, create, and resume remain fenced to the accepted canonical directory in both Electron main and Rust activation.                                      |
| No developer tools by default                                   | Pass                                  | Default consumer capability closure adds only shell workflow operations; current provisioning is reapplied on resume so removed developer tools cannot persist. |
| Separate launcher/icon identity                                 | Pass                                  | Existing profile/scaffold/package contracts remain unchanged.                                                                                                   |
| Multiple declared backends/modules                              | Pass                                  | Renderer still receives declared bounded projections only; no endpoint/process authority was added.                                                             |
| Session discovery/resume/recovery                               | Pass locally                          | Current-directory list, compacted history, transcript ledger, integrity, and draft-preserving failures are covered.                                             |
| Permissions, forms, and mutation confirmation                   | Pass locally                          | Every reachable callback has a declared response operation and safe pending-state projection; streamed progress cannot silently cancel a pending decision.                    |
| Credentials/private reasoning/raw tool data excluded            | Pass locally                          | No secret value, raw tool input/output, full permission path, backend exception, or thought chunk crosses preload.                                              |
| Outputs                                                         | Backend pass; GUI integration pending | Use ADR-0013 inventory and artifact guard during the future GUI milestone.                                                                                      |
| Named-shell exclusion                                           | Pass                                  | No named shell or domain GUI was implemented.                                                                                                                   |

## Comprehensive audit coverage

| Lens                         | Verdict after correction | Boundary checked                                                                                           |
| ---------------------------- | ------------------------ | ---------------------------------------------------------------------------------------------------------- |
| Architecture seams           | Pass locally             | Electron main retains ACP, process, directory, credential, and adapter authority; preload stays bounded.   |
| Cascade and blast radius     | Pass locally             | Per-product process registry, bounded lists/ledgers, deadlines, and fail-closed capability resolution.     |
| Repository compliance        | Pass locally             | Repo governance, consumer contracts, source-of-truth rules, and DS-7 evidence rules remain explicit.       |
| Concurrency                  | Pass after SHP-DEF-052    | Progress/permission interleaving, close/send races, stale generations, and one-use responses.              |
| Data integrity               | Pass locally             | Canonical directory/session binding, transcript integrity, atomic settings, and current provisioning.      |
| Input/output boundaries      | Pass locally             | Strict bounded inputs and safe projections; no secrets, raw ACP authority, or arbitrary backend calls.     |
| Invariant synchronization    | Pass locally             | Snapshot/events, confirmation state, accepted directory, provisioning, and session state agree.            |
| Negative space               | Pass locally             | No GUI, named shell, global-settings mutation, developer-tool default, generic RPC, or credential storage.  |
| Reliability                  | Pass locally             | Startup, restart, prompt failure, shutdown, cleanup, and registry recovery are bounded and classified.      |
| Security                     | Pass locally             | Sender/frame validation, opaque credential references, safe errors, directory fencing, and thought hiding. |
| State transitions            | Pass after SHP-DEF-052    | Only terminal session outcomes clear pending interactions; invalid/stale operations fail closed.           |
| Temporal behavior            | Pass locally             | Resume buffering, event-gap repair, deadlines, transport-close grace, and restart ordering are explicit.   |
| GUI workflow contract        | Backend pass; GUI gated  | The nonvisual workflow is supportable without renderer guesses; presentation evidence remains deferred.    |
| Failsafe readiness           | Pass locally             | Applicable user/config/dependency/interruption/network/corruption/stall/degraded scenarios reach safe states. |

## Failsafe scenario record

| Record    | Family | Injection/evidence                                                                 | Predicted safe state                  | Observed classification |
| --------- | ------ | ---------------------------------------------------------------------------------- | ------------------------------------- | ----------------------- |
| FSR-DS-01 | SC-USR | Hostile directory, stale generation, unknown credential/session, and invalid prompt tests | `fail_closed`                         | `safe-stop`             |
| FSR-DS-02 | SC-CFG | Malformed/oversized settings, profile, consumer, and provisioning fixtures         | `fail_closed`                         | `safe-stop`             |
| FSR-DS-03 | SC-DEP | Missing backend/resource/credential/adapter startup and live-child paths           | `fail_closed` or honest `fail_degraded` | `safe-stop` / `degraded-honest` |
| FSR-DS-04 | SC-INT | Interrupted settings writes plus packaged window/backend close and stale cleanup   | `fail_rollback`                       | `safe-stop`             |
| FSR-DS-05 | SC-NET | ACP/provider/adapter failure, malformed response, timeout, and reconnect paths      | `fail_visible` or honest `fail_degraded` | `safe-stop` / `degraded-honest` |
| FSR-DS-06 | SC-COR | Corrupt settings and transcript-gap/compacted-resume paths                         | `fail_visible` or `fail_resumable`    | `safe-stop` / `degraded-honest` |
| FSR-DS-07 | SC-STL | Credential deadline, startup phase bounds, prompt cancellation, and ACP close grace | `fail_visible`                        | `safe-stop`             |
| FSR-DS-08 | SC-DEG | Unavailable credential catalog/adapter and recovery-state projections              | honest `fail_degraded`                | `degraded-honest`       |

The local runtime subsystems score at least 2 for detection, containment, recovery, and signal;
the tested directory/settings, managed-backend, session/interaction, and adapter paths score 3 for
their exercised scenarios. Credential recovery remains a deliberate handoff to full Gosling rather
than shell ownership, so it is present but not a shell-local mutation path. No averaging changes the
release decision: DS-7 acceptance itself remains not ready because the exact corrective source has
not yet been committed, clean-source packaged, or validated by current exact-SHA CI.

## Validation evidence

- `cargo fmt --all -- --check` — passed.
- `cargo clippy --all-targets -- -D warnings` — passed.
- Exact-final full Rust workspace with an isolated Gosling config root — passed, including 1,773
  core tests plus all integration and doc-test suites. `GOSLING_DISABLE_KEYRING` was explicitly
  absent, proving the test-only file-secret boundary rather than relying on an environment bypass.
- Desktop tests — 110 files, 795 tests passed.
- Live managed-backend shell session integration — 15 tests passed, including cross-directory
  resume rejection before `loadSession` and accepted-directory resume after restart.
- Shell runtime E2E — 7 tests passed, including Rust-side wrong-directory load rejection and
  replacement of a stale developer-shell selection after provisioning removed it.
- Desktop typecheck, ESLint, and i18n validation — passed.
- Shell consumer/profile/scaffold/package-script tests — 57 tests passed.
- Current macOS arm64 Default Shell package/readback — passed; profile hash
  `830f6143a45ea309c42f03cb440410b3eb6484009c86cda4aa98f0a7e1282950`, binary hash
  `75790e0e489e7c589cb7880750df344452fff331d399fac30f61178fef780ca5`. Because this
  package predates the committed corrective candidate, it is behavioral evidence, not DS-7
  `sourceClean:true` acceptance.
- Actual packaged renderer/preload/backend replay — reached verified `ready`; the initial close
  exposed SHP-DEF-046/047 and a live orphan, which was terminated by exact PID. After patch/rebuild,
  startup pruned the dead registry entry, close terminated the backend with code 0, no matching
  process remained, and `backend-processes.json` contained an empty process list.
- A later packaged preload-boundary workflow probe restored a backend-canonicalized temporary
  directory, listed sessions, rejected a stale generation through the stable `STALE_REQUEST`
  envelope, created/detached/resumed a session, replayed bounded history into a
  `resume_uncertain` transcript, and rejected an empty prompt through `INVALID_REQUEST` without
  starting a model call or corrupting the active session. Gosling intentionally excludes
  zero-message sessions from `session/list`; the probe verified direct resume instead of treating
  that filter as data loss. Its two empty test sessions, temporary settings, and temporary workspace
  were removed afterward through the product session command and exact temporary-path cleanup.
- A follow-up authority audit found SHP-DEF-048. Focused unit tests and the live managed-backend
  integration prove that cross-directory resume is rejected before `loadSession`, while resume
  through the runtime controller supplies and accepts only the main-owned canonical directory.
- The package was rebuilt after SHP-DEF-048. Through the real packaged preload, a session created
  under temporary directory A was rejected after restart under temporary directory B with only
  `SESSION_UNAVAILABLE`; no foreign path crossed the failure, no session became active, shutdown
  remained clean, and the exact empty test session and temporary state were removed afterward.
- A deeper backend-policy audit found SHP-DEF-049. Rust now rejects relocation even for a direct ACP
  caller, revalidates current shell provisioning during activation, and replaces persisted
  extension/skill selections with the current allowlist. The shell-runtime E2E first created a
  session with a developer shell and explicit skill selection enabled, restarted with an empty
  extension allowlist and omitted skill selection, rejected a different directory, then loaded from
  the stored canonical directory with no tools, enabled extensions, or obsolete skill-selection
  state.
- The exact-final acceptance rerun exposed SHP-DEF-050. Stack sampling proved the apparent
  workspace-service stall was a synchronous macOS Keychain read during unit-test credential
  migration. Test builds now select file-backed secrets for default and custom `Config` instances;
  production behavior is unchanged. The feature-enabled isolation regression, formerly stalled
  workspace test, and entire workspace pass without a keyring-disable override.
- The next exact-source packaged replay exposed SHP-DEF-051: the three-second credential-catalog
  deadline returned `unavailable`, but the timed-out blocking worker remained inside macOS
  Keychain decryption and forced normal cleanup to `SIGKILL`. Managed Electron backends now make
  protected-store access noninteractive, and main waits a bounded interval for ACP transport close
  before signalling the child. On the same host and credential state, the rebuilt package reached
  `ready` in 23 ms with catalog status `available`, exposed only `core:session`, stopped in 6 ms,
  exited the backend with code 0, left no matching process, and recorded an empty registry.
- The package was rebuilt after SHP-DEF-051 and independently read back with the new binary hash.
  Through the real packaged preload it reached compatible `ready`, exposed only the neutral
  `core:session` module, then closed with backend exit code 0, no matching process, and an empty
  product-local process registry.
- A final interaction-lifecycle pass exposed SHP-DEF-052: the runtime treated every session update
  other than prompt start as terminal, so an ordinary `tool_call_update` could silently cancel the
  permission or form it accompanied. Cleanup now occurs only on completed, cancelled, or failed
  outcomes. Regressions hold a permission across streamed tool progress and a form across streamed
  agent content, resolve each exact one-use action explicitly, and the full Desktop suite passes
  795/795.
- Source-negative-space and diff checks — no GUI or named-shell implementation was added.

## Workflow and failure-path verdict

The backend/main contract now supports this theoretical path without requiring renderer guesses:

1. start and expose verified product/lifecycle facts;
2. choose or restore one validated working directory;
3. choose a safe credential reference, or hand off to full Gosling for relink;
4. list up to 20 message-bearing current-directory sessions, then create or resume one;
5. reconcile compacted history, live updates, and bounded transcript repair;
6. submit/cancel a prompt while preserving the draft on classified failure;
7. answer permission, supported form, and domain-confirmation requests through fenced one-use IDs;
8. consume declared module and durable Outputs projections without renderer backend authority;
9. retry, save redacted diagnostics, or hand off to full Gosling when local recovery is insufficient.

The user-visible design package has not been produced. Under the supplied `plan-webapp-design`
workflow, Gate 1 (product/workflow design) and Gate 2 (front-end handoff) remain deferred until DS-7
is accepted; build Gates 3–6 are not started.

## Decision and next gate

The corrective implementation is locally ready as a candidate revision. It is **not yet safe to
begin the GUI**, because the containing revision still needs clean-source package/readback, current
CI bound to its exact SHA, and post-CI operator GO. In accordance with the operator's rule, this
audit does not create the Default Shell GUI implementation plan yet.

Next action: publish the containing revision, require mandatory CI for its exact head, repeat and
record clean-source package readback, and request the explicit DS-7 decision. Only a GO at that
point unlocks the generic GUI design/handoff plan; named shells remain blocked through M5.
