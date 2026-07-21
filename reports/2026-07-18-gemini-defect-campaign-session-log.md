# Gemini defect-repair campaign session log — 2026-07-18

**Campaign skill:** `repair-defect-campaign`.  
**Plan:**
[`2026-07-18-gemini-defect-campaign-plan.md`](2026-07-18-gemini-defect-campaign-plan.md).  
**Branch:** `main`; no commit or push authorized.  
**Baseline:** `5f5a3acde8a671199e5d384df2fddd9a8aa35066`.

## Gates 0–2

Status: complete before source edits.

- Validated all 12 supplied candidates against the current repository rather than
  accepting the consolidated summaries at face value.
- Confirmed three localized defects initially, rejected eight stale or incorrect
  claims with source/test evidence, and retained one already-routed architectural
  residual. A post-repair adversarial pass then confirmed GEM-010 in adapters other
  than the incorrectly cited OpenAI definition.
- Frozen four locality groups: desktop external URL opening, structured path
  discovery, unique shell-output files, and provider error classification.
- Baseline validation:
  - concurrent extension-state merge: passed (1 test);
  - immediate-lock contention regression: passed (1 test);
  - concurrent session creation: passed (2 tests);
  - working-directory scope inspector: passed (16 tests);
  - provider HTTP classification: passed (20 tests);
  - provider Rust/TypeScript drift guard: passed (1 test);
  - desktop tests: passed (73 files, 516 tests).

## Stage record

### Stage 1 — GEM-001: desktop external-URL IPC

Status: repaired; campaign-wide verification pending.

- Replaced the platform command launch with Electron `shell.openExternal` after
  normalizing an HTTP(S)-only URL. The existing preload/IPC contract remains
  compatible, while Windows no longer receives renderer data through `cmd.exe`.
- Added `normalizeWebUrl` regression tests for shell-metacharacter-bearing URLs,
  blocked protocols, malformed input, and non-string IPC payloads.
- Targeted validation: `pnpm exec vitest --run src/utils/urlSecurity.test.ts` —
  passed, 2 tests.
- Adversarial review: the renderer value is now passed only as one URL string to
  Electron's external-opening API; there is no platform shell, command flag, or
  executable-name interpolation in this handler.

### Stage 2 — GEM-003: structured path discovery

Status: repaired; campaign-wide verification pending.

- Replaced the fixed top-level key list with recursive argument traversal. Path
  semantics are recognized across snake_case, kebab-case, and camelCase keys for
  files, paths, directories, folders, roots, and working directories.
- Explicit absolute, parent-relative, tilde, `$HOME`, file-URI, and Windows-drive
  path shapes are detected even under unfamiliar aliases. Structured text payloads
  remain exempt from path guessing, and the shell parser now also recognizes paths
  in option assignments and redirections.
- Added regression coverage for nested aliases, arrays, unknown-key traversal,
  text-payload false positives, file URIs, shell home expansion, option assignments,
  and redirections.
- Targeted validation: `cargo test -p gosling --lib
  working_dir_scope_inspector::tests` — passed, 22 tests.
- Adversarial review: object recursion resets inherited array semantics so a
  `files: [{ content: ... }]` payload does not classify content as a path; traversal
  and symlink checks still use the existing canonical, fail-closed boundary.

### Stage 3 — GEM-008: unique shell-output files

Status: repaired; campaign-wide verification pending.

- Removed the eight-slot modulo. Stdout, stderr, and interleaved truncation files now
  use the same unique monotonic call ID, so later commands cannot overwrite an output
  path already returned to the model or renderer.
- The existing per-tool temporary directory still owns cleanup; the repair does not
  make output files persistent beyond the shell tool's lifecycle.
- Replaced slot-cycling tests with regressions proving IDs remain unique past eight,
  32 earlier outputs retain their exact content, and 32 concurrent allocations are
  distinct.
- Targeted validation: `cargo test -p gosling --lib
  agents::platform_extensions::developer::shell::tests` — passed, 16 tests.
- Adversarial review: all three output labels share one call ID but retain distinct
  stream prefixes; concurrent allocation remains atomic and no filename is derived
  from command content.

### Stage 4 — GEM-010: provider error classification

Status: repaired; campaign-wide verification pending.

- `ProviderError::from(reqwest::Error)` now preserves status semantics for
  authentication, credit exhaustion, context limits, rate limits, and server errors
  instead of collapsing every status-bearing error into `RequestFailed`.
- Model discovery for LiteLLM, Ollama, OpenRouter, NanoGPT, Tetrate, Databricks v1/v2,
  GitHub Copilot, and Kimi now preserves typed transport errors and classifies HTTP
  failures before parsing success JSON. Ollama's intentional 404-to-static-catalogue
  fallback remains unchanged.
