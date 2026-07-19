# Docker lifecycle defect-repair campaign session log — 2026-07-18

Skill: private catalog `repair-defect-campaign`  
Finding: PROC-DOCKER-001 — detached Docker cleanup can be abandoned at runtime shutdown

## Gate 0 — orientation and safety

- Baseline: `main` at `9c03a99126b996aa668943e79bee849efd817e88`, equal to `origin/main`.
- The untracked `.planning/` tree and `2026-07-18-forty-lens-comprehensive-*` reports were
  identified as independent Gemini audit output and left untouched.
- No commit, push, merge, container deletion, or other remote/external mutation was authorized.
- The repair is limited to Docker-backed extension lifecycle management, agent/server shutdown,
  and the Docker tests that reproduced the leak.
- `extension_manager.rs` and `agent.rs` exceed 2,000 lines, so the campaign applied the smallest
  local lifecycle patch rather than attempting source modularization.

## Root cause and frozen touch set

The two real-Docker tests used `Drop` guards that spawned detached Tokio tasks to run
`docker rm -f`. Test runtime teardown could cancel those tasks before Docker saw the removal. The
same failure pattern existed in production: `Extension::drop` spawned an untracked task to kill a
process started by `docker exec`, and the process-global `AgentManager` was never explicitly
drained before the server canceled its shutdown token and exited its runtime.

Touch set:

- `crates/gosling/src/agents/container.rs`
- `crates/gosling/src/agents/extension_manager.rs`
- `crates/gosling/src/agents/agent.rs`
- `crates/gosling/src/execution/manager.rs`
- `crates/gosling-server/src/state.rs`
- this session log

## Repair

- Docker process termination is bounded to three seconds. Normal lifecycle paths await it;
  unexpected final drops use a bounded synchronous fallback that does not depend on a live Tokio
  runtime.
- Extension add, replace, remove, and manager shutdown are serialized. Ordinary replacement keeps
  the old client live until the new client starts successfully. Docker-backed replacement stops
  the old process first because argv-based process identity cannot safely distinguish overlapping
  identical generations.
- Agent removal and whole-manager shutdown explicitly drain extension managers. LRU eviction
  awaits cleanup when the cache owns the final agent reference.
- Agent-manager shutdown cancels active work, blocks late agent creation, drains all cached agents
  concurrently, and clears lifecycle state before server shutdown completes.
- Server shutdown now aborts extension loaders and drains the process-global agent manager before
  canceling the server token, keeping the runtime alive until cleanup completes.
- Docker test containers use an explicitly awaited removal on success and a bounded synchronous
  fallback during panic unwinding. No test teardown relies on detached work.

## Regression and adversarial review

- Real-Docker coverage verifies awaited extension removal, whole-extension-manager shutdown, and
  runtime-independent drop cleanup while preserving the shared container.
- Agent-manager coverage verifies cancellation, agent/cache draining, lifecycle-map cleanup, and
  rejection of late creation after shutdown begins.
- Adversarial review found a replacement-availability regression in the first patch: a failed
  non-Docker replacement removed the old client. The full library suite caught it. The repair now
  preserves the old client for transports that can overlap safely while retaining stop-before-start
  for Docker-backed replacements.
- A shutdown race was closed by gating agent creation: cleanup cannot drain the cache and then lose
  a late-created agent before runtime exit.
- Hard `SIGKILL` of the backend cannot run in-process cleanup. The desktop's normal SIGTERM path and
  parent-death supervisor use the repaired graceful shutdown; a direct backend SIGKILL remains an
  operating-system limitation.

## Validation

- `cargo fmt --all -- --check` — passed.
- `git diff --check` — passed.
- Focused container tests — passed, 6 tests.
- Focused real-Docker extension lifecycle test — passed.
- Focused agent-manager shutdown test — passed.
- First full `cargo test -p gosling --lib -- --test-threads=1` — 1,507 passed and 1
  replacement-availability regression failed; the regression was repaired.
- Corrected full `cargo test -p gosling --lib -- --test-threads=1` — passed, 1,508 tests.
- `cargo test -p gosling-server` — passed, 31 library tests, 31 binary tests, 3 TLS tests,
  and doc tests.
- `cargo clippy -p gosling -p gosling-server --all-targets -- -D warnings` — passed.
- Docker census before and after corrected test runs — unchanged at the same 10 historical
  `gosling-res003-*` containers; the campaign added zero orphans and did not delete old containers.
- Final host-process census — no zombies and no repository `target/` Gosling processes. The
  installed Gosling application retained its expected launchd-owned root, helper children, and one
  backend child.

Logical checkpoint only; no commit or remote mutation performed.
