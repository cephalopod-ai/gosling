# Defect ledger — Gosling shared shell productization

Every audit finding or implementation defect enters this ledger and receives one disposition: `open`, `fixed`, `verified-not-a-defect`, `deferred-with-risk`, `blocked-by-input`, or `blocked-by-tooling`. A fixed defect requires a regression test or an explicit not-testable reason.

## Baseline known gaps

| ID          | Source                                       | Finding                                                                                                                                                                    | Severity | Root-cause status                                               | Planned gate | Disposition / evidence                                                                                                                                                                                                                                                                                                                                                                                                            |
| ----------- | -------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- | --------------------------------------------------------------- | ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| SHP-DEF-001 | PR #46 deferral/static inspection            | `createMinimalShellHost` has no production Electron call site; no shared shell application bootstraps it                                                                   | high     | Known implementation gap, not yet a defect in merged foundation | Gate 4       | fixed: dedicated shell bootstrap/main/Vite entries package and run; source/package evidence in `evidence/gate-4.md`                                                                                                                                                                                                                                                                                                               |
| SHP-DEF-002 | PR #46 deferral                              | No actual packaged Electron renderer-to-backend shell smoke test exists                                                                                                    | high     | Known validation gap                                            | Gate 6       | fixed locally on the supported macOS arm64 host: a freshly built Default Shell package was driven through its real Electron renderer/preload boundary and reached backend-verified `ready`; startup-phase timeout coverage preserves the failure oracle. Exact-revision CI binding remains DS-7.                                                                                                                                  |
| SHP-DEF-003 | GitHub CI runs `31642034573` / `31659795858` | Linux `Build and Test Rust Project` can fail before Gosling tests because restored Cargo state lacks native `rusty_v8`                                                     | high     | CI lacked deterministic independent native archive provisioning | Gates 0–1    | fixed locally: tested helper prepares before rust-cache and exports one path; remote Linux verification remains blocked (`evidence/gate-1.md`)                                                                                                                                                                                                                                                                                    |
| SHP-DEF-004 | PR #46 deferral/static package inspection    | Shell-specific icons, updater feeds, complete package artifacts, and release-profile inputs are absent                                                                     | medium   | Deliberately deferred distribution work                         | Gates 3/7    | open                                                                                                                                                                                                                                                                                                                                                                                                                              |
| SHP-DEF-005 | Static inspection                            | Current Forge configuration derives only product name, protocol, and package ID from independent environment values; other product identity/assets remain Gosling-specific | high     | No canonical product-profile resolver yet                       | Gates 2–3/7  | fixed at Gate 3: strict canonical resolver and thin Forge projection replace independent overrides; 34 profile/Forge/CLI/fixture tests pass; package readback remains tracked by SHP-REQ-012                                                                                                                                                                                                                                      |
| SHP-DEF-006 | Static inspection                            | `ShellFrame`/`ShellStatus` do not provide a shared runtime state/recovery/diagnostic/handoff application                                                                   | medium   | Deliberately minimal foundation                                 | Gate 5       | open                                                                                                                                                                                                                                                                                                                                                                                                                              |
| SHP-DEF-007 | Static inspection                            | No observed installed coexistence test covers full Gosling plus multiple shell identities                                                                                  | high     | Acceptance harness absent                                       | Gate 6       | fixed for the supported DS-7 host: the automated two-identity process test passes, and the 2026-08-15 host replay observed installed full Gosling plus packaged Default Shell and fixture B concurrently with disjoint app/process registries, single-instance locks, independent shutdown, no shell orphan, and full Gosling left running. Cross-platform repetition remains R6, not a reason to erase the observed host result. |
| SHP-DEF-008 | Static inspection                            | No explicit bundled-core/profile/provisioning/handoff compatibility gate exists in Electron startup                                                                        | high     | Compatibility policy not yet frozen                             | Gates 2/4    | fixed: canonical capabilities plus main-owned ACP/profile/provisioning compatibility, post-preflight session seam, strict handoff schema sender/receiver                                                                                                                                                                                                                                                                          |

These entries are not claims that implementation is broken beyond its documented current scope. They convert agreed remaining work and known CI failure into trackable closure items.

## Findings discovered during execution

