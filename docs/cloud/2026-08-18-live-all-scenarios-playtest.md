# Gosling live all-scenarios playtest and repair report — 2026-08-18

## Executive result

The authoritative 110-card library was evaluated against current `main` at `c203282753b93066f0af88ce196200b785e6454f` using disposable CLI, provider, server, session, workspace, and packaged-Desktop state.

Before repair: **26 Pass · 7 Fail · 77 Blocked**.

After the five selected reliability/data-flow repairs: **31 Pass · 0 Fail · 79 Blocked**. The two
Desktop cards that exposed the artifact regression moved from Fail to Blocked, not Pass, because the
post-fix signed package was build/test verified but macOS Keychain blocked the isolated app before a
final click-through could run. “Blocked” means the complete atomic card lacked evidence; it is not a
claim that the product behavior failed.

The five repaired defect roots were:

1. headless Ctrl-C returned success and invented an assistant reply;
2. non-TTY session removal announced deletion before failing and had no explicit bypass;
3. non-TTY session fork copied data before interactive input failed;
4. ACP stdin EOF dropped an accepted initialize response;
5. workspace-less CLI session artifacts were inventoried but not granted exact-file preview access.

No security item was promoted above these reliability and data-flow failures. Existing intentionally
deferred architecture, performance, and security work remains unchanged.

## Target and method

- Repository: `/Users/eric/Work/vscode/forked/gosling`
- Baseline: clean `main` at `c203282753b93066f0af88ce196200b785e6454f`
- Repair branch: `codex/playtest-repair-20260818`
- Involvement: L2 standard (inferred)
- Scenario source: `docs/test_scenarios/README.md` plus cards 01–18; 110 cards counted
- Provider oracle: loopback OpenAI-compatible fixture at `127.0.0.1:18765`
- Disposable state/evidence: `/tmp/gosling-playtest-20260818`
- Desktop: isolated packaged-app state; the installed user's Desktop state was not mutated
- Pass rule: every assertion in a card needed direct evidence; partial execution remained Blocked

The pass exercised help and command surfaces, config validity and preservation, provider success and
malformed/empty/rate-limit responses, machine output, session import/export/permissions, concurrent
CLI runs, broken MCP fail-closed behavior, authenticated serve boundaries, hard-kill recovery,
interactive fork independence, Desktop navigation/settings/workspaces/narrow layout, artifact
inventory, and ACP framing/EOF behavior. Cards requiring unavailable accounts, OAuth providers,
multiple live clients/windows, long-duration load, approval fixtures, TLS material, or complete atomic
matrices remained Blocked.

## Scenario outcome ledger

| File / area | Pass | Blocked | Fail after repair |
|---|---|---|---|
| 01 lifecycle | LC-01–04 | — | — |
| 02 chat | CH-01–04 | CH-05–06 | — |
| 03 workspaces | WS-01 | WS-02–04 | — |
| 04 providers/models | PM-01 | PM-02–04 | — |
| 05 extensions | EX-03–04 | EX-01–02 | — |
| 06 skills/plugins/subagents | — | SK-01–03 | — |
| 07 sessions | SE-01–03 | — | — |
| 08 permissions | — | PA-01–03 | — |
| 09 CLI surface | CL-01–04 | — | — |
| 10 Desktop settings | ST-01, ST-03 | ST-02 | — |
| 11 headless/serve/ACP | HS-03 | HS-01–02 | — |
| 12 stress/chaos | SX-01, SX-06 | SX-02–05, SX-07–09 | — |
| 13 advanced CLI/sessions | AC-02 | AC-01, AC-03–10 | — |
| 14 Desktop/native | — | DT-01–10 | — |
| 15 context/filesystem | CX-07 | CX-01–06, CX-08–10 | — |
| 16 provider/network | PN-01, PN-05 | PN-02–04, PN-06–10 | — |
| 17 authenticated protocol | AP-01, AP-05 | AP-02–04, AP-06–10 | — |
| 18 state integrity | SI-07 | SI-01–06, SI-08–10 | — |
| **Total** | **31** | **79** | **0** |

