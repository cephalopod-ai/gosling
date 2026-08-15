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