| ID          | Source                             | Finding                                                                                                                                            | Severity | Root-cause status                                                                         | Planned gate             | Disposition / evidence                                                                                                     |
| ----------- | ---------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- | -------- | ----------------------------------------------------------------------------------------- | ------------------------ | -------------------------------------------------------------------------------------------------------------------------- |
| SHP-DEF-009 | Gate 0 GitHub run `31660173759`    | Current merged-main Rust CI fails `test_weather_tool` because an Anthropic replay hash is missing after V8 compilation succeeds                    | medium   | Recorded second-turn key omitted Anthropic pre-tool text now preserved in session history | R0                       | fixed: recording input/key reconciled; focused replay and two Linux Rust executions pass (`evidence/r0.md`)                |
| SHP-DEF-010 | Gate 0 toolchain probe             | Direct shell resolved Homebrew Rust 1.97.1 instead of repository-pinned Rust 1.92                                                                  | medium   | Hermit was not activated in the invoking shell                                            | All implementation gates | fixed operationally: every authoritative command starts with `source bin/activate-hermit`; no code test required           |
| SHP-DEF-011 | Gate 1 hostile helper review       | A valid cached archive without a `.sha256` sidecar was accepted without an integrity record                                                        | high     | Cache-hit condition treated missing sidecar as acceptable                                 | Gate 1                   | fixed: cache hit now requires valid archive + sidecar + matching digest; regression deletes sidecar and proves repair      |
| SHP-DEF-012 | Gate 1 supply-chain audit          | First workflow draft prepared V8 after Cargo cache restoration, allowing helper seeding from restored `target/**` instead of pinned upstream bytes | high     | Correct helper was invoked at the wrong trust-boundary order                              | Gate 1                   | fixed: helper tests and fresh prepare run before rust-cache; workflow-order regression and supply-chain audit pass         |
| SHP-DEF-013 | Gate 3 resolver review             | Initial asset resolver treated `iconBase` as a file instead of the frozen path stem                                                                | high     | Partial implementation did not yet inventory target-specific extensions                   | Gate 3                   | fixed: target asset inventory plus missing/type/format/dimension tests                                                     |
| SHP-DEF-014 | Gate 3 Forge release audit         | Non-publishable fixture could still consume Windows signing environment values                                                                     | high     | Fixture guard initially covered only macOS signing/notarization                           | Gate 3                   | fixed: source-derived signing guard covers macOS and Windows; hostile environment regression passes                        |
| SHP-DEF-015 | Gate 3 collision review            | Initial collision checker compared raw values across unrelated identity fields                                                                     | high     | Collision key lacked field identity                                                       | Gate 3                   | fixed: ten same-field maps with platform case normalization; exhaustive collision and cross-field non-collision tests      |
| SHP-DEF-016 | Gate 3 asset audit                 | Existing checks trusted icon extension/existence without validating format or square dimensions                                                    | medium   | Asset inventory lacked content inspection                                                 | Gate 3                   | fixed: dependency-free PNG/ICO/ICNS/SVG structural and dimension validators with malformed regressions                     |
| SHP-DEF-017 | Gate 4 packaged readback           | Forge could package ignored stale `ui/desktop/src/bin/gosling` while the manifest named current HEAD                                               | critical | Package path trusted an independently staged ignored artifact                             | Gate 4                   | fixed: tracked host build/stage/package/verify wrapper; exact built/staged/embedded hash regression and real readback pass |
| SHP-DEF-018 | Gate 4 macOS package readback      | Electron Packager stripped underscores from fixture `macosBundleId`, producing profile/plist identity mismatch                                     | high     | Profile validator allowed characters Packager normalizes away                             | Gate 4                   | fixed: stable hyphenated fixture IDs, stricter validator, hostile test, and exact plist readback                           |
| SHP-DEF-019 | Gate 4 renderer package inspection | Custom Vite root emitted renderer assets outside Forge package staging                                                                             | high     | Renderer output root did not match Forge staging assumptions                              | Gate 4                   | fixed: root-level shell entry and dedicated renderer input; verifier requires packaged `shell.html`                        |

## Findings from project-shell readiness reassessment