- Added central reqwest classification coverage for 401, 429, 503, and 400, plus an
  adapter regression proving Ollama model discovery preserves authentication errors.
- Targeted validation:
  - `cargo test -p gosling-providers --lib errors::reqwest_error_tests` — passed;
  - `cargo test -p gosling-providers --lib fetch_supported_models` — passed,
    15 tests;
  - focused Gosling provider modules — passed, 48 tests (one LiteLLM filter had no
    colocated tests; the module compiled in every subsequent Gosling test build).
- Adversarial review: error bodies remain handled by the shared status mapper,
  network errors are no longer rewrapped as request errors, and provider-specific
  successful-body validation is unchanged.

### Routed — GEM-005: ACP data-root isolation

Status: routed to source modularization; no partial source patch planned.

- Prior campaigns record this as `DEF-002` and `AUD-GOS-011`.
- A correct repair must replace process-global configuration, paths, and request-log
  ownership with explicit per-agent dependencies. Mutating a global root per agent
  would race and leave provider/configuration consumers unresolved.

## Gate 9 — closeout

Status: complete with one routed architectural residual and one pre-existing lint
baseline failure.

### Campaign result

- Repaired GEM-001, GEM-003, GEM-008, and GEM-010.
- Closed GEM-002, GEM-004, GEM-006, GEM-007, GEM-009, GEM-011, and GEM-012 as
  verified stale or incorrect claims, with source and regression evidence in the
  plan.
- Routed GEM-005 to source modularization; no partial global-path workaround was
  introduced.
- No commit or remote mutation was performed.

### Final validation

- `cargo fmt --all -- --check` — passed.
- `cargo test -p gosling-providers` — passed, 424 tests plus doc tests.
- `cargo test -p gosling --lib` — passed, 1,515 tests.
- `cargo test -p gosling-sdk-types` — passed, 9 tests plus doc tests.
- `cargo clippy -p gosling -p gosling-providers -p gosling-sdk-types
  --all-targets -- -D warnings` — passed.
- `pnpm run typecheck` — passed.
- `pnpm test` — passed, 74 files and 518 tests.
- Changed-file ESLint and Prettier checks — passed.
- ACP schema and TypeScript SDK generation — passed when the two recipe commands
  were run directly; `git diff` reported no generated drift.
- `git diff --check` — passed.
- Post-test process census found no surviving Gosling, Vitest, Cargo, schema
  generator, Docker exec, or shell-test background process.

### Existing validation limitation

`pnpm run lint:check` remains red on five pre-existing `no-undef` errors and four
pre-existing React hook warnings in `createWebSocketStream.test.ts`,
`scroll-area.test.tsx`, `Hub.tsx`, and `useNavigationSessions.ts`. None of those
files is touched by this campaign. The changed UI files pass focused ESLint and
Prettier checks.

The `just check-acp-schema` wrapper failed before generation because its recipe ran
from a directory where the embedded `cd crates/gosling` could not resolve. Running
the exact schema generator and TypeScript generator commands directly succeeded and
produced no diff, so GEM-012 is closed as no drift rather than misreported as a pass
of the broken wrapper.

### Files changed

- Desktop URL boundary: `ui/desktop/src/main.ts`,
  `ui/desktop/src/utils/urlSecurity.ts`, and
  `ui/desktop/src/utils/urlSecurity.test.ts`.
- Filesystem scope: `crates/gosling/src/permission/working_dir_scope_inspector.rs`.
- Shell output durability:
  `crates/gosling/src/agents/platform_extensions/developer/shell.rs`.
- Provider classification: `crates/gosling-providers/src/errors.rs`,
  `crates/gosling-providers/src/ollama.rs`, and the LiteLLM, OpenRouter, NanoGPT,
  Tetrate, Databricks v1/v2, GitHub Copilot, and Kimi adapters under
  `crates/gosling/src/providers/`.
- Campaign evidence: this session log and its plan.


## Superseding open-defect repair campaign - 2026-07-20

The later campaign repairs the ACP runtime path boundary identified by GEM-005, removes the recorded Desktop `no-undef` and unstable-hook-dependency debt, coalesces provider inventory startup requests, and makes `check-acp-schema` independent of the caller's working directory.

Status: `completed_with_partial_verification`. Static source repair and regression coverage were recorded, but builds, tests, lint, formatting, schema generation, and Git operations were not run because they were outside this campaign's authorization.

## Final verification supersession - 2026-07-20

The follow-on campaign is complete. Rust formatting/tests/clippy, Desktop typecheck/tests/lint/i18n, and the caller-independent ACP schema wrapper all pass. No open defect remains from GEM-005 or the frozen open-defect inventory; Git publication was not part of this verification.
