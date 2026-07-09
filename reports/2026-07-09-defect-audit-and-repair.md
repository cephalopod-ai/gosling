# Defect audit and repair - 2026-07-09

Scope: full repository, fresh audit run following on from `reports/2026-07-06-fresh-comprehensive-audit.md` and `reports/2026-07-06-defect-campaign-log.md`. Findings F-001 through F-005 from the 2026-07-06 audit were confirmed still fixed and are not re-reported here.

Approach: parallel audits across four areas (server/security/plugins, providers/agent-core/MCP/SDK, CLI/desktop-UI/CI workflows, plus deeper follow-up passes on subagent orchestration, extension lifecycle, and process/resource lifecycle) fed into a single triage-and-repair pass. Given the volume of findings this run surfaced, only obvious, low-risk, well-scoped defects were repaired; higher-risk concurrency/architectural findings were documented but deferred (see "Deferred findings" below) rather than repaired under time pressure.

## Repairs

### R1: Unauthenticated PR comment could trigger privileged desktop-app builds

Severity: Critical. Confidence: High.

`.github/workflows/pr-comment-bundle-intel.yml` and `.github/workflows/pr-comment-bundle-windows.yml` ran `.bundle-intel`/`.bundle-windows` builds of a commenter's own PR head SHA as soon as *any* GitHub user commented the trigger phrase, with no write-access check. The Windows variant grants `id-token: write` (Azure Trusted Signing OIDC) to that build even with `signing: false`. The sibling workflows `pr-comment-bundle.yml` and `pr-comment-build-cli.yml` already gate on `getCollaboratorPermissionLevel` (`admin`/`maintain`/`write`) before checkout, citing `GHSA-4h72-4h3w-4587`/`GHSA-mqm8-hhf6-wvjq`; the intel/windows workflows had regressed from that pattern.

Fix: added the identical "Verify commenter permissions" `actions/github-script` gate to both files, wired the same way (`continue` output now reflects `authorized`, and `Run command action`/`Checkout code`/`Get PR head SHA` steps are all conditioned on it).

Files: `.github/workflows/pr-comment-bundle-intel.yml`, `.github/workflows/pr-comment-bundle-windows.yml`.

### R2: Desktop app CSP, permission handler, and Origin override never applied to real app windows

Severity: High. Confidence: High.

`ui/desktop/src/main.ts` creates both app windows with `webPreferences.partition: MAIN_WINDOW_SESSION_PARTITION` (`'persist:gosling'`, a distinct `Session` object). But the CSP-injecting `webRequest.onHeadersReceived`, the media-permission `setPermissionRequestHandler`, and the `Origin` header override in `webRequest.onBeforeSendHeaders` were all registered on `session.defaultSession` — a session no window in the app actually uses. This silently made the CSP hardening (`script-src 'self'`, restricted `connect-src`/`frame-src`, `object-src 'none'`, etc.) a no-op for the real browsing session, and left Electron's default (auto-grant) permission behavior in effect for camera/microphone/geolocation/notifications requested by content in that session — including MCP App iframes rendered inside the main window.

Fix: changed all three handlers to register on `session.fromPartition(MAIN_WINDOW_SESSION_PARTITION)`, matching the pattern already used correctly by `installBackendCertificateVerifier`.

File: `ui/desktop/src/main.ts:2423`, `:2430`, `:2460` (pre-patch line numbers).

### R3: Regex alternation precedence bug caused false-positive/negative security classification

Severity: Medium-High. Confidence: High.

`crates/gosling/src/security/patterns.rs` — the `python_remote_exec` and `container_escape` threat patterns wrote `A.*B|C` without grouping. Since `|` has the lowest precedence in regex, this parsed as `(A.*B) | C`, not `A.*(B|C)`:

- `container_escape`: `r"(chroot|unshare|nsenter).*--mount|--pid|--net"` matched any text containing the bare substring `--pid` or `--net` standalone (verified: `curl --netrc https://example.com` matched via the `--net` substring inside `--netrc`, with no chroot/unshare/nsenter present).
- `python_remote_exec`: `r"python[23]?\s+-c\s+.*urllib|requests.*exec"` matched any text containing `requests` followed later by `exec`, independent of `python -c` (verified: `"the requests package supports async and can execute callbacks"` matched).

Fix: added explicit grouping — `(chroot|unshare|nsenter).*(--mount|--pid|--net)` and `python[23]?\s+-c\s+.*(urllib|requests).*exec` — and added regression tests (`container_escape_requires_both_sides_of_alternation`, `python_remote_exec_requires_both_sides_of_alternation`) covering both the intended matches and the previously-false-positive inputs.

File: `crates/gosling/src/security/patterns.rs`.

### R4: InlinePython extension dependencies bypassed the OSV malware scan

Severity: High (security enforcement gap). Confidence: High.

