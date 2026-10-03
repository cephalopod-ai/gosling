# Muninn recall tool-schema repair — 2026-10-02

Operator asked to investigate and patch Muninn failing in Gosling while
recalling unimplemented application ideas. The client-side defect belongs
here: `/Users/eric/Work/vscode/forked/gosling`, `main` at `c036ce53d`, initially
clean. Adjacent Muninn repair is recorded in its October monthly summary.
Catalog MCP repair procedure applied to the bounded integration failure.

## Evidence and cause

A public session export showed repeated `muninn__muninn_recall` calls filling
every optional field, including invented cursor `initial` and guessed scope
selectors. Muninn correctly refused those cursors; repeated attempts received
the same error. The fallback retrieval timed out separately. No private
session database was inspected or changed.

`create_codex_request` manually rendered function declarations without
`strict`, while the generic Responses formatter already explicitly sets
`strict: false` and tests it. [OpenAI's function-calling contract](https://developers.openai.com/api/docs/guides/function-calling#strict-mode)
says Responses may normalize omitted strictness into strict mode, whose
fields must all be required. This supports the optional-field coercion
explanation; the remote normalized schema was not captured. Explicitly
setting `strict: false` preserves arbitrary MCP input schemas and optional
arguments on both ordinary and lite Responses requests. Local tool validation
still enforces the original schema.

## Patch and validation

- Added `strict: false` to the existing shared tool declaration builder.
- Added two model-specific regression cases that inspect the correct payload
  location, require explicit false strictness, and compare the entire original
  schema including query-only required fields.
- `source bin/activate-hermit && RUSTUP_AUTO_INSTALL=0 DEVELOPER_DIR=/Library/Developer/CommandLineTools
  cargo check --locked --offline -p gosling --lib --tests`: exit 101 before
  reaching this module, with two missing-`Debug` errors for
  `AuthenticationExtensionSetRequest` in concurrent authentication work.
- `cargo fmt --all -- --check`: exit 1 on unrelated concurrent authentication
  changes. No whole-tree formatting mutation was run.
- Targeted `rustfmt --edition 2021 --check crates/gosling/src/providers/chatgpt_codex.rs`:
  exit 0. `git diff --check` also passed. The new retained-record links resolve.
- Cargo build/test/Clippy were not invoked because repo AGENTS.md requires an
  explicit operator request for those commands. The new tests are not claimed
  as compiled or executed. No provider request, binary install, live client
  reload, commit, publication, or service restart occurred.

Other authentication files became dirty after initial inspection and were
preserved. This patch touches only the provider, this record, test ledger,
docs index, and the exact ignore negation retaining this log. Self-review
checked both request layouts, unchanged schema bytes, server-side validation,
and no changes to permissions or credential handling. Rollback is the scoped
provider/documentation diff; no stored data changes exist.

Status: source-fixed, partially validated, deployment-unverified. After the
concurrent compilation blocker is resolved and build/test are requested, run
the two named cases, rebuild/install Gosling, and retry with a fresh recall
omitting cursor and unrequested selectors. The separate live Muninn corpus
hashing latency and startup timeout are not fixed by this provider patch.