| ID          | Source                                  | Finding                                                                                                           | Severity | Root-cause status                                                                                     | Planned gate | Disposition / evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| ----------- | --------------------------------------- | ----------------------------------------------------------------------------------------------------------------- | -------- | ----------------------------------------------------------------------------------------------------- | ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| SHP-DEF-020 | Current main / GitHub run `31695906352` | GNU `stat -f` output reaches numeric comparison and triggers `File: unbound variable` before Linux Rust tests     | high     | portable size probe used command success instead of numeric/platform semantics                        | R0           | fixed: GNU-first/BSD-fallback numeric probe plus hostile fake-`stat` regression; two Linux jobs pass (`evidence/r0.md`)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| SHP-DEF-021 | Renderer/Vite/Forge inspection          | One hard-coded neutral renderer is always built; no consumer can supply a renderer without host edits             | critical | original plan specified a domain slot but no composition topology/manifest                            | R1–R2        | fixed: R2 strict consumer manifest, fixed-host Vite/Forge projection, two neutral bundles, and no shell fallback without a consumer manifest                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| SHP-DEF-022 | Preload/ACP inspection                  | Renderer has no session/prompt/update/cancel service; main ACP is preflight/create/resume only                    | critical | frozen preload omitted the primary application workflow                                               | R1/R3        | locally fixed: typed main-owned create/resume/prompt/cancel and bounded update projection cross the narrow IPC bridge; real child tests cover stream, cancel, child loss/retry, and compacted resume. Package/install reproduction remains R6                                                                                                                                                                                                                                                                                                                                                                 |
| SHP-DEF-023 | Focused ACP callbacks                   | Permission is always cancelled, elicitation declined, and session updates discarded                               | critical | preflight-safe callbacks were treated as application-ready                                            | R3/R5        | locally fixed: permission/elicitation callbacks await explicit single-use generation/session-fenced decisions and cancel on teardown; live allow-once, deny, and MCP form submission tests pass. R5/R6 retain presentation/package evidence                                                                                                                                                                                                                                                                                                                                                                   |
| SHP-DEF-024 | Rust CLI/runtime inspection             | A domain adapter needs an operator-owned, live-negotiated, bounded lifecycle before any shell can report it ready | critical | R4 local conformance complete; packaged/cross-platform reproduction remains                           | R1/R4/R6/R8  | locally fixed: descriptor v2, a validated `domain_adapters` registry, hardened stdio startup, exact live-descriptor negotiation, frame-level MCP/resource-count limits, a session/generation-fenced single-use Rust confirmation record, and typed capability-gated Electron relay now have neutral stdio and authenticated `gosling serve` evidence. The live route proves snapshot, guarded mutation, confirmation, mismatch-to-`incompatible`, idle and in-flight crash projection, normal and non-cooperative cleanup, and backend/adapter restart. R6/R8 retain packaged and cross-platform reproduction |
| SHP-DEF-025 | Lifecycle/controller inspection         | Every contracted lifecycle state/action needs a production producer and recovery proof                            | high     | producer coverage exceeds complete recovery acceptance                                                | R3/R6        | locally fixed for R3: prompt, credential, cleanup, invariant, child-loss/retry, adapter crash/restart, and forced-cleanup cases have production producers and live evidence. R6 retains packaged reproduction                                                                                                                                                                                                                                                                                                                                                                                                 |
| SHP-DEF-026 | Runtime snapshot inspection             | Renderer cannot read server-verified identity, namespace, provisioning summary, session, or adapter capability    | high     | snapshot has verified facts but recovery/status coverage remains incomplete                           | R1/R3        | locally fixed: safe snapshot carries bounded identity, compatibility, verified runtime namespace, provisioning issue paths, session, pending interactions, and adapter status/capability while excluding raw ACP/configuration/path authority                                                                                                                                                                                                                                                                                                                                                                 |
| SHP-DEF-027 | Forge/package inspection                | Shell mode inherits Gosling metadata/permissions/templates and bundles all `src/bin` resources                    | high     | thin projection covered primary identity but not complete least-privilege metadata/resource inventory | R2/R6/R7     | fixed for R2 macOS-host package evidence: one target-specific Gosling binary, exact declared resources, no inherited document/TCC metadata, minimized Flatpak permissions, and tamper-resistant readback; R6/R7 retain cross-platform workflow evidence                                                                                                                                                                                                                                                                                                                                                       |
| SHP-DEF-028 | Plan/evidence reconciliation            | Gate 4 GO omitted several declared failure paths and moved them to Gate 6                                         | high     | gate decision narrowed to process boundary without changing original exit criteria                    | R0/R3/R6     | fixed locally 2026-08-19 without rewriting the historical GO: the reusable acceptance job now executes the real child integration matrix plus packaged backend-loss/retry, explicit stop, clean close, fresh launch, and multi-identity coexistence. Static workflow tests prevent silently narrowing those gates again; exact-revision remote execution remains tracked by SHP-DEF-053.                                                                                                                                                                                                                                                                  |
| SHP-DEF-029 | Profile/build topology inspection       | Profiles/scripts require a Gosling checkout; no external consumer SDK/template/conformance path exists            | high     | product identity extension was mistaken for project-shell consumption                                 | R1/R2/R8     | fixed locally 2026-08-19: versioned `@repo-makeover/gosling-shell-kit` owns the shared resolver, external neutral scaffold, conformance command, and deterministic build-resolution interface. A packed archive is installed into a temporary package outside Gosling; init/check/resolve pass without host edits or a Gosling checkout, while unpinned versions and caller-declared roots fail closed. Registry publication remains separately unauthorized and unnecessary for the local package proof.                                                                                                      |
| SHP-DEF-030 | Workflow inventory                      | Reusable shell build/smoke/release workflows do not exist                                                         | high     | original Gate 7 not implemented                                                                       | R7           | fixed locally 2026-08-19 for the authorized unsigned acceptance boundary: a read-only reusable workflow and PR/main/nightly/manual caller build, read back, launch, recover, stop, restart, and coexist on pinned macOS arm64/x64, Linux x64, and Windows x64 runners. Signing, publication, notarization, updater promotion, and production release remain explicitly disabled and require a later operator-authorized release gate.                                                                                                                                                                                                                       |

## Findings discovered during R0 repair

| ID          | Source                                      | Finding                                                                                                              | Severity | Root-cause status                                                                                    | Planned gate | Disposition / evidence                                                                                                |
| ----------- | ------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- | -------- | ---------------------------------------------------------------------------------------------------- | ------------ | --------------------------------------------------------------------------------------------------------------------- |
| SHP-DEF-031 | PR run `31729883101` Desktop profile job    | Parallel profile tests can make package readback observe another test's temporary untracked fixture                  | high     | Node test files ran concurrently while the verifier intentionally checks repository-global state     | R0           | fixed: profile suite uses `--test-concurrency=1`; 41/41 locally and in both clean CI executions                       |
| SHP-DEF-032 | PR run `31729883101` Linux Rust job         | Concurrent tool completion can suspend while holding `BEGIN IMMEDIATE`, blocking response persistence for 30 seconds | high     | A write transaction spanned async scheduling even though the conditional completion update is atomic | R0           | fixed: direct conditional autocommit update; focused regression passed 10 times, full agent 17/17, and Linux CI twice |
| SHP-DEF-033 | PR run `31731280500` Linux helper self-test | The helper regression's BSD-first mtime probe compared successful GNU `stat -f` filesystem prose as a timestamp      | medium   | The test repeated the production portability assumption for mtime                                    | R0           | fixed: numeric GNU-first/BSD-fallback mtime probe; local matrix repeated three times and Linux helper step passes     |

## Findings from PG-50 revision-bound rerun

| ID          | Source                                  | Finding                                                                                                                                                        | Severity | Root-cause status                                                                                       | Planned gate | Disposition / evidence                                                                      |
| ----------- | --------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- | ------------------------------------------------------------------------------------------------------- | ------------ | ------------------------------------------------------------------------------------------- |
| SHP-DEF-034 | PG-50 rerun, `cargo clippy -D warnings` | Manual `if let Err(..) { return Err(..) }` block in the R1-R4 ACP prompt-failure path (`crates/gosling/src/acp/server.rs`) fails clippy's `question_mark` lint | low      | R1-R4 implementation did not run clippy with `-D warnings` on the final isolated diff before this rerun | PG-50        | fixed: rewritten as `persisted?;` in commit `b921e6e`; clean clippy rerun confirmed         |
| SHP-DEF-035 | PG-50 rerun, commit isolation           | The R1-R4 shell commit (`5933637`) incidentally carried an unrelated `SummarizerSection.tsx` auto-fill/auto-persist UX change                                  | low      | Scope hygiene gap when the R1-R4 work was committed, not a functional defect in either change           | PG-50        | fixed: isolated out by commit `c232d04`, which restores the file to its pre-`5933637` state |