`crates/gosling/src/agents/extension_manager.rs`'s `ExtensionConfig::Stdio` branch calls `extension_malware_check::deny_if_malicious_cmd_args` before spawning an `npx`/`uvx` process. The `ExtensionConfig::InlinePython` branch builds an equivalent `uvx --with mcp --with <dep> ... python <file>` command from user/attacker-supplied `dependencies`, but never checked those dependencies against OSV at all — the only call site of the malware check in the crate was the `Stdio` branch.

Fix: added `extension_malware_check::deny_if_malicious_pypi_dependencies`, which parses each entry in `dependencies` with the existing PyPI token parser and queries OSV per-package (mirroring `deny_if_malicious_cmd_args`'s single-package check, but covering the full dependency list instead of only the first non-flag arg). Wired it into the `InlinePython` branch before the temp file/command is built.

Files: `crates/gosling/src/agents/extension_malware_check.rs`, `crates/gosling/src/agents/extension_manager.rs`.

### R5: Zombie child process on login-shell PATH resolution timeout

Severity: Low-Medium (resource leak). Confidence: High.

`crates/gosling/src/agents/platform_extensions/developer/shell.rs`'s `resolve_login_shell_path`: on the timeout branch (`rx.recv_timeout` times out because the user's shell profile takes >5s, e.g. slow `nvm`/`rbenv` init), the code called `child.kill()` but never `child.wait()`. `Child::kill()` sends `SIGKILL` but does not reap the process, and `Child` has no `Drop` impl that waits — so the process was left as a zombie until gosling itself exits. This runs once per `ShellTool::new(true)` (agent/session startup with `use_login_shell_path`), so each slow-shell-profile session start leaked one zombie.

Fix: added `let _ = child.wait();` immediately after the kill on the timeout path.

File: `crates/gosling/src/agents/platform_extensions/developer/shell.rs`.

### R6: Inverted success/failure return from `open-directory-in-explorer`

Severity: Low. Confidence: High.

`ui/desktop/src/main.ts`'s `open-directory-in-explorer` IPC handler wrapped `shell.openPath()`'s result in `!!(...)`. `Electron.shell.openPath()` resolves to an empty string on success and a non-empty error message on failure, so `!!result` reported success as `false` and failure as `true` — inverted. The three current call sites (`DirSwitcher.tsx`) discard the boolean today, so this had no observable effect yet, but any caller that branches on the return value would see backwards behavior.

Fix: changed the handler to return `errorMessage === ''` instead of `!!errorMessage`.

File: `ui/desktop/src/main.ts`.

### R7: Missing least-privilege `permissions:` block on two workflows

Severity: Low (defense-in-depth). Confidence: Medium.

`.github/workflows/pr-smoke-test.yml` and `.github/workflows/check-release-pr.yaml` had neither a top-level nor job-level `permissions:` block, unlike every other workflow in the repo, which explicitly scopes `GITHUB_TOKEN`. Both workflows only use `actions/checkout`, `actions/download-artifact`, and similar read-only actions, so `contents: read` is sufficient. `pr-smoke-test.yml` is already gated against forks via a `check-fork` job condition, so this was a defense-in-depth gap rather than a directly exploitable one.

Fix: added `permissions: contents: read` to both workflows.

Files: `.github/workflows/pr-smoke-test.yml`, `.github/workflows/check-release-pr.yaml`.

## Verification

Passed:

- `source bin/activate-hermit && cargo check -p gosling`
- `source bin/activate-hermit && cargo fmt --all` (no additional diffs beyond the changes above)
- `source bin/activate-hermit && cargo test -p gosling --lib -- security::patterns platform_extensions::developer::shell extension_malware_check extension_manager` — 61 passed, 0 failed, including two new regression tests
- `source bin/activate-hermit && cargo clippy -p gosling --all-targets -- -D warnings`
- `git diff --check`
- `python3 -c "import yaml; ..."` — all four edited workflow YAML files parse
- `cargo check --workspace --all-targets` (background, pre-existing baseline) fails only on `v8-goose`'s build script trying to download a static V8 binary, which is blocked by this sandbox's network policy (HTTP 403) — an environment limitation unrelated to these changes, not a regression.

Not run / residual verification limits:

- `pnpm run typecheck` / `pnpm install` for `ui/desktop` could not complete: this sandbox's network policy blocks `codeload.github.com` (used to fetch a native dependency's source tarball), so `node_modules` could not be installed. The `main.ts` changes (R2, R6) were verified by direct code reading, Electron API documentation for `shell.openPath`/`session.fromPartition`, and `prettier --write` (which parses and reformats the file, confirming it is syntactically valid TypeScript) — but no `tsc`/`eslint`/unit-test run was possible this session.
- No desktop UI playtest was performed.
- Full workspace `cargo test` was not run (only targeted tests for touched modules), due to the `v8-goose` build blocker above making a full workspace build infeasible in this sandbox.

## Deferred findings (not repaired this run)

