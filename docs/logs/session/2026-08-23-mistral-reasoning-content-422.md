# Mistral `reasoning_content` 422 repair

Date: 2026-08-23  
Skill: `repair-defect-priority`  
Branch: `repair/mistral-reasoning-content-422-2026-08-23`  
Base: `502d79825`

## Input finding

The operator supplied a Mistral API response showing HTTP 422 from
`/v1/chat/completions`. Every validation error identified
`messages[*].assistant.reasoning_content` as an extra forbidden request field.

## Selected batch

- P1 backend compatibility: the bundled Mistral profile inherited the OpenAI
  engine default `preserves_thinking: true`, causing stored assistant thinking
  to be replayed under a field Mistral rejects.
- No other defects were selected or deferred. Feature work, shared formatter
  changes, persisted conversation rewrites, and unrelated provider cleanup were
  out of scope.

## Patch

- `crates/gosling/src/providers/declarative/mistral.json` explicitly sets
  `preserves_thinking` to false.
- `crates/gosling/src/config/declarative_providers.rs` adds a regression test
  pinning that bundled-provider capability.
- Code checkpoint: `8e1501aff`.

The new regression test was run before the profile change and failed with
`Mistral rejects reasoning_content on assistant request messages`. It passed
after the capability correction.

## Verification

| Command | Result | Coverage |
|---|---|---|
| `cargo fmt --check` | pass | Rust formatting |
| `cargo test -p gosling --lib test_mistral_omits_replayed_reasoning_content` | pass (failed before patch) | bundled Mistral capability regression |
| `cargo test -p gosling-providers preserve_thinking_context` | 2 passed | shared formatter preserves by default and omits when disabled |
| `cargo test -p gosling --lib config::declarative_providers::tests` | 17 passed | bundled/custom declarative provider loading |
| `cargo test -p gosling-providers` | 450 passed | provider serialization and response handling |
| `cargo test -p gosling --lib` | 1,697 passed | core regression suite |
| `cargo build -p gosling` | pass | crate build |
| `cargo clippy -p gosling --all-targets -- -D warnings` | pass | all-target static analysis |

A live paid Mistral completion was not sent. The supplied 422 establishes the
remote schema failure, while the deterministic config and formatter tests prove
the offending field is now omitted from the request path.

## Architecture and regression audit

- Authoritative sources: the supplied Mistral validation response governs the
  rejected request shape; `DeclarativeProviderConfig.preserves_thinking` is the
  active repository capability contract; `formats/openai.rs` tests govern the
  shared OpenAI-compatible serialization behavior.
- Touched-surface mapping: only the bundled Mistral capability declaration and
  its config regression test changed. Persisted messages, response parsing,
  tool calls, model selection, streaming, and other provider profiles did not.
- Pre-repair disposition: pre-existing contract drift — the Mistral profile
  enabled a request field rejected by the provider.
- Post-repair disposition: conformant for the observed request shape.
- Drift delta: no new drift; the provider-specific capability now selects the
  already-supported omission path.
- Backward compatibility: stored thinking remains intact and can still be used
  by providers that support replay. Mistral continues returning visible model
  output; only unsupported historical reasoning replay is omitted.

## Record closure

- `docs/TODO.md`: fresh Mistral 422 finding -> resolved, with code and
  verification pointers.
- No pre-existing in-code marker, issue, or deferred-work record named this
  defect.

Residual risk: no authenticated live Mistral completion was sent during this
repair. Follow-up: rebuild the consuming CLI/Desktop artifact before retrying an
existing session.

Final status: `completed_verified`.