## Findings discovered during the 2026-08-15 DS-7 corrective replay

| ID          | Source                                     | Finding                                                                                                                                       | Severity | Root-cause status                                                                                                                             | Planned gate | Disposition / evidence                                                                                                                                                                                                                                                                                                                                                |
| ----------- | ------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------- | -------- | --------------------------------------------------------------------------------------------------------------------------------------------- | ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| SHP-DEF-036 | Packaged Electron renderer/backend replay  | The package remained in `validating` because provisioning synchronously enumerated credential secrets and macOS Keychain access stalled       | high     | Shell provisioning read the credential catalog even when no fixed profile was provisioned; the synchronous Keychain call had no backend bound | DS-4/DS-7    | fixed locally: no-profile provisioning skips the catalog; required shell catalog reads run on a blocking worker with a three-second fail-closed bound scoped per agent; every Electron preflight phase has a ten-second diagnostic bound. Unit regressions pass and the rebuilt package reaches `ready` with credentials honestly `unavailable` on the unsigned host. |
| SHP-DEF-037 | Live selectable-credential acceptance test | An unknown caller-selected credential ID could reach session creation, and the acceptance test explicitly tolerated either success or failure | high     | The backend validated base provisioning but did not re-resolve the caller's selected opaque ID immediately before session creation            | DS-4/DS-7    | fixed locally: session creation now re-reads the bounded catalog, rejects unknown/revoked/provider-mismatched profiles, and carries the resolved safe profile into provider selection and session snapshot. The live child test proves unknown rejection and successful pinning of a real configured profile.                                                         |

## Findings from the 2026-08-15 pre-GUI workflow/data-flow audit

