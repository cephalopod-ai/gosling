# 2026-10-17 — Meta Muse providers

## Task

Add Meta Muse API and subscription/CLI providers. Baseline: clean `main` at
`6dd8b7ae8`. Scope is local source changes; no installation or publication.

## Decisions and checkpoint

- API provider `meta`: existing OpenAI-compatible Chat Completions engine,
  `MODEL_API_KEY`, documented Muse Wonder/Spark model IDs and 1,048,576-token limits.
- Subscription provider `muse-code`: installed Muse Code's `exec --json` protocol,
  with saved CLI authentication and its own tools. No invented ACP adapter.
- Headless Muse cannot relay Gosling approval prompts: require Auto mode and retain
  Muse's sandbox. Gosling-hosted tools and image attachments must fail explicitly.
- Each turn replays Gosling's text/tool history into a fresh Muse process; this
  adapter does not claim native session resume or context ownership.
- API-key environment variables are removed from the CLI child. Muse's stored
  credential precedence still applies; a stored API key can select API billing.
- Independent source/documentation reads were batched. Edits and validation stay
  sequential because they depend on the finished provider contract.
- Both providers are registered in the shared setup catalog. Muse's Desktop
  dialog uses external setup instructions, with executable discovery determining
  configured status; it does not claim to have checked subscription authentication.
- Model discovery can construct the wrapper before the session selects Auto mode.
  Mode enforcement occurs at mode changes and before inference.
- CLI events have a 16 MiB line limit and a 600-second inactivity deadline.
  Stderr is drained without retaining unbounded output. Existing subprocess,
  process-group and abort-on-drop guards own cancellation cleanup.

## Files changed

- `.gitignore`
- `crates/gosling-providers/src/canonical/catalog.rs`
- `crates/gosling-providers/src/formats.rs`
- `crates/gosling-providers/src/formats/meta.rs`
- `crates/gosling-providers/src/openai.rs`
- `crates/gosling/src/providers/init.rs`
- `crates/gosling/src/providers/inventory/registrations.rs`
- `crates/gosling/src/providers/mod.rs`
- `crates/gosling/src/providers/muse_code.rs`
- `crates/gosling/src/providers/declarative/meta.json`
- `crates/gosling/tests/meta_muse_provider_test.rs`
- `docs/INDEX.md`
- `docs/logs/session/2026-10-17-meta-muse-provider.md`
- `documentation/docs/getting-started/providers.md`
- `documentation/docs/guides/meta-muse.md`

## Evidence

- [Meta overview](https://dev.meta.ai/docs/overview)
- [Chat Completions](https://dev.meta.ai/docs/protocols/chat-completions)
- [Reasoning](https://dev.meta.ai/docs/reasoning)
- [Muse authentication](https://dev.meta.ai/docs/muse-code/auth)
- [Muse headless interface](https://dev.meta.ai/docs/muse-code/extending)
- Local Muse Code `1.4.4 (1.4.4-R5419.1)`: inspected CLI help and ran an
  isolated, credential-free echo probe. Observed version-1 event envelopes,
  `run.lifecycle.started`, `run.output.delta`, and `run.terminal.completed`.

## Validation

Host: macOS arm64, repository-pinned Rust `1.92` toolchain, default Gosling features.
No dependencies or toolchains were installed; Cargo used `--locked --offline`.

- `RUSTUP_AUTO_INSTALL=0 cargo fmt --all` and final `cargo fmt --all -- --check` — passed.
- `RUSTUP_AUTO_INSTALL=0 cargo check -p gosling --lib --locked --offline` — passed.
- `RUSTUP_AUTO_INSTALL=0 cargo check -p gosling --test meta_muse_provider_test --locked --offline`
  — passed on the final source, without warnings. Eight regression tests compile;
  they were not executed. They cover registry/setup metadata, effort mapping,
  authenticated streaming/nonstreaming API requests, tool and usage handoff,
  private prompt transport, stderr draining, invalid terminal outcomes, permission
  boundaries, model discovery before mode selection, and cancellation.
- `git diff --check`, new-file whitespace scan, provider JSON assertions, local
  documentation target checks and governance marker checks — passed.
- `@mdx-js/mdx` compilation of the new guide and modified providers page — passed.
- Isolated installed Muse 1.4.4 echo probe with a private prompt file, JSON output,
  disabled approval and no session log — passed: 13 version-1 envelopes, matching
  root run identities, expected Unicode/multiline terminal text and exit 0.
  The first expanded probe included `--model` with the echo provider and correctly
  exited 2 (`--model` requires Meta or local). Repeating without model selection
  verified the headless transport contract. The echo probe does not exercise the
  Rust wrapper, Meta model selection, authentication, tools or billing.

## Qualification and follow-ups

Source implementation is complete and partially validated. Cargo build/test/clippy
remain unrun under AGENTS.md's explicit-request rule. No installed Gosling binary
was replaced. Authenticated API/subscription inference and interactive Desktop/CLI
journeys remain unverified. Next qualification command, when requested:
`cargo test -p gosling --test meta_muse_provider_test --locked --offline`.

The wrapper returns the final answer after successful completion, does not forward
Gosling extensions, and cannot guarantee subscription billing if Muse has a stored
API key. These limits are documented in its setup instructions and user guide.