These were verified as real by the audit but were judged too architecturally invasive, too concurrency-sensitive, or too low-severity/speculative to fix safely in this obvious-work pass. Recorded here for a future dedicated repair session.

- **Plugin trust keyed by filesystem path, not content; `--auto-update` silently swaps plugin content in place** (`crates/gosling/src/plugins/discovery.rs`, `crates/gosling/src/plugins/mod.rs`). Trust is persisted per directory path and never re-derived from current contents; an auto-updating plugin's git remote can push new hook/MCP-server content after the user's initial trust grant, and it runs automatically on the next discovery cycle with no re-approval. Partly an accepted trade-off of the opt-in `--auto-update` feature. Medium confidence.
- **`egress_inspector.rs`'s `is_web_tool` has no `__http_request` namespaced-suffix rule** (unlike the F-002 fix for shell tools), so a hypothetical namespaced HTTP tool would bypass egress inspection. No current extension uses that tool name — speculative, not verified exploitable.
- **Symlink-following path traversal in the Electron renderer file-access sandbox** (`ui/desktop/src/main.ts` `assertRendererFileAccess`, used by `read-file`/`write-file`/`delete-file`/`ensure-directory`/`list-files`). Only does a lexical containment check, never resolves symlinks, unlike the directory-picker flow which explicitly rejects them. A symlink inside a trusted project root (e.g. `.goslinghints` pointing at `~/.ssh/id_rsa`) would be followed transparently. High confidence; deferred because a correct fix (resolve-then-check for 5 call sites) needs UI-level verification we couldn't perform (TS toolchain blocked, see above).
- **Windows `cmd.exe` command injection via `open-in-chrome`** (`ui/desktop/src/main.ts`, `openInChrome`). Unescaped shell metacharacters in a URL passed to `cmd.exe /c start` could break out on Windows. Currently dead code — not called from any renderer path. Medium confidence, low current risk.
- **`SummonClient::drop` silently skips cancellation of in-flight background subagents under lock contention** (`crates/gosling/src/agents/platform_extensions/summon.rs:334`, uses `try_lock()`). A correct fix likely requires changing `background_tasks` from `tokio::sync::Mutex` to a synchronously-lockable type across ~10 call sites, and is entangled with the next item.
- **A concurrent `peek` holds the `background_tasks` lock across an `.await`** on a per-task notification buffer (`summon.rs` ~lines 682-691), widening the critical section that the Drop fix above depends on. No proven deadlock today; latent risk.
- **`handle_load_task_result`'s "wait for task" path falsely reports a running task as "not found"** for up to 300s if a concurrent `load(task_id)` call race occurs (`summon.rs` ~line 760).
- **MCP handshake has no timeout** (`crates/gosling/src/agents/mcp_client.rs` `connect_with_container`): the `timeout` field is stored but never applied to the initial `client.serve(transport).await` call, only to later RPCs. A hung/malicious MCP server can block `add_extension` (and thus session startup) forever. High confidence, moderate fix complexity (needs `tokio::time::timeout` wrapping plus cleanup of the still-alive child process on timeout).
- **Blocking synchronous subprocess wait inside async tool handlers** (`crates/gosling-mcp/src/subprocess.rs:71-72`, `resolve_login_shell_path`): runs on the calling tokio worker thread with no `spawn_blocking`/timeout, unlike the equivalent logic in `shell.rs` (which this crate was "ported from" and which does wrap it correctly). High confidence.
- **`output_capped` hangs indefinitely on backgrounded child processes** (`crates/gosling-mcp/src/computercontroller/mod.rs:81-126`): a script that backgrounds a grandchild without redirecting output leaves the pipe write-end open past the direct child's exit, so `drain()` never sees EOF and the tool call hangs forever with no timeout. High confidence, needs a proper process-group-kill or read-timeout design.
- **`dispatch_tool_call`'s availability re-check can be silently skipped** if an extension is removed in the narrow window between `resolve_tool` and the check (`extension_manager.rs:1852-1867`). Low confidence/severity — the underlying MCP client call is the real trust boundary.
- **Frontend tool response can be attached to the wrong tool-call ID** via a stale dequeue from the shared, unvalidated `tool_result_rx` channel (`crates/gosling/src/agents/tool_execution.rs:188-194`), if a stale/late `handle_tool_result` call lands after its originating turn was cancelled. Medium confidence — requires a specific cancellation race.
- **Output-slot file collision under high shell-call concurrency** (`crates/gosling/src/agents/platform_extensions/developer/shell.rs`, `OUTPUT_SLOTS = 8`): 9+ concurrent shell calls can collide on the same on-disk truncated-output file. Medium confidence, requires unusually high concurrency.
- **`file_timestamp_manipulation` pattern has the same un-grouped-alternation shape** as R3 (`touch\s+-[amt]\s+|utimes|futimes`), but low severity/impact and ambiguous intent (may be deliberately matching bare `utimes`/`futimes` syscall mentions) — not changed.