Pre-repair Fail cards were CH-03, SE-01, HS-03, AC-02, DT-06, DT-07, and AP-05.
CH-03, SE-01, HS-03, AC-02, and AP-05 passed their post-fix reruns. DT-06 and DT-07 are now
Blocked pending the final packaged click-through described below.

## Consolidated findings and closure

### GSL-PLAY-2026-007 — headless cancellation false-success and invented reply

- Priority/domain: P1 reliability and data integrity
- Evidence before: live PTY Ctrl-C during `gosling run` exited 0; export removed the user turn and
  persisted `Yes — what would you like me to do?` as if the model had replied.
- Repair: headless cancellation exits 1, retains the user prompt, and persists
  `Run cancelled by user before completion.` Headless provider errors no longer synthesize the
  interactive recovery prompt.
- Proof: live PTY exit 1; exported session retained `FIXTURE-DELAY` plus the truthful notice; invented
  text absent.
- Status: **Closed** by `e7ff63031`.

### GSL-PLAY-2026-005A — non-TTY session removal

- Priority/domain: P1 reliability / CLI correctness
- Evidence before: exit 1 with `not connected` after `The following sessions will be removed:`;
  target remained.
- Repair: default non-TTY invocation exits before stdout or mutation with an actionable error;
  `--yes` / `-y` provides explicit automation behavior.
- Proof: refusal exit 1 and zero stdout bytes with target retained; `--yes` exit 0 with target removed.
- Status: **Closed** by `e7ff63031`.

### GSL-PLAY-2026-005B — non-TTY fork mutated before failure

- Priority/domain: P1 data integrity / CLI correctness
- Evidence before: a fork was copied before the interactive session failed on disconnected stdin.
- Repair: non-TTY `session --resume --fork` refuses before session lookup/copy and names the constraint.
- Proof: refusal exit 1, zero stdout, session count unchanged; interactive fork produced a distinct
  `20260818_51`, accepted `MARKER-FORK-REPAIR`, and the source export did not contain that marker.
- Status: **Closed** by `e7ff63031`.

### GSL-PLAY-2026-006 — ACP EOF dropped queued initialize response

- Priority/domain: P1 reliability / protocol data flow
- Evidence before: initialize followed by EOF exited 0 with empty stdout/stderr.
- Repair: EOF now allows a bounded one-second connection drain; completed connection errors remain
  errors, while a permanently pending connection still terminates within the scenario deadline.
- Proof: live exit 0 with one valid JSON-RPC line (5,237 bytes, id 1); three EOF unit tests passed.
- Status: **Closed** by `e5436dfe6`.

### GSL-PLAY-2026-008 — workspace-less artifact preview capability missing

- Priority/domain: P1 data flow / Desktop correctness
- Evidence before: the Outputs inventory named the assistant-referenced Markdown file but preview
  returned `Renderer file access denied for path outside approved roots`.
- Root cause: `null` session workspace (“explicitly none”) was collapsed with `undefined` (“fall back
  to active workspace”), and the router could not publish an artifact-only capability config.
- Repair: preserve the three-state workspace selection and support a workspace-less config containing
  only canonical, existing, exact user-facing artifact files.
- Proof: six focused suites / 25 tests, typecheck, and `just package-ui` passed. The signed package's
  post-fix renderer did not start because macOS Keychain blocked in `SecItemCopyMatching`; no credential
  prompt was accepted or bypassed.
- Status: **Repaired, partially live-verified** by `0cb1ec2d4`; DT-06/DT-07 remain Blocked until one
  packaged click-through confirms the preview.

## Scenario-library assessment

The library is broad and its 110-card index is internally consistent, but several cards should be
tightened. The playtest did not edit the authoritative cards mid-run.

Recommended updates:

- CH-03: add a headless Ctrl-C branch requiring non-zero/structured cancellation, retained user intent,
  and no synthetic assistant text.
