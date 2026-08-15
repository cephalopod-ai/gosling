# 2026-08-15 Default Shell DS-7 corrective closure

## Task

Re-evaluate the merged Gosling foundation and continue corrective patching until a Default Shell
GUI GO recommendation can be supported without relying on stale or partial evidence.

## Files changed

- bounded shell credential discovery and selected-profile re-resolution in the Rust ACP server;
- bounded, phase-reported Electron ACP preflight and immediate pre-session credential refresh;
- live credential pinning, settings interruption/permission, and timeout regression tests;
- Default Shell architecture, traceability, risks, defects, plan-change, build-state, and DS-7
  acceptance records.

## Validation run

- `cargo fmt --all -- --check` — passed;
- `cargo clippy --all-targets -- -D warnings` — passed;
- `cargo test -p gosling --lib shell_validation` — 8/8 passed;
- `cargo test -p gosling-cli --test shell_runtime_e2e_test` — 6/6 passed;
- Desktop TypeScript check — passed;
- Desktop shell tests — 18 files, 169/169 passed;
- profile/consumer/package tests — 57/57 passed;
- Default Shell macOS arm64 package and independent readback — passed, binary hash
  `38c0154cad71f5bb3a924d1bc835a00e970c24383602a6465265cda217cd4fd6`;
- actual packaged Electron renderer/preload/backend replay — reached `ready`, compatible, with no
  provisioning issues and credential catalog safely unavailable on the unsigned host;
- full Gosling plus two neutral shell identities — concurrent isolation and cleanup observed.

## Risks and follow-ups

- The work is isolated as one clean candidate revision and all fixture profiles report
  `sourceClean:true`; the final handoff must bind mandatory green CI to that candidate's exact head
  SHA because the green merged-main base run is historical evidence only.
- Cross-platform package repetition, signing, notarization, publication, and named shells remain
  outside this Default Shell GUI gate.

## Workflow/data-flow corrective audit addendum

The later pre-GUI walkthrough closed consumer callback-capability, compacted-resume history,
current-directory session discovery, bounded transcript repair, structured permission/form
projection, stable recovery errors, domain-confirmation mirroring, and private-reasoning projection
gaps. A fresh defensive pass also made main independently filter returned session summaries to the
accepted directory, bounded process-lifetime interaction replay IDs, and preserved pending
permissions/forms across nonterminal streamed progress.

Validation after the addendum:

- exact-final full Rust workspace under an isolated Gosling config root — passed with 1,773 core
  tests plus every integration and doc-test suite;
- Desktop tests — 110 files, 795/795 passed;
- Desktop typecheck, ESLint, and i18n checks — passed;
- shell profile/consumer/scaffold/package-script tests — 57/57 passed;
- current macOS arm64 package/readback — passed; profile hash
  `830f6143a45ea309c42f03cb440410b3eb6484009c86cda4aa98f0a7e1282950`, binary hash
  `75790e0e489e7c589cb7880750df344452fff331d399fac30f61178fef780ca5`.

The first unisolated Rust run had one environment-sensitive failure because the operator's global
summarizer setting is `on`; the isolated run passed, and no unrelated summarizer behavior was
changed. This package was built before the corrective candidate was committed, so it is behavioral
evidence only. Clean-source package/readback, mandatory CI on the exact corrective commit, and
explicit operator GO remain required before the GUI plan or implementation begins.

The observed Gemini provider OAuth failure (`OAuth login failed: Internal error`) is recorded in
`docs/TODO.md` and `defects.md` only. It was not investigated or modified.

The addendum's packaged replay then found two more high-severity lifecycle defects. Closing the
window destroyed renderer contents before the final `stopping` notification, so `send` threw,
cleanup aborted, and the backend became an orphan. The dead record also survived the next launch
because generic shell startup did not invoke the existing registry reconciler. Event publication
now drops destroyed/racing sends, and `createMinimalShellHost` reconciles the product-local registry
before spawn or fails closed. The observed orphan was terminated by its exact PID. A rebuilt package
then reached verified `ready`, pruned the stale record, terminated the new backend with code 0 on
window close, left no matching process, and left `backend-processes.json` with `processes: []`.

A subsequent nonvisual packaged preload probe exercised the newly added session surfaces directly:
restored canonical directory, bounded list, stale-generation failure, create, invalid-prompt
rejection, transcript read, detach, direct resume, and compacted-history replay all passed. No model
prompt was sent. `session/list` correctly omitted the newly created zero-message session because the
ACP server deliberately lists only sessions with messages; direct resume proved the session itself
remained available. The two empty probe sessions were removed with Gosling's session command scoped
to the Default Shell runtime namespace, and the temporary settings/workspace/probe files were
removed. The packaged app left no shell backend process and an empty process registry.

