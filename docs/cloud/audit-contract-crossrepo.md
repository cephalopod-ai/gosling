# Audit Lens — Cross-Language / "Cross-Repo" Contract Seams

Lens: `audit-contract-crossrepo` (v0.2). Mode: **audit-only / read-only**.
Builds on `docs/cloud/00-orientation.md`. Only this file was written.

## Scope statement (read first)

gosling is a **monorepo**; there is **no external repository in scope**. This lens
treats every producer↔consumer boundary that **crosses a language or package
boundary** as a "cross-repo" seam, per the engagement's scoping note
(orientation §7). The seams audited:

1. **Rust core (producer) → ACP JSON schema → TypeScript SDK (`ui/sdk`, consumer)** —
   the primary generated-artifact seam.
2. **`ui/gosling-binary` invocation contract** — the `gosling` native binary (Rust)
   invoked by the SDK / desktop over the `acp` CLI subcommand + `GOSLING_BINARY`.
3. **Rust core → gosling-server OpenAPI (`utoipa`) → `ui/desktop/openapi.json`** —
   a second, HTTP-flavored schema artifact.
4. **Desktop hand-maintained `ui/desktop/src/types/*` vs Rust shapes** — duplicated
   TS DTOs.
5. **UPSTREAM.md goose-fork format assumptions** — gosling consumes goose's
   inherited serialized formats (share links, imported sessions).

**Out of scope for this lens** (routed elsewhere per skill scope boundary):
single-repo layering / internal DTO leakage (→ `audit-contract-internalapi`);
third-party provider API resilience (→ `audit-pipeline-externalapi`); the
`session/import_formats/{claude_code,codex,pi}.rs` consumers of *other tools'*
session formats are closer to external-tool ingestion than an internal seam and
are noted only as adjacent.

## Orientation / pins