| ID          | Source                                 | Finding                                                                                                                                                                                       | Severity | Root-cause status                                                                                                                                                                                    | Planned gate            | Disposition / evidence                                                                                                                                                                                                                                                                                                                                                                                                                              |
| ----------- | -------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| SHP-DEF-038 | Operator-supplied provider setup image | Gemini provider configuration reports `OAuth login failed: Internal error`                                                                                                                    | medium   | The modal discarded the provider-specific ACP error payload and rendered only the generic JSON-RPC message                                                                                           | Provider auth follow-up | fixed 2026-08-17: the modal uses `describeAcpError`; its focused regression preserves the provider-specific cause. See `docs/logs/session/2026-08-17-gemini-oauth-error-repair.md` and `docs/TODO.md`.                                                                                                                                                                                                                                                   |
| SHP-DEF-039 | Consumer/runtime contract audit        | A consumer could submit prompts while omitting permission and elicitation response channels, stranding requests                                                                               | critical | Capability prerequisites did not model callback response obligations                                                                                                                                 | DS-7                    | fixed locally: resolver prerequisites plus callback-side fail-closed gating and regressions                                                                                                                                                                                                                                                                                                                                                         |
| SHP-DEF-040 | Compacted-resume walkthrough           | History emitted during `loadSession` was discarded while the controller was in `resuming`                                                                                                     | high     | Update ingestion accepted only an active prompt attempt                                                                                                                                              | DS-7                    | fixed locally: resume-fenced history buffering, explicit delivery kind, and replay regression                                                                                                                                                                                                                                                                                                                                                       |
| SHP-DEF-041 | Session/reload recovery walkthrough    | The renderer had neither bounded session discovery nor a transcript event-gap repair operation                                                                                                | high     | Resume required a caller-known ID and update sequencing had no bounded recovery path                                                                                                                 | DS-7                    | fixed locally: current-directory session summaries and a 256-update/48-KiB active-session ledger                                                                                                                                                                                                                                                                                                                                                    |
| SHP-DEF-042 | Error/context projection audit         | Prompt failure omitted cause/recovery, invoke errors were arbitrary strings, and provider/model context was absent                                                                            | high     | Transport and UI recovery were not represented as stable safe types                                                                                                                                  | DS-7                    | fixed locally: `ShellOperationFailure`, draft-preserving prompt failures, and safe session context projection                                                                                                                                                                                                                                                                                                                                       |
| SHP-DEF-043 | Interaction/privacy projection audit   | Permission/form summaries were insufficient, domain-confirm pending state disagreed across snapshot/event paths, and thought chunks were renderer content                                     | high     | Safe projection contracts lagged the reachable backend behavior                                                                                                                                      | DS-7                    | fixed locally: bounded structured summaries, schema validation, safe confirmation mirror/event, and thought-chunk rejection                                                                                                                                                                                                                                                                                                                         |
| SHP-DEF-044 | Fresh session-discovery audit          | A nonconforming backend list response could project a session from outside the selected working directory                                                                                     | high     | Main requested a directory filter but did not independently enforce the returned boundary                                                                                                            | DS-7                    | fixed locally: main normalizes and filters every safe summary against the accepted directory before applying the 20-item bound; regression included                                                                                                                                                                                                                                                                                                 |
| SHP-DEF-045 | Fresh interaction-lifecycle audit      | Replay-protection IDs accumulated without a hard process-lifetime bound                                                                                                                       | medium   | The issued-ID set preserved replay rejection but had no exhaustion ceiling                                                                                                                           | DS-7                    | fixed locally: new interactions fail closed after 4,096 issued IDs until the bounded runtime is restarted                                                                                                                                                                                                                                                                                                                                           |
| SHP-DEF-046 | Packaged normal-close replay           | A final lifecycle event sent after renderer destruction threw synchronously, aborted backend cleanup, emitted an unhandled rejection, and left `gosling serve` orphaned                       | high     | IPC event publication assumed `webContents.send` remained valid throughout shutdown                                                                                                                  | DS-7                    | fixed locally: all shell events drop after renderer destruction and catch the check/send race; unit regression plus rebuilt packaged replay terminates the backend with code 0 and leaves no matching process                                                                                                                                                                                                                                       |
| SHP-DEF-047 | Packaged restart after failed shutdown | The dead orphan's process-registry record survived a later successful shell run                                                                                                               | high     | The generic shell host registered/unregistered its current child but did not run the existing stale-process reconciliation before spawn                                                              | DS-7                    | fixed locally: shell host runs the existing product-local cleanup before every spawn and fails closed if cleanup cannot complete; unit order/failure regressions and rebuilt package prove stale entry pruning and an empty registry after close                                                                                                                                                                                                    |
| SHP-DEF-048 | Post-package resume authority audit    | A renderer that knew another session ID could bypass the filtered picker and load that session's stored working directory                                                                     | critical | `session/list` independently filtered the accepted directory, but `session.resume` trusted the ID and used the server-reported directory without comparing it to main's accepted canonical directory | DS-3/DS-7               | fixed locally: main passes its current accepted directory into resume; the ACP transport compares it to bounded server session metadata before `loadSession`, and mismatch is projected as `SESSION_UNAVAILABLE`. Unit, live managed-backend, and rebuilt packaged-preload regressions prove a mismatch never loads or reveals the foreign path and the accepted-directory resume succeeds                                                          |
| SHP-DEF-049 | Shell ACP activation-policy audit      | Direct `loadSession` could relocate a workspace-less shell session to the caller's directory, and a resumed session could retain extensions or skills removed from current shell provisioning | critical | Shell activation accepted the ACP caller's working directory and reused persisted extension/skill selection without reapplying the current product policy                                            | DS-1/DS-3/DS-7          | fixed locally: the Rust server canonicalizes the requested directory, requires it to equal the stored shell-session directory, revalidates current provisioning, and rebuilds current extension/skill selection before activation. The 7/7 shell-runtime E2E suite proves wrong-directory rejection, removal of a formerly enabled developer shell, and clearing of obsolete skill-selection state on correct resume                                |
| SHP-DEF-050 | Exact-final Rust acceptance rerun      | The documented full-workspace test path could block indefinitely while reading the operator's real macOS Keychain                                                                             | high     | Unit-test `Config::global()` selected production system-keyring storage; workspace migration probed configured provider secrets during otherwise isolated tests                                      | DS-7                    | fixed locally: test builds force both default and custom `Config` instances onto file-backed secret storage while production selection remains unchanged. A feature-enabled regression passes, the formerly blocked test completes without an override, and the exact-final full workspace passes with `GOSLING_DISABLE_KEYRING` explicitly absent                                                                                                  |
| SHP-DEF-051 | Exact-source packaged shutdown replay  | A selectable-catalog shell could reach `ready` but leave a timed-out macOS Keychain lookup alive, forcing normal backend cleanup to escalate from `SIGTERM` to `SIGKILL`                      | high     | The managed backend allowed protected-store UI from a headless child, and its timed-out `spawn_blocking` credential lookup remained live during Tokio runtime shutdown                               | DS-4/DS-7               | fixed locally: Electron-managed backends enforce noninteractive protected-store access, so unlocked credentials remain readable while a prompt-required lookup fails instead of blocking; main also gives ACP transport closure a bounded grace interval before backend termination. The rebuilt package reports an available catalog, reaches `ready` in 23 ms, stops in 6 ms, exits the backend with code 0, and leaves an empty process registry |
| SHP-DEF-052 | Post-patch interaction lifecycle audit | A streamed tool-progress update cleared the session's pending permission or form before the user could answer it                                                                              | critical | Runtime cleanup treated every update other than prompt start as terminal even though ACP `tool_call_update` and content progress are nonterminal                                                     | R3/DS-7                 | fixed locally: pending interactions survive all streamed progress and are cleared only by completed, cancelled, or failed session outcomes. Regressions hold a real permission across `tool_call_update` and a form across agent content, then resolve each explicitly; the full Desktop suite passes 795/795                                                                                                                                       |

## Findings from DS-7 operator acceptance (2026-08-18)

These two entries are gate conditions rather than broken behavior in merged source. They are
recorded here so `plan-webapp-design` Gate 3 cannot start by assuming either is closed.

### SHP-DEF-053 — DS-7 acceptance evidence is not bound to current `main`