Reviewing that packaged path exposed SHP-DEF-048: the picker/list filtered to the accepted working
directory, but a renderer-supplied known session ID could still reach `loadSession` using the
session's different stored directory. Resume now receives main's accepted canonical directory and
compares it with the bounded `session/info` result before loading. A mismatch returns the stable
`SESSION_UNAVAILABLE` recovery and never calls `loadSession`; focused unit and live managed-backend
regressions pass. The integration fixture was also brought back into conformance with the consumer
callback declarations and structured elicitation projection added by this corrective patch.

The package was rebuilt again after the resume fix. A real packaged-preload probe created and
detached an empty session under temporary directory A, restarted with main restoring temporary
directory B, and attempted the known A session ID. The package returned only
`SESSION_UNAVAILABLE`, disclosed no A path, left `session: null`, shut down without an orphan, and
left an empty process registry. The exact empty probe session was then removed with the product
session command, and all temporary settings, scripts, directories, and scoped cleanup logs were
removed; a database check found no remaining probe sessions.

A deeper Rust activation review then exposed SHP-DEF-049. Electron main's new check protected the
packaged preload path, but direct ACP `loadSession` could still supply a different directory for a
workspace-less shell session, and activation reused an older persisted extension selection after
current provisioning removed it. Shell activation now canonicalizes and requires the stored
directory, revalidates current provisioning, and rebuilds the current extension/skill selection
before loading. The shell-runtime E2E suite is now 7/7: its new regression creates a session with a
developer shell, restarts with an empty extension allowlist, rejects a different directory with
`SHELL_SESSION_DIRECTORY_MISMATCH`, and loads the stored directory with no tools or enabled
extensions. No GUI code was added.

A final read-through found that omitted current `skillIds` would otherwise preserve the older
persisted selection. Shell activation now removes that obsolete state when the current provisioning
omits it. The 7/7 shell-runtime E2E regression covers this clearing behavior, and Clippy passes with
warnings denied.

Two subsequent full-workspace attempts—parallel and serial under fresh isolated roots—stalled when
workspace initialization migrated global provider credentials. Stack sampling proved the test
thread was blocked in macOS Keychain decryption, not the mock server or shell code. Unit-test builds
had selected production system-keyring storage even though the data root was isolated. Both default
and custom unit-test `Config` instances now force file-backed secrets; production builds retain the
existing keyring policy. A `system-keyring`-enabled regression passes, the formerly blocked test
completes without an override, and the exact-final full workspace passes—1,773 core tests plus every
integration and doc-test suite—with `GOSLING_DISABLE_KEYRING` explicitly absent.

The macOS arm64 package was rebuilt after SHP-DEF-051. Independent readback matched profile hash
`830f6143a45ea309c42f03cb440410b3eb6484009c86cda4aa98f0a7e1282950` and new embedded-binary hash
`75790e0e489e7c589cb7880750df344452fff331d399fac30f61178fef780ca5`. A real packaged-preload
smoke reached compatible `ready`, exposed only `core:session`, and closed normally. The backend
exited with code 0, no matching process remained, and the product process registry was empty.

An exact-source replay then exposed SHP-DEF-051. Although the credential-catalog timeout returned
control, stack sampling showed its blocking worker still inside macOS Keychain decryption, so Tokio
runtime shutdown survived `SIGTERM` until Electron escalated to `SIGKILL`. Managed Electron
backends now enforce noninteractive protected-store access, and main gives the ACP transport a
bounded close interval before backend termination. On the same host and Keychain state, the rebuilt
package reached `ready` in 23 ms with credential catalog status `available`, exposed only
`core:session`, stopped in 6 ms, exited the backend with code 0, left no matching shell process, and
left the product process registry empty.

The post-patch interaction audit then exposed SHP-DEF-052. Session progress updates were incorrectly
treated as terminal cleanup, so `tool_call_update` could clear a pending permission or form before
the operator responded. Runtime cleanup is now limited to completed, cancelled, and failed session
outcomes. Focused regressions hold an ACP permission across streamed tool progress and a form across
streamed agent content, then resolve each exact action explicitly; the full Desktop suite passes
795/795. No renderer or named-shell GUI was added.
