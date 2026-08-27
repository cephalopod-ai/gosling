# Test ledger

| Test area | Command / evidence | Last known result | Coverage meaning | Gaps |
|---|---|---|---|---|
| Rust formatting | source bin/activate-hermit && cargo fmt --check | passed | Rust style is clean. | Does not validate behavior. |
| Rust lint | source bin/activate-hermit && cargo clippy --all-targets -- -D warnings | passed | All-target clippy is clean. | Does not include external release artifacts. |
| Rust core baseline | source bin/activate-hermit && cargo test -p gosling --lib | passed: 1,888 tests on 2026-08-27 | Core library behavior, including durable session leases and import quarantine, passes after this campaign. | External services and installed artifacts are separate evidence. |
| Hints regression | cargo test -p gosling hints::load_hints --lib | passed: 26 tests | Confirms removal of the noisy test print did not affect hint behavior. | Focused scope. |
| Desktop typecheck | cd ui && node_modules/.bin/tsc --noEmit --project desktop/tsconfig.json | passed | Desktop TypeScript is type-correct in this checkout. | No packaged-app validation. |
| Gemini OAuth UI | Desktop Vitest provider-modal and ACP-error tests | passed: 10 tests | Preserves provider-specific ACP error detail in the modal. | No live account sign-in. |
| Documentation typecheck | source bin/activate-hermit && cd documentation && npm run typecheck | passed on 2026-08-27 | Documentation config, theme, and component TypeScript are clean. | Does not exercise production link resolution. |
| Documentation tests | source bin/activate-hermit && cd documentation && npm test | passed: 16 tests | Docs mapping and Goose compatibility conversions pass. | Focused script coverage. |
| Documentation production build | source bin/activate-hermit && cd documentation && npm run build | passed: 165 Markdown pages exported | Docusaurus compiles both client/server and rejects broken links. | Generated site was not deployed. |
| Shell profile/conformance | source bin/activate-hermit && cd ui/desktop && pnpm run shell:test-profile && pnpm run shell:check-profiles | passed: 67 tests and 3 profile checks | Shell manifest, package-verifier, workflow, lifecycle, and consumer contracts pass locally. | Remote cross-platform rerun is pending. |
| Windows Rust warning repair | cargo check --target x86_64-pc-windows-msvc -p gosling -p gosling-mcp | blocked after target install by missing Windows C headers in the macOS cross environment | Rust target resolution progressed into native dependency builds; the exact CI cfg warnings were patched. | Requires a Windows runner for authoritative confirmation. |
| Rust campaign build/tests/lint | source bin/activate-hermit && cargo build && cargo test --workspace && cargo clippy --all-targets -- -D warnings | passed on 2026-08-27 | The complete workspace, integration/doc tests, updated dependency graph, and all-target Clippy are clean. | Platform-specific remote jobs remain separate evidence. |
| Rust dependency posture | cargo-deny check advisories && cargo-machete | passed on 2026-08-27 | Current advisories and unused direct dependencies are clean; the obsolete RUSTSEC-2023-0071 exception is gone. | Does not replace secret/history scanning. |
| Desktop campaign validation | source bin/activate-hermit && cd ui/desktop && pnpm run typecheck && pnpm test -- --run | passed: 134 files, 1,071 tests | Desktop types and unit/integration behavior, including MCP App message authority and Research seat eligibility, pass. | No packaged release artifact. |
| OIDC proxy identity | source bin/activate-hermit && cd oidc-proxy && pnpm test | passed: 11 tests | Canonical repository claims and allowlist behavior pass. | Cloudflare runtime fell back to its latest supported compatibility date in the local harness. |
| Fresh-clone build | Not run | skipped | No stranger-machine evidence. | Requires a separate scratch-clone drill. |
| Documentation dependency audit | cd documentation && npm audit --package-lock-only | open: 25 transitive advisories (19 high, 6 moderate) | Current npm lockfile audit is recorded. | Remaining roots have no compatible complete automated fix. |