- Repo root: `/home/user/gosling`. Not-a-standalone-git note: it **is** a git repo.
- Branch: `claude/gosling-stress-test-audit-jwhooa`; HEAD `bcf7277`.
- Working tree: clean except untracked `docs/` (this audit's output).
- **Deployment relationship:** *lockstep* for the desktop bundle — the release
  workflow builds the Rust binary and bumps both `Cargo.toml` and desktop
  `package.json` in one job (`.github/workflows/bundle-desktop.yml:106-113,124`).
  The standalone npm SDK (`@repo-makeover/gosling-sdk@0.20.2`) publishes
  *independently* with the binary as an `optionalDependencies` (`workspace:*`)
  (`ui/sdk/package.json:52-58`), so the published-SDK↔published-binary pairing is
  *not* lockstep.

## Contract inventory

| # | Artifact | Producer (path @ commit) | Consumer(s) | Transport | Source of truth | Drift check |
|---|---|---|---|---|---|---|
| 1 | ACP JSON schema + method meta | `crates/gosling/src/bin/generate_acp_schema.rs` → `crates/gosling/acp-schema.json`, `acp-meta.json` @ `a9ccfd5` | `ui/sdk/src/generated/{types,zod,client,index}.gen.ts` | committed JSON → `@hey-api/openapi-ts` codegen | **Rust (schemars)** | **`just check-acp-schema` git-diff gate** in CI `schema-check` job + runtime `check:compat` at publish |
| 2 | `gosling acp` binary invocation | native `gosling` binary (Rust) | `ui/sdk/src/resolve-binary.ts`, `ui/desktop/src/goslingServe.ts` | subprocess spawn + `GOSLING_BINARY` env + local HTTP/WS `…/acp?token=` | Rust CLI arg surface | `check:compat` (publish only); lockstep for desktop bundle |
| 3 | Server OpenAPI spec | `crates/gosling-server/src/openapi.rs` (`utoipa`) → `ui/desktop/openapi.json` | **none in-repo** (generated `src/api` client removed & forbidden, CLAUDE.md) | committed JSON | Rust (utoipa) | **NONE** (not in `check-acp-schema`, not in CI) |
| 4 | Provider / session UI DTOs | Rust structs (e.g. `crates/gosling-providers/src/base.rs:23`) | `ui/desktop/src/types/{providers,session,…}.ts` | hand-written mirror, fed via ACP adapter | ambiguous (parallel hand copies) | TS compile only (indirect) |
| 5 | Nostr share-link deeplink format | `crates/gosling/src/session/nostr_share.rs` (`gosling://sessions/nostr`) | end users w/ inherited goose links; desktop `main.ts` protocol handler | URL scheme | Rust (rewritten from goose) | unit tests |

Risk: seams 1–2 are **P1** (generated client + protocol). Seam 3 is **P2/P3**
(no live consumer). Seams 4–5 are **P2**.

## Seam verdicts

- **Seam 1 (ACP schema → SDK): SAFE / well-guarded.** Rust is the single source of
  truth; TS is generated; CI mechanically fails on any drift. See non-finding NF-1.
- **Seam 2 (binary invocation): SAFE for desktop (lockstep); additive-risk for the
  standalone SDK** — mitigated by the publish-time `check:compat` gate, but that
  gate is read-only-method-only (XREPO-GSL-004) and there is no runtime
  protocol-version negotiation (XREPO-GSL-003).
- **Seam 3 (server OpenAPI): ORPHANED + UNGATED** → XREPO-GSL-001.
- **Seam 4 (desktop hand DTOs): held** via adapter mediation — see NF-2.
- **Seam 5 (goose share links): breaking-silent vs the stated preservation policy**
  → XREPO-GSL-002.

---

## Findings

### XREPO-GSL-001: `ui/desktop/openapi.json` is an orphaned, ungated schema artifact that can silently drift from the gosling-server it claims to describe

Severity: Low
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Architecture (XREP-003 / XREP-006)

Evidence:
- Producer: `crates/gosling-server/src/openapi.rs:574` (`pub struct ApiDoc`) +
  `crates/gosling-server/src/bin/generate_schema.rs:15` writes `…/openapi.json`;
  regenerated only via `Justfile:25` / `Justfile:153-155` (`generate-openapi`).
- Committed artifact: `ui/desktop/openapi.json` exists (only committed
  `openapi.json` in the tree).
- **No in-repo consumer:** grep for any consumer of `openapi.json` under `ui/`,
  `.github/` returned nothing; the generated client `ui/desktop/src/api` does not
  exist and is explicitly forbidden (`AGENTS.md` "Never: Recreate
  `ui/desktop/src/api`"). Desktop reaches the backend exclusively over ACP.
- **Not in any drift gate:** the schema gate diffs only
  `crates/gosling/acp-schema.json crates/gosling/acp-meta.json
  ui/sdk/src/generated/` (`Justfile:158-162`); `ui/desktop/openapi.json` is absent,
  and `generate-openapi` is not referenced in `.github/workflows/ci.yml`. Its
  version is bumped by a blind `jq` string edit at release
  (`Justfile:320-321`), not by regeneration-and-diff.

Observed behavior:
- The committed OpenAPI spec can diverge arbitrarily from the actual
  `gosling-server` handlers without any test or CI job failing.

Expected boundary:
- Either a regeneration-and-diff gate (as seam 1 has) or removal of the orphaned
  artifact so no external HTTP consumer is misled by a stale contract.

Failure mechanism:
- The `src/api` consumer that once forced regeneration was deleted; the spec file
  and its generator were left behind with no gate replacing the deleted consumer's
  implicit pressure.

Break-it angle:
- Add a route or change a field in `gosling-server`; `cargo build` and the full CI
  pass while `ui/desktop/openapi.json` still advertises the old shape to any
  external client that fetches it.

Impact:
- Any out-of-repo HTTP client or documentation generated from this spec sees a
  potentially stale contract. In-repo blast radius is nil (no consumer), which is
  why this is Low, not Medium.

Operational impact:
- Blast radius: Cross-system (external clients only). Side-effect class: none in-repo.
- Reversibility: reversible. Operator visibility: silent. Rerun safety: safe.

Adjacent failure modes:
- Same class as XREPO-GSL-004 (a real contract with a partial/absent gate).

Recommended mitigation:
- Remediation patterns: regen-and-diff gate, or delete-orphaned-artifact.
- Minimal repair: add `generate-openapi` output to the `check-acp-schema`
  git-diff set, **or** delete `ui/desktop/openapi.json` + its generator if truly
  unused.
- Behavior test: CI regenerates `openapi.json` and fails on diff.

Implementation assessment:
- Complexity: local_guardrail. Cost: XS. Cost drivers: modules, tests.
- Nominal implementation agent: codex.
- Rationale: one Justfile/CI line, or a file+bin deletion.

Validation:
- Introduce a one-field server route change; assert CI fails on the openapi diff
  (post-fix) instead of passing.

Non-goals:
- Do not reintroduce a generated `ui/desktop/src/api` client (forbidden).

---

### XREPO-GSL-002: gosling deliberately rejects inherited `goose://` share links, contradicting the UPSTREAM.md preservation policy

Severity: Low
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Compliance-Posture (XREP-008 — deprecation/removal without consumer migration)

Evidence:
- Policy (producer-of-record): `UPSTREAM.md:17` — "Preserve explicit
  compatibility notes for inherited formats such as **old share links** when the
  product still supports them."
- Code rejects them: `crates/gosling/src/session/nostr_share.rs:255-262`
  `parse_deeplink` returns `Err("Invalid Gosling Nostr session share link")` unless
  `scheme() == "gosling"`.
- Deliberate + tested: `crates/gosling/src/session/nostr_share.rs:375-381`
  `rejects_legacy_goose_deeplink` asserts `goose://sessions/nostr…` is an error;
  `:383-391` `detects_gosling_share_deeplinks` asserts `is_session_share_deeplink`
  returns `false` for `goose://`.
- Provenance: the rebrand commit `a9ccfd5` and follow-up `d47e19a` ("Fix rebrand
  regressions") collapsed a pre-rebrand dual check — `d47e19a` shows the residual
  `starts_with("gosling://…") || starts_with("gosling://…")` (both branches
  identical), the fossil of an original `goose:// || gosling://` acceptor.

Observed behavior:
- A user importing a share link created by upstream goose (the imported v1.38
  baseline) receives a hard "Invalid" error rather than a successful import.

Expected boundary:
- Per the stated policy, inherited `goose://sessions/nostr` links should still
  parse (accept both schemes) OR the policy line should be amended to declare the
  break intentional.

Failure mechanism:
- The rebrand replaced every `goose` token with `gosling`, including the legacy
  acceptor branch, turning a backward-compat allowance into a rejection; a test
  was then written to lock in the rejection.

Break-it angle:
- Feed a real goose v1.38 `goose://sessions/nostr?nevent=…&key=…` link → import
  fails closed (correct fail direction, but contradicts policy).

Impact:
- Migrating goose users cannot import previously-shared sessions. Fail-**closed**,
  so no security/corruption risk — purely a documented-compatibility contradiction.

Operational impact:
- Blast radius: Workflow (per-user import). Side-effect class: user-visible.
- Reversibility: reversible. Operator visibility: UI-visible (error). Rerun safety: safe.

Adjacent failure modes:
- Broader fork-format posture: verify `session/import_formats/*` and any other
  inherited on-disk formats against the same preservation policy.

Recommended mitigation:
- Remediation patterns: dual-scheme acceptor, or policy amendment.
- Minimal repair: accept both `goose://` and `gosling://` in `parse_deeplink` /
  `is_session_share_deeplink`; update the two tests. **OR** amend `UPSTREAM.md:17`
  to record that legacy share links are intentionally dropped.
- Behavior test: `parse_deeplink("goose://sessions/nostr?…")` returns `Ok`.

Implementation assessment:
- Complexity: governance_decision (policy vs product intent). Cost: XS.
- Cost drivers: docs, tests.
- Nominal implementation agent: human-owner (decide keep-break vs restore-compat),
  then codex.
- Rationale: the code is trivial; the real question is a product/provenance
  decision the maintainer must make.

Validation:
- Whichever way it is decided, the test and `UPSTREAM.md` must agree.

Non-goals:
- Do not change the nostr encryption/relay logic in this slice.

---

### XREPO-GSL-003: ACP `initialize` echoes the client's `protocol_version` back unvalidated — no version negotiation gate

Severity: Low
Confidence: Likely
Evidence basis: simulation-reasoned
Domain: Architecture (XREP-004 / XREP-012)

Evidence:
- Producer: `crates/gosling/src/acp/server.rs:2181`
  `Ok(InitializeResponse::new(args.protocol_version)…)` — the agent reflects the
  caller-supplied `args.protocol_version` verbatim rather than responding with the
  highest version it actually supports.
- Consumer pin: `ui/sdk/package.json` peerDependency
  `"@agentclientprotocol/sdk": "^0.19.0"` (`ui/sdk/package.json` peerDependencies)
  — the protocol version is owned by that third-party SDK, floating across the
  `^0.19.0` range for standalone SDK consumers.
- No `MIN_PROTOCOL`/version-compat branch found near the handshake (grep for
  `protocol_version` in `acp/server.rs` returned only the echo site).

Observed behavior:
- A client advertising any `protocol_version` gets it echoed back as "agreed",
  so both sides believe a version is mutually supported without either checking.

Expected boundary:
- ACP `initialize` is a negotiation point: the agent should return
  `min(agent_max, client_requested)` (or reject unsupported majors), not echo.

Failure mechanism:
- The handshake trusts the client's number; capability compatibility is never
  gated on it.

Break-it angle:
- A future/older `@agentclientprotocol/sdk` (allowed by `^0.19.0`) speaking a
  protocol the Rust agent does not actually implement is told it is supported;
  divergence surfaces later as a malformed message rather than a clean
  version-mismatch error at handshake.

Impact:
- For the desktop bundle (lockstep versions) impact is nil; for the independently
  published SDK against an independently pinned binary it is a fail-open on
  version skew. Low because ACP shape drift is separately caught by Zod at
  message time in `client.gen.ts` (`z<Type>.parse`).

Operational impact:
- Blast radius: Service (per-connection). Side-effect class: network.
- Reversibility: reversible. Operator visibility: silent at handshake. Rerun safety: safe.

Adjacent failure modes:
- Couples with XREPO-GSL-004: the only cross-version pairing test is read-only.

Recommended mitigation:
- Remediation patterns: version-negotiation gate.
- Minimal repair: clamp/validate `args.protocol_version` against the agent's
  supported set in `initialize`.
- Behavior test: initialize with an unsupported protocol version → clamped
  response or explicit error, not an echo.

Implementation assessment:
- Complexity: external_service_semantics. Cost: S. Cost drivers: modules, tests.
- Nominal implementation agent: codex.
- Rationale: localized to the handshake, but the "supported set" must be defined.

Validation:
- Seam test at a mismatched protocol version asserting negotiated (not echoed)
  output.

Non-goals:
- Do not re-implement the upstream ACP SDK's transport.

---

### XREPO-GSL-004: publish-time runtime seam test (`check:compat`) exercises only read-only ACP methods; write-path DTOs are never round-tripped against the real binary

Severity: Low
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Architecture (XREP-016 — seam test coverage gap)

Evidence:
- `ui/sdk/scripts/check-binary-compat.mjs` header: boots `gosling acp` and calls
  "every **read-only** ACP method"; `READ_ONLY_CHECKS` lists only
  `providersList_unstable`, `providersCatalogList_unstable`,
  `defaultsRead_unstable`, `preferencesRead_unstable`, `sourcesList_unstable`,
  `dictationConfig_unstable`, … (all read paths).
- Write-path methods that exist in the same generated client but are **not**
  exercised: e.g. `providersCustomCreate_unstable`, `providersCustomUpdate_unstable`,
  `providersConfigSave_unstable`, `defaultsSave_unstable`, `preferencesSave_unstable`
  (`ui/desktop/src/acp/providers.ts:96,107,133,178,196`).
- Gate wiring: `.github/workflows/publish-npm.yml:105-108` runs `check:compat`
  against the freshly-built linux binary.

Observed behavior:
- Request-side DTOs for write methods (agent-inbound params) are validated only
  *structurally* by the static schema-diff gate (seam 1), never against the actual
  running binary the way read responses are.

Expected boundary:
- The runtime pairing test should cover at least one representative write DTO in
  each direction, since request-shape drift (consumer→producer, forward
  compatibility) is exactly what a read-only test cannot catch.

Failure mechanism:
- The static gate (seam 1, NF-1) is strong and covers *all* types structurally, so
  this is a defense-in-depth gap, not an open hole — hence Low.

Break-it angle:
- A serde rename on a request-only field would be caught by the static schema
  regen-diff (NF-1) but would silently pass `check:compat`; if the static gate
  were ever bypassed (e.g. schema regenerated but Rust serde attr changed without
  schemars reflecting it), no runtime test would notice a write break.

Impact:
- Reduced runtime confidence on write paths at publish; mitigated by seam 1.

Operational impact:
- Blast radius: Repo (publish confidence). Side-effect class: none.
- Reversibility: reversible. Operator visibility: silent. Rerun safety: safe.

Adjacent failure modes:
- XREPO-GSL-003 (no protocol negotiation) compounds runtime-skew blind spots.

Recommended mitigation:
- Remediation patterns: bidirectional seam test.
- Minimal repair: add one idempotent write+readback (e.g. `preferencesSave` then
  `preferencesRead`) to `READ_ONLY_CHECKS`' successor set, cleaning up after.
- Behavior test: the new round-trip asserts the persisted value.

Implementation assessment:
- Complexity: workflow_protocol. Cost: S. Cost drivers: tests.
- Nominal implementation agent: codex.
- Rationale: additive test against an existing harness.

Validation:
- Write→read round-trip asserts value equality against the live binary.

Non-goals:
- Do not weaken the static schema gate; this is additive.

---

## Explicit non-findings (checked and held)

**NF-1 — ACP schema seam (Rust → `ui/sdk` generated client) is mechanically gated.**
Single source of truth is the Rust schemars types
(`crates/gosling/src/bin/generate_acp_schema.rs:10-14`). CI job `schema-check`
runs `just check-acp-schema` (`.github/workflows/ci.yml:167-201`), which
regenerates and `git diff --exit-code`s `crates/gosling/acp-schema.json`,
`acp-meta.json`, and `ui/sdk/src/generated/` (`Justfile:158-162`). The `changes`
path filter classes any non-`documentation/**` edit as `code`
(`.github/workflows/ci.yml:38-39`), so a Rust ACP type change cannot silently skip
the gate. Break-it check: the two ACP source commits after the schema was last
generated (`d47e19a`) touched only `manage_sessions.rs`'s internal
`is_nostr_session_link` helper (`git show d47e19a` — no `pub struct/enum/fn`,
serde, or `method` change), so the committed schema is **not** stale. The
generated client additionally validates every response at runtime with Zod
(`z<Type>.parse` in `ui/sdk/src/generated/client.gen.ts`, produced by
`generate-schema.ts:225-233`). XREP-001/004/006/016 do **not** apply to this seam.

**NF-2 — Desktop hand-maintained `src/types/*` are adapter-mediated, not raw
cross-language parsers.** `ui/desktop/src/acp/providers.ts:32-63` constructs the
hand-written `ProviderDetails`/`ProviderMetadata` (`ui/desktop/src/types/providers.ts`)
field-by-field from the **gated** SDK DTOs (`entry.providerId`, `entry.models[]…`),
not by casting raw Rust JSON. The hand type is an internal UI model; the actual
cross-language shape is the SDK DTO covered by NF-1. Removed producer fields the
adapter references would fail `tsc`; added optional fields are silently ignored
(normal, low-risk adapter behavior). XREP-002/009 do not rise to a finding here.

**NF-3 — Desktop↔binary version pairing is lockstep at bundle time.**
`bundle-desktop.yml:106-124` bumps `Cargo.toml` and desktop `package.json` versions
together and builds `gosling` from the same source in the same job, and the
desktop consumes the SDK/binary via `workspace:*` (`ui/desktop/package.json:52`).
No version-skew window exists for the desktop app (the standalone-SDK case is
covered by XREPO-GSL-003/004).

## Different-angle checks performed

- **version-timeline:** the only post-schema-regen ACP commit (`d47e19a`) is a
  non-schema refactor → no stale generated client (NF-1).
- **constant angle:** the `goose://` vs `gosling://` literal was traced via
  `git log -S`; the rebrand collapsed a dual acceptor into a rejecter
  (XREPO-GSL-002).
- **generated-code angle:** regeneration is enforced by CI diff (NF-1); not
  re-run locally here (see limits).
- **unknown-input angle:** ACP responses are Zod-parsed at runtime (fail-closed);
  the protocol-version handshake is **not** gated (XREPO-GSL-003).

## Validation limits (not reviewed / not executed)

- **No build/regeneration executed.** `just check-acp-schema`,
  `cargo run --bin generate-acp-schema`, and `generate_schema` (server OpenAPI)
  were **not** run (heavy Rust build + hermit toolchain, out of read-only budget).
  NF-1's "not stale" verdict rests on git archaeology (no schema-surface commit
  after the last regen), not on a fresh regen-diff — a regen could in principle
  surface non-determinism. To upgrade NF-1 to test-reproduced, run
  `just check-acp-schema` and confirm a clean diff.
- **`ui/desktop/openapi.json` was not diffed against a fresh `generate_schema`
  run** (XREPO-GSL-001 is argued from "no consumer + no gate", not from a proven
  content drift; a fresh regen-diff would quantify the actual staleness).
- **Published-artifact pins not inspected.** How `workspace:*` binary
  optionalDependencies are rewritten at `pnpm publish` time (exact version range
  the published SDK pins for the binary) was not extracted from a built tarball;
  XREPO-GSL-003's standalone-skew concern is therefore Likely, not Confirmed.
- **Runtime seams not driven.** The ACP handshake, `check:compat`, and any
  version-skew pairing were reasoned statically, not executed. XREPO-GSL-003/004
  runtime manifestations are simulation-reasoned / source-evidenced, capped
  accordingly.
- **`session/import_formats/{claude_code,codex,pi}.rs`** (consumers of other
  tools' session formats) were inventoried but not deep-audited — closer to
  external-tool ingestion than an internal cross-language seam; recommend the
  `audit-pipeline-externalapi` lens cover their parse robustness.
- **`ui/text` (Ink TUI) consumption of the SDK** was not separately traced; it
  shares the same gated SDK seam (NF-1) but its specific call sites were unreviewed.
