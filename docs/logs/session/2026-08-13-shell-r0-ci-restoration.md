# 2026-08-13 Gosling shell R0 CI-restoration checkpoint

- Task: implement R0 from the project-shell readiness plan end to end before beginning R1.
- Catalog: the private agent-skills catalog returned no matching reusable workflow; the GitHub CI repair workflow governed remote failure inspection and validation.
- Original failure: main run `31695906352`, Linux job `94433546982`, stopped before Rust tests because GNU `stat -f` prose reached a numeric Bash comparison under `set -u`.
- Implementation: numeric GNU-first/BSD-fallback size and mtime probes; hostile fake-`stat` coverage; serialized source-clean profile verification; autocommit conditional tool completion; refreshed Anthropic weather replay input/key.
- Successive evidence: failed runs `31729883101`, `31730510908`, and `31731280500` exposed independent profile-race, SQLite-lock, replay-drift, and test-harness portability defects after the original failure was removed.
- Local validation: helper matrix and syntax; Rust format and Clippy; full Gosling agent integration 17/17; focused concurrent-tool regression 10 consecutive passes; tool-operation storage 2/2; focused Anthropic replay; shell profile/package 41/41 with clean source readback; `git diff --check`.
- Remote validation: PR run `31731952749` / Linux job `94554362098` passed helper, prepare, and full Rust tests at `436c846f0`; merged-main run `31732990062` / Linux job `94557761229` repeated those passes at `3feffca7c`.
- Merge: PR #47 merged to `main` as `3feffca7c86c7f429b65ee749b8596e5ff4b3d9d`.
- Gate decision: R0 GO and milestone M0 baseline healthy. R1 architecture is next; R2–R8 and project-shell consumer readiness remain pending.
- Historical evidence: Gate 4 remains process-boundary acceptance only. Renderer crash, ACP reconnect/compacted resume, forced cleanup, and the typed failure matrix are reopened under R3/R6.
- Scope: no named product profile, domain prompt/adapter, project renderer, branding, release destination, signing, publication, upload, or updater activation.
- Test ledger: no `docs/testing/test-ledger.yaml` or equivalent required ledger exists.
