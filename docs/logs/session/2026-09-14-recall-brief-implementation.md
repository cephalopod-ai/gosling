# 2026-09-14 — Provenance-safe Recall Brief implementation

## Task and authority

The maintainer explicitly requested a review and revision of the supplied
architectural draft followed by code implementation. The draft's `plan_only`
authority described its earlier creation, not this implementation request.
Target: `main` at `82be8db80d28765bed3f4d3b8c00dffcda5e99b1`, initially clean.
No Muninn files are modified.

## Reviewed contract and revisions

- Gosling's ordinary provider reply streams chunks before a retrospective
  report validator can inspect the final answer. V1 stays an explicit ACP
  report action.
- Muninn serves a wrapped `MemorySearchResult`; exact hit links use pinned
  `muninn://memory/{store}/{memory-id}?revision=...` URIs. The normalizer
  validates that wrapper, hit identity, excerpt coordinates, and safety signals.
- V1 quotes only the returned excerpt window. It does not fetch a mutable head
  or perform the optional pinned-resource hydration proposed in the draft.
- Model source coverage requires exact returned quotes. The host assigns source
  keys and citations and derives the recall receipt; the model cannot supply
  either identity or world-verification status.
- The pure core uses the canonical SDK report DTOs so ACP and core share one
  wire contract; the core does not use ACP transport or session policy APIs.

## Checkpoint 1 — typed contract and pure core

Files changed: `crates/gosling-sdk-types/src/recall_brief.rs`, SDK module exports,
`crates/gosling/src/recall_brief/`, and
`crates/gosling/tests/{recall_brief.rs,fixtures/recall_brief_santa.json}`.
The Santa fixture has distinct exact revisions with the same `effort_group` and
separate authored valid-time windows.

Validation:

- Baseline `cargo test -p gosling-sdk-types --locked`: 31 tests and doc tests passed.
- `cargo fmt --all`: passed after the first code slice.
- `cargo test -p gosling --test recall_brief --locked`: 7 tests passed, including
  separate A/B citations, exact-revision deduplication, malformed/partial/empty
  outcomes, a valid reported-belief proposal, and fabricated-citation fallback.

## Checkpoint 2 — ACP, permission boundary, and reviewed documentation

Files changed: `crates/gosling/src/acp/server/recall_brief.rs` and ACP dispatch,
session gate/plan-start fencing, `crates/gosling-test-support/src/mcp.rs`,
`crates/gosling/tests/acp_custom_requests_test.rs`, the OpenAI SSE fixture,
SDK wire tests, README, Recall Brief guide, ADR-0022, docs index, and the
architecture registry. No CLI or Desktop UX command was added because the
reviewed V1 contract makes the explicit typed ACP action its first entrypoint;
those clients can adopt the same contract later.

The ACP fixture exercises a selected enrolled Muninn MCP connection and an
actual Gosling-managed OpenAI provider adapter. It also verifies that an open
plan pre-denies recall before external catalog access, and that wrong-session
extension selection and denied MCP permission do not reach the recall tool.
The report retains both distinct Santa revisions and unresolved world
existence. The MCP mock advertises only `muninn_recall`, so it has no writer,
staging, or promotion action to invoke.

Validation at this checkpoint:

- `cargo test -p gosling-sdk-types --test recall_brief_dto_wire --locked`: 3 passed.
- `cargo test -p gosling --test recall_brief --locked`: 10 passed, adding
  pinned-window rejection, partial/empty/failed facet signals, and a planted
  verified-world statement fallback.
- `cargo test -p gosling --test acp_custom_requests_test test_recall_brief --locked`:
  3 passed, including the end-to-end selected-tool/provider path and two
  boundary cases.

## Final validation and limits

The final Santa fixture reports A and B as separately cited reported beliefs,
places the change in the labeled Inference section with both quotes, and leaves
gift-deliverer existence unresolved. A second four-source mock adds C's
literal-source assertion and a historical Saint Nicholas record under distinct
referent senses. The rendered findings now include their checked exact quotes;
untrusted Markdown in source titles is escaped. An ACP request lifetime guard
cancels its Muninn call if the request is dropped.

- `cargo test -p gosling-sdk-types --locked`: 34 unit/integration tests and
  doc tests passed.
- Final `cargo test -p gosling --test recall_brief --locked`: 12 passed.
- Final `cargo test -p gosling --test acp_custom_requests_test test_recall_brief
  --locked`: 3 passed.
- `cargo fmt --all -- --check`: passed.
- Final `cargo clippy --all-targets --locked -- -D warnings`: passed after
  one local parser-style fix.
- `.architecture/components.yaml` and `invariants.yaml` parse as YAML, have
  unique entries, and every newly declared path exists. New documentation
  targets exist, the AGENTS governance marker remains present, and
  `git diff --check` passed. `GEMINI.md` is absent.

Unisolated `cargo test -p gosling --locked` was partially successful: 1,983
core tests passed, four ignored, and four failed. Three failed prompt-manager
snapshots picked up a user-customized `system.md` from the normal Gosling
configuration root; those 11 focused tests passed against a fresh test root.
The fourth failure is a pre-existing HEAD contradiction: the source declares
`CURRENT_SCHEMA_VERSION = 36` while the untouched
`unshipped_schema_35_corrects_hash_v1_in_place` test asserts 35. A broad run
with a fresh configuration root and only that known baseline assertion skipped
passed all remaining core and integration tests. No unrelated snapshot,
configuration, migration, or user file was changed.

Qualification remains bounded to shaped Muninn fixtures and mocked provider
outputs plus one local OpenAI adapter fixture. The structural validator cannot
prove arbitrary paraphrase fidelity, autobiographical accuracy, or external
world truth. No live Muninn/provider latency or second model profile was
measured, and no CLI/Desktop entrypoint, installed-app acceptance, or published
release is claimed.
