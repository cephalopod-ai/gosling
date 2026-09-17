# Session input compilation and size recovery

- Date: 2026-09-16.
- Task: Repair `SHELL_LIBRARY_SELECTION_TOO_LARGE` recovery and enable a complete source compilation for the user's comprehensive report workflow.
- Baseline: clean `main` at `a71d4ccfc`; Rust 2021 workspace, MSRV 1.91.1, installed pinned Rust 1.92, Electron/React Desktop. No dependencies or storage schema changes planned.
- Workflow: catalog `plan-rust-app`, execute mode, explicit local patch authorization. Independent reads/checks are batched; schema generation and consumers remain sequential. Dory state is available under registered name `gosling`; absolute-path lookup was rejected. No prior checkpoint.
- Diagnosis: 512 KiB inline source-text guard; the visible checked inputs alone total 688 KiB. Desktop checks only item count and displays model-switch recovery for input errors.
- Design: preserve inline limits; compile explicitly selected IDs (up to all 128 visible project/session items) to a bounded, private, uniquely named Markdown file in the session working directory. Register it as an Output. Include source index, hashes and intact pasted text/citations. Large selections use a small manifest directing the agent to read all sources in sections. Compilation is source preservation, not a claim that report synthesis is complete. Missing/unextractable inputs or exceeded budgets fail without publishing partial content. Planning policy still gates file creation.
- Baseline validation: focused Desktop library, prompt lifecycle, and ArtifactPane tests: 58 passed.
- Implementation: added a typed ACP compilation method and generated its schema/SDK; added Select all, Clear selection, Compile all inputs and selection byte feedback to Desktop; automatically compile oversized prompt selections; preserve the original message and selection on attachment failure with input-specific retry. Removed silent linked-text truncation. Added four Rust integration regressions and focused Desktop regressions. Updated ADR 0015, the documentation index and i18n catalogs. No dependency, database schema or installed-app changes.
- Validation scope: repository instructions reserve `cargo build`, `cargo test`, and `cargo clippy` for explicit build/test requests. Use formatting, Cargo check of changed targets, schema generation, focused Desktop tests and type/lint checks; identify any unexecuted Rust/runtime acceptance clearly.

## Validation results

All commands used the repository Hermit environment. Rust commands used the installed pinned toolchain with `RUSTUP_AUTO_INSTALL=0`; Cargo generation/check used `DEVELOPER_DIR=/Library/Developer/CommandLineTools` on the macOS host.

- `cargo fmt`: passed.
- `cargo check --locked -p gosling --test acp_input_compilation_test`: passed after the final backend change. This typechecked the real ACP integration test target; it did not execute its four tests.
- From `crates/gosling`, `cargo run --locked --features code-mode,aws-providers,telemetry,otel,rustls-tls,system-keyring --bin generate-acp-schema`: passed; ACP schema and metadata regenerated.
- From `ui/sdk`, `pnpm run generate` and `pnpm run build:ts`: passed; generated client/types/validators match the Rust API.
- From `ui/desktop`, `pnpm exec vitest run src/acp/sessionLibraryInputs.test.ts src/acp/__tests__/errors.test.ts src/acp/__tests__/chatSessionLifecycle.test.ts src/acp/__tests__/chatSessionController.test.ts src/acp/__tests__/chatSessionStore.test.ts src/hooks/useChatSession.test.tsx src/components/artifacts/ArtifactPane.test.tsx src/components/Hub.test.tsx`: 153 tests passed in 8 files. Coverage includes size/count compilation, exact inline boundary, linked-file fallback, preserving selections and retrying the original message without duplication, compiling unchecked inputs, and failure feedback.
- Desktop `pnpm run typecheck` and focused ESLint with `--max-warnings 0` over every changed TypeScript/TSX source and test: passed.
- Desktop i18n sync, compile and check: passed; 21 script tests passed and 15 locale catalogs validated. Initial extraction stopped at the source-change review gate for `sessionInputs.help`; all existing locale values were inspected and were English fallbacks. The authorized wording change was applied to those fallbacks, then synchronized with `--accept-source-changes`.
- Browser smoke: real `SessionInputControls`, selection state, size/error helpers and CSS served by Vite; ACP/native boundaries mocked. Select all 19 inputs, byte feedback, clear selection, compile all unchecked inputs, open the returned path, and failure feedback passed in installed Chrome at 450 px and 320 px. Screenshots were visually inspected; no page errors or narrow-layout overflow. The initial temporary harness had a CommonJS React JSX-runtime import failure; corrected harness imports/dependency optimization resolved it without product changes. This is UI evidence, not backend/provider acceptance.
- `git diff --check`: passed. Documentation index/log paths and governance markers checked.

## Handoff and remaining verification

The source patch is complete and partially validated. The installed app was not rebuilt, installed or exercised against the live input collection. No live session, input row or report file was modified. Rust integration tests were authored and typechecked but not executed; Cargo build/test/clippy remain unexecuted under the repository restriction.

After rebuilding through the existing development workflow, use **Compile all inputs** to save every listed source, including unchecked ones, as one Markdown file in the chat working directory and Outputs. To request the synthesized report, **Select all** and resend the original report prompt; the attachment manifest directs the agent to read every source in sections and preserve citations. Actual complete synthesis depends on available file-reading tools and the model following that request; this patch does not claim the report was generated or comprehensively verified.

The compilation limit is 32 MiB including its index/framing; existing 20 MiB linked-file and parser-complexity limits remain. Pasted text is intact, while linked documents use extracted text rather than original layout and images need separate inspection. Missing or unextractable sources and exceeded budgets fail before file publication. If Outputs registration fails after publication, the error explicitly identifies the saved file instead of claiming that no output exists.
