# Test ledger

| Test area | Command / evidence | Last known result | Coverage meaning | Gaps |
|---|---|---|---|---|
| Rust formatting | source bin/activate-hermit && cargo fmt --check | passed | Rust style is clean. | Does not validate behavior. |
| Rust lint | source bin/activate-hermit && cargo clippy --all-targets -- -D warnings | passed | All-target clippy is clean. | Does not include external release artifacts. |
| Rust core | source bin/activate-hermit && cargo test -p gosling | passed: 1,687 unit tests plus integration targets | Core crate and included tests passed. | Existing ignored ACP-provider tests remain surfaced by the suite. |
| Hints regression | cargo test -p gosling hints::load_hints --lib | passed: 26 tests | Confirms removal of the noisy test print did not affect hint behavior. | Focused scope. |
| Desktop typecheck | cd ui && node_modules/.bin/tsc --noEmit --project desktop/tsconfig.json | passed | Desktop TypeScript is type-correct in this checkout. | No packaged-app validation. |
| Gemini OAuth UI | Desktop Vitest provider-modal and ACP-error tests | passed: 10 tests | Preserves provider-specific ACP error detail in the modal. | No live account sign-in. |
| Documentation typecheck | source bin/activate-hermit && cd documentation && pnpm run typecheck | failed | Documentation project currently has Docusaurus/TypeScript errors. | Release blocker until repaired and rerun. |
| Fresh-clone build | Not run | skipped | No stranger-machine evidence. | Requires a separate scratch-clone drill. |
| Dependency audit | Not run | unavailable | No current vulnerability count. | Install/use an approved audit tool. |