- SE-01: add non-TTY refusal and `--yes` automation branches, including “no stdout before refusal” and
  byte-equivalent session retention.
- AC-02: retain the interactive independence check and add a non-TTY assertion that refusal occurs
  before copying.
- HS-03 and AP-05: explicitly pipe initialize followed immediately by EOF and require the accepted
  response to flush before bounded shutdown.
- DT-06 and DT-07: add a workspace-less CLI-created session, relaunch/import, and macOS
  `/tmp` ↔ `/private/tmp` canonical-alias case; existing referenced deliverables should preview without
  a picker, while missing files stay named and code/config remains gated.

A new scenario file is warranted for the product-shell surface added after the original library:
`19-shell-productization-and-handoff.md`. It should cover shell identity/profile collision, provisioning
readback, runtime-namespace isolation, renderer capability negative space, domain-adapter lifecycle,
handoff integrity, and packaged verification. Those contracts are extensively documented under
`docs/build/shell-productization/` but are not represented as atomic playtest cards.

## Reconciliation with previous reports and TODOs

- `2026-08-15-live-all-scenarios-playtest.md`: GSL-PLAY-2026-005 and 006 were reproduced and are now
  closed above. Its broken-MCP hang was already fixed and remained Pass in this run.
- `2026-08-16-live-all-scenarios-playtest.md`: its non-TTY removal finding was reproduced and closed.
  Its headline `60 Pass · 3 Fail · 47 Blocked` is not used as the baseline because fixture-required
  cards were promoted without retained atomic evidence; this report preserves that evidence conflict
  instead of silently treating the count as comparable.
- `docs/TODO.md`: no selected playtest finding was an existing TODO row, so no row was deleted or
  fabricated. Deferred architecture/performance/security items remain unchanged.
- The August 18 session-artifact repair record correctly described the intended capability; this run
  found a regression in the workspace-selection data flow, not a change to ADR-0013.

## Validation

Passed:

- `cargo fmt`
- targeted `gosling-cli` tests and full CLI lib test: 245/245 before the added parser test; added parser
  test passed separately
- ACP EOF tests: 3/3
- focused Desktop tests: 25/25
- Desktop full suite: 810/810 across 112 files
- Desktop `pnpm run typecheck`
- `cargo build`
- `cargo clippy --all-targets -- -D warnings`
- `just package-ui`
- live reruns for cancellation, removal, fork, and ACP EOF

Not fully green:

- `cargo test` reached the auth integration suite after `gosling` 1,823/1,823 unit tests passed, then
  failed 2 of 25 `acp_transport_auth_test` cases. `query_token_is_accepted` and
  `authenticated_acp_router_allows_packaged_desktop_null_websocket_origin` expect query-string tokens
  to pass (406), while current `auth.rs` deliberately rejects `?token=` and its unit test pins 401.
  The isolated serial rerun reproduced both failures. This is pre-existing stale test-contract drift,
  unrelated to the five repairs; it remains open as test hygiene.
- The final packaged Desktop preview click is blocked by macOS Keychain startup as described above.

## Residual risks and next actions

1. Update the six existing cards and create the shell-productization card set described above.
2. Resolve the two stale ACP auth integration expectations in favor of the active header-only security
   contract, then rerun `cargo test`.
3. Repeat DT-06/DT-07 once the local Keychain prompt can be handled by the operator; do not weaken or
   bypass Keychain to automate the proof.
4. Route `crates/gosling-cli/src/session/mod.rs` and `cli.rs` (both over 2,000 lines) to a dedicated
   source-modularization run; this repair correctly avoided splitting them mid-patch.
5. Continue the 79 Blocked cards with the accounts, clients, long-duration fixtures, and platform
   permissions needed for atomic evidence.

## Final status

`completed_with_partial_verification`

The five selected defect roots are repaired. Four have live post-fix proof; the fifth has regression,
type, package, and contract proof but awaits one Keychain-unblocked packaged click-through. The full
Rust suite is not claimed green because of the two unrelated stale auth-test expectations.

