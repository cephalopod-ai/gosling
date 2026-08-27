# Maintainer-authority patch set

Date: 2026-08-27

## Outcome

The twelve records previously grouped as maintainer/external-authority work are
now split by what can actually be completed in source:

- Nine local code, product-policy, security, data, concurrency, and governance
  records are implemented and validated.
- The local portions of three release/repository records are implemented and
  validated.
- Three outward gates remain deliberately unexecuted: enabling the default
  branch ruleset, pushing and observing remote CI, and signing/tagging/publishing
  after installed-artifact validation.

## Closed source records

- Workspace deletion preserves pinned session/project-library data.
- Imported transcripts carry history, not provider/model/workspace/credential
  or executable authority.
- MCP Apps require user confirmation before proposed text enters chat.
- Managed-context providers are excluded from all multi-model Research seats.
- Session turns have a durable cross-process lease with heartbeat and recovery.
- `goslingd` enforces loopback-only binding.
- Unauthenticated MCP App guest/proxy access requires a loopback peer.
- The earlier remote MCP guest exception is superseded by the local-only plane.
- `.dory/` is ignored operational state, not canonical evidence.

## Release-source repairs

- BuildNotify succeeds visibly when its optional webhook is absent.
- Rust advisories and unused dependencies pass current `cargo-deny` and
  `cargo-machete` checks.
- The complete Rust workspace, Desktop, and documentation validation gates pass.
- Release ledgers distinguish observed local evidence from unperformed external
  and installed-artifact work.

## Validation summary

- Rust: format, build, complete workspace tests, and all-target Clippy passed.
- Desktop: typecheck and 1,071 tests passed.
- Documentation: typecheck, 16 tests, and production build passed.
- Supply chain: `cargo-deny check advisories` and `cargo-machete` passed.
- Workflow/governance: YAML parse, missing-webhook path, diff whitespace, and
  required governance marker checks passed.

## External hard gates

No external mutation was performed. A maintainer must separately authorize and
observe each action:

1. Enable and verify GitHub ruleset `18782969`.
2. Push the branch/revision and require green remote checks on that exact SHA.
3. Only after installed artifacts, signatures, checksums, and platform scenarios
   pass, create/push the candidate tag, publish, and perform unauthenticated
   readback.