- Discovered at gate / audit: DS-7 operator acceptance, `audits/ds-7-operator-acceptance.md`
- Requirement(s): SHP-REQ-054
- Severity: high
- Symptom and reproduction: the accepted DS-7 evidence is bound to
  `240ab751585afc03c68a710f8be10ea891ab168f`. `main` has since advanced 76 commits to
  `437d7bd7d7866356ddd3eb6feb0c32b52b4e8528`, and `6634ece38` ("fix(acp): carry the ACP secret in
  the WebSocket subprotocol, not the URL") modified `ui/desktop/src/shell/acpRuntime.ts`,
  `ui/desktop/src/shell/runtimeController.ts`, and `ui/desktop/src/shellHost.test.ts`. Under the
  repository's verify-don't-trust rule, revision-bound acceptance does not transfer across that
  change. Reproduce by comparing `git log 240ab7515..HEAD -- ui/desktop/src/shell` against the
  audit's recorded revision.
- Root cause: normal trunk progress after a revision-bound acceptance; no defect in the change
  itself. The ACP secret now travels in the WebSocket subprotocol rather than the URL, which is a
  transport-security improvement, but the packaged startup and coexistence replays that DS-7
  accepted were not re-run against it.
- Security/data/process/release impact: process only. Design Gates 1-2 read contracts and add no
  code, so they are unaffected. Gate 3 would build a renderer against evidence that no longer
  matches its own source tree.
- Disposition: `partially validated`. On the 2026-08-19 campaign working tree, Clippy, the full
  Desktop suite, shell profile, lint/type/i18n checks, the 15-case authenticated child integration,
  supported-host package/readback, packaged backend-loss/retry, explicit stop, clean close, fresh
  launch, and two-product coexistence pass. The reusable four-target workflow is locally schema-
  and policy-checked. Exact-revision remote CI remains outstanding because push was not authorized.
- Patch/files: `.github/workflows/shell-package-reusable.yml`,
  `.github/workflows/shell-package-smoke.yml`, `ui/desktop/scripts/shell-package-lifecycle.js`, and
  their contract tests make the formerly manual validation obligation repeatable.
- Regression test or not-testable reason: not a code defect. Closure requires reproducing the DS-7
  battery on one clean revision: full Rust workspace, full Desktop suite,
  `pnpm --dir ui/desktop run shell:test-profile`, `shell:check-profiles` with `sourceClean:true`,
  `shell:conformance`, a supported-host package/readback with recorded profile and backend hashes,
  a packaged renderer-to-backend startup and close/restart replay, and green current CI for that
  exact revision. The local lifecycle requirements are now met; commit cleanliness and remote CI
  must be recorded after the campaign commit exists.
- Validation/evidence: `gui/gate-3-build-record.md` §4,
  `../../logs/session/2026-08-18-default-shell-closure.md`, and
  `../../logs/session/2026-08-19-shell-defect-campaign.md`.
- Residual risk: local packaged evidence and static workflow validation are not substitutes for a
  green run of the committed four-target workflow and required CI at the same revision.

### SHP-DEF-054 — the durable Outputs inventory has no shell renderer projection

- Discovered at gate / audit: DS-7 acceptance surface mapping for Gate 2
- Requirement(s): SHP-REQ-055
- Severity: medium
- Symptom and reproduction: step 6 of the GUI implementation order in
  `docs/architecture/default-shell-template.md` requires the Default Shell GUI to consume the
  durable session Outputs inventory established by ADR-0013. The backend method
  `_gosling/unstable/session/artifacts/list` exists, but `shellIpcChannels` in
  `ui/desktop/src/shell/ipc.ts` declares no artifacts channel, `GoslingShellAPI` in
  `ui/desktop/src/shell/preloadApi.ts` exposes no artifacts namespace, `CAPABILITY_BY_CHANNEL` in
  `ui/desktop/src/shell/ipcMain.ts` maps no artifacts capability, and `ShellRuntimeSnapshot` in
  `ui/desktop/src/shell/runtimeSnapshot.ts` carries no artifact field. A renderer therefore has no
  authorized route to Outputs.
- Root cause: DS-5 scoped the module registry to extensions, skills, and one adapter. The Outputs
  inventory was accepted separately under ADR-0013 and never given a shell projection, because no
  renderer existed to need one.
- Security/data/process/release impact: none in merged source — the absence is fail-closed. The risk
  is that Gate 3 closes the gap the wrong way, with a renderer-side directory scan or a generic
  artifact passthrough, either of which would create a second authority over output provenance and
  contradict ADR-0013.
- Disposition: `fixed locally`.
- Patch/files: Rust exposes `_gosling/unstable/shell/session/artifacts/list` as a bounded,
  active-session-only projection containing only `name`, coarse `kind`, and `relation`; it returns
  at most 100 items plus `totalCount` and `truncated`. The generated SDK, consumer capability
  `session.artifacts.read`, typed main/preload bridge, and C-26 `OutputsPanel` carry that projection
  without path or filesystem authority.
- Regression test or not-testable reason: closure requires negative-space tests proving the renderer
  cannot reach the filesystem or an undeclared artifact, plus a live-child test that the projection
  matches the ADR-0013 inventory for the active session only.
- Validation/evidence: the Rust leakage regression, custom-method registry test, capability-gated
  main/preload/UI tests, negative-space suite, real authenticated `gosling serve` child test, full
  Rust/Desktop suites, and supported-host package/readback pass. See the Gate 3 build record §4.
- Residual risk: the projection is read-only and intentionally cannot open an artifact; adding file
  access later requires a separate authority and review.

## Findings from the Gate 3 GUI build (2026-08-18)

### SHP-DEF-055 — the host offers handoff in states where handoff cannot succeed

- Discovered at gate / audit: Gate 3 build audit, `gui/gate-3-build-record.md`
- Requirement(s): SHP-REQ-053, SHP-REQ-056
- Severity: high
- Symptom and reproduction: `lifecycle.ts` lists `handoff` in `allowedActions` for
  `relink_required` and `incompatible`. The operation cannot work in either state:
  `parseHandoffPrepareRequest` in `ui/desktop/src/shell/ipcMain.ts` calls
  `assertString(value.sessionId, 'sessionId', 512)`, which rejects an empty string, and
  `handoffPrepare` in `ui/desktop/src/shell/bootstrap.ts` throws
  `handoff request generation is stale or unavailable` when `controller.getAcp()` is null. After a
  failed startup there is no session and no ACP connection. A GUI that trusted `allowedActions`
  would render a primary "Open in Gosling" button that always fails, and the failure classifies as
  `STALE_REQUEST` with recovery `refresh`, which is misleading.
- Root cause: `allowedActions` describes lifecycle intent, not operation preconditions. The two were
  never reconciled, because no renderer existed to act on the list.
- Security/data/process/release impact: none — the operation fails closed. The impact is on recovery:
  Gate 1 §7 names "Open in Gosling" as the _only_ honest action for `relink_required`, so a user whose
  credential was revoked has no in-product path at all.
- Disposition: `fixed locally` by selecting the least-authority option: `handoff` is no longer an
  allowed lifecycle action in `relink_required` or `incompatible`. Server-owned envelope creation
  remains unchanged and live-session handoff remains available in `ready` and `degraded`.
- Patch/files: `ui/desktop/src/shell/lifecycle.ts` narrows the producer; the renderer's independent
  session/identity checks remain defense in depth.
- Regression test or not-testable reason: `ShellApp.test.tsx` asserts no handoff button in
  `relink_required` and `incompatible` with no identity or session, that the button appears and sends
  the real session id in `degraded`, and that the store refuses a sessionless handoff.
- Validation/evidence: `gui/gate-3-build-record.md` §4.
- Residual risk: startup credential failures remain instructional and require the user to open full
  Gosling themselves; the shell no longer advertises an operation that cannot succeed.

### SHP-DEF-056 — the renderer bundle acquired `node:fs` and `node:path` through the settings module

- Discovered at gate / audit: Gate 3 build, first real renderer build
- Requirement(s): SHP-REQ-055, SHP-REQ-059
- Severity: high
- Symptom and reproduction: the Default Shell settings panel renders the theme list and the
  0.8–2.0 text-scale bounds. Importing those values from `ui/desktop/src/shell/localSettings.ts`
  dragged that whole main-process module into the renderer graph. Reproduced by
  `vite build --config vite.shell.renderer.config.mts`, which reported
  `Module "node:fs" has been externalized for browser compatibility` and the same for `node:path`.
  Vite externalises rather than fails, so the bundle shipped with stubs that throw when called.
- Root cause: a values import across the main/renderer boundary. The type-only discipline was
  followed everywhere else, and the negative-space suite wrongly whitelisted this one module.
- Security/data/process/release impact: no secret or path crossed the boundary — the module was
  reachable but never called from the renderer. The risk was structural: a later change could have
  called a filesystem function from renderer code and been externalised silently.
- Disposition: `fixed`.
- Patch/files: `ui/desktop/src/shell/settingsSchema.ts` (new) holds the schema version, theme values,
  scale bounds, and validators with no Node or Electron import.
  `ui/desktop/src/shell/localSettings.ts` re-exports every symbol, so no existing importer changed
  and there is still one source of truth. `SettingsPanel` and the store import from
  `settingsSchema`. The whitelist was removed from `negativeSpace.test.ts`.
- Regression test or not-testable reason: `negativeSpace.test.ts` asserts that only `settingsSchema`
  may be imported for values from `../shell/*`, and that no kit module imports a Node builtin. The
  renderer build reports zero externalised modules.
- Validation/evidence: `gui/gate-3-build-record.md` §3 and §4.
- Residual risk: the executable check is import-shape based. A transitive leak through a _new_ pure
  module that itself imports Node would pass the import test and be caught only by the build warning,
  so the renderer build must stay part of the acceptance battery.

### SHP-DEF-057 — Gate 3 verification did not run on the operator's checkout or against the committed lockfile

- Discovered at gate / audit: Gate 3 build, environment assessment
- Requirement(s): SHP-REQ-061
- Severity: medium
- Symptom and reproduction: the operator's `ui/node_modules` was installed on macOS; the file bridge
  that reaches it executes Linux binaries, so `rollup`'s native module fails to load and neither
  `vitest` nor `vite` can start there. Verification therefore ran on a reconstructed Linux copy
  installed with `--no-frozen-lockfile` (`vitest` 4.1.0, `vite` 7.3.1), as root, with Electron
  packaging dependencies removed from that copy's `package.json`.
- Root cause: environment, not code.
- Security/data/process/release impact: none directly. The impact is evidential: §4 of the build
  record is not revision-bound, lockfile-bound, or CI-bound.
- Disposition: `fixed`. Verification ran in the operator checkout using its installed,
  lockfile-resolved dependencies and Hermit-managed toolchain.
- Patch/files: none. The container's `package.json` edit was deliberately not transferred; the
  repository copy is untouched.
- Regression test or not-testable reason: not a code defect.
- Validation/evidence: `gui/gate-3-build-record.md` §4 records the focused and full local results.
- Residual risk: none specific to the reconstructed environment; exact-revision CI remains tracked
  separately by SHP-DEF-053.

### SHP-DEF-059 — Desktop override collapsed the right dashboard panel

- Discovered at gate / audit: operator live GUI review, 2026-08-19.
- Requirement(s): Default Shell desktop-parity override; Gate 1 layout hierarchy.
- Severity: medium.
- Symptom and reproduction: at normal desktop width, Recent tasks rendered below Workspace instead
  of as the third panel on the right.
- Root cause: the desktop-specific stylesheet overrode the base three-column dashboard grid with a
  two-column grid and a 900 px maximum width.
- Security/data/process/release impact: none; the regression hid the intended desktop information
  hierarchy and wasted the right side of the canvas.
- Disposition: `fixed` on 2026-08-19. The desktop override now retains three columns through normal
  desktop widths and stacks only at the existing narrow breakpoint.
- Patch/files: `ui/desktop/src/shell-ui/shell.css` and the active GUI/session records.
- Regression test or not-testable reason: jsdom has no layout engine, so the existing focused tests
  protect the panel content and breakpoint classes while installed-renderer inspection proves the
  column bounds.
- Validation/evidence: Desktop lint/typecheck/focused tests, package/sign/install/readback, and a
  Playwright CDP inspection of the installed Electron renderer passed. The three panels shared the
  same y-coordinate and had strictly increasing x-coordinates at 1440 px viewport width.
- Residual risk: the existing responsive breakpoint still intentionally stacks panels below 800 px.

### SHP-DEF-060 — New Chat navigation action did not create a session

- Discovered at gate / audit: operator live GUI review, 2026-08-19.
- Requirement(s): Default Shell desktop-parity navigation; Gate 1 primary-action behavior.
- Severity: high.
- Symptom and reproduction: clicking New Chat on the default dashboard left the same dashboard
  visible and produced no error or composer.
- Root cause: the navigation handler only changed the local view to `workspace`; unlike Start new
  task, it never invoked the typed `session.create` operation.
- Security/data/process/release impact: the primary navigation action was inert. Starting from an
  active chat also needed ordered detach-before-create behavior to avoid overlapping sessions.
- Disposition: `fixed` on 2026-08-19. New Chat now creates immediately, detaches an active session
  first, and stops if detach fails.
- Patch/files: `ui/desktop/src/shell-ui/ShellApp.tsx`, `state/store.ts`, and focused component tests.
- Regression test: `starts a session from the New Chat navigation action`, `detaches an active
session before starting a new chat`, and `does not create a new chat when detaching the active
session fails`.
- Validation/evidence: the pre-patch installed renderer remained on the dashboard with no composer;
  focused Vitest failed before the patch and passed afterward. In the rebuilt installed app, the
  same click opened the chat workspace and exposed the Your request composer.
- Residual risk: New Chat remains unavailable while the runtime omits `session.create`, or while an
  active session cannot safely detach because it is streaming or awaiting an interaction.

### SHP-DEF-061 — current Desktop CI used a stale settings projection oracle

- Discovered at gate / audit: exact-revision CI reconciliation, run `32306201277`, Desktop job
  `96239443801`, on `main` revision `386a7442a38165fd524c4910a8b493afaae5d3eb`.
- Requirement(s): SHP-REQ-054; exact-revision CI acceptance.
- Severity: high.
- Symptom and reproduction: the Desktop job failed two `bootstrap.test.ts` cases after 988 tests
  passed. The settings read/update/reset handlers returned the required safe `modelSelection`
  projection, but four exact expected objects still described the older appearance/recovery-only
  response.
- Root cause: the settings IPC response gained `modelSelection` without updating every exact
  Bootstrap test oracle. Production behavior was correct; the stale tests made current CI red.
- Security/data/process/release impact: no unsafe runtime behavior was found, but the red required
  job prevented SHP-DEF-053 exact-revision acceptance and could obscure later regressions.
- Disposition: `fixed` locally on 2026-08-19. The assertions now require the complete unavailable
  model-selection projection across initial read, appearance update, cancelled reset, and confirmed
  reset.
- Patch/files: `ui/desktop/src/shell/bootstrap.test.ts` and the active campaign records.
- Regression test: the two formerly failing Bootstrap cases are themselves the regressions; exact
  equality now fails if the safe settings projection drops or mutates `modelSelection`.
- Validation/evidence: the pre-patch focused run reproduced 2 failures and 10 passes. Post-patch
  focused/full Desktop results are recorded in
  `docs/logs/session/2026-08-19-shell-defect-campaign.md`.
- Residual risk: remote CI remains revision-bound; the final campaign commit must receive a green
  required Desktop job before SHP-DEF-053 can close.

## New entry template

### SHP-DEF-058 — Default template exposed Gosling's credential catalog

- Discovered at gate / audit: operator live GUI review, 2026-08-19.
- Requirement(s): SHP-REQ-046; SHP-RSK-042.
- Severity: high.
- Symptom and reproduction: launching the committed Default Shell displayed the operator's
  `featherless` credential profile and offered “Use this account,” even though credential/provider
  selection belongs in the main Gosling application.
- Root cause: the committed fixture overrode the scaffold's fixed credential policy with
  `selectable_catalog`, and both the fixture and scaffold declared `credential.select`.
- Security/data/process/release impact: the projection contained only approved safe metadata, but
  it exceeded the intended authority and exposed an unrelated account-selection workflow.
- Disposition: `fixed` on 2026-08-19. The default fixture and generated scaffolds use fixed policy
  without catalog capability; fixed-policy snapshots ignore stale local selections; account UI is
  absent without explicit selection authority.
- Patch/files: Default Shell fixture provisioning/consumer manifest, shell scaffold generator,
  credential controller, shell UI, tests, and active design records.
- Regression test or not-testable reason: scaffold conformance rejects the credential list method;
  controller tests cover stale selection under denied policy; ShellApp tests prove no account
  catalog or controls render.
- Validation/evidence: focused shell/scaffold tests, desktop lint/typecheck, package/readback, and
  live installed-app inspection.
- Residual risk: explicitly provisioned selectable-catalog products retain their existing behavior.

### SHP-DEF-NNN — title

- Discovered at gate / audit:
- Requirement(s):
- Severity:
- Symptom and reproduction:
- Root cause:
- Security/data/process/release impact:
- Disposition:
- Patch/files:
- Regression test or not-testable reason:
- Validation/evidence:
- Residual risk:
