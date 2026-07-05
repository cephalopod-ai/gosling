# Dead / Duplicate / Deprecated Code Audit — gosling

Lens: `audit-deadcode-cleanup`. Scope focus per tasking: orphaned / dead /
duplicated / deprecated / legacy / compat-drifted code, with emphasis on
**leftover residue from features the fork claims to have dropped** (recipe,
schedule, gateway, local-models/local-inference) and the "smaller footprint"
claim. Authority: **audit-only / read-only**. Builds on
`docs/cloud/00-orientation.md`. IDs: `DEAD-GSL-NNN`.

---

## Summary (headline)

The dropped-feature removal in this fork is **unusually clean**. The features the
README says were dropped — local-inference stack, `recipe` / `schedule` /
`gateway` / `local-models` subcommands — have been removed *thoroughly*: no
leftover Cargo dependencies, no ACP handlers, no capability advertisements, no
DTOs, no desktop UI components, no generated-schema entries, no Justfile/CI
references, and no config-key writers survive. This directly and honestly serves
the footprint goal (Cargo.lock 1065 vs 1251 pkgs; 0 hits for
`llama-cpp`/`hf-hub`/`mlx-rs` in `Cargo.lock`).

The residue that *does* remain is small and low-severity:

1. **`EXTRACTION_PLAN.md`** is a 25 KB planning document that is now **partially
   stale**: two of its three workstreams (local-inference, gateway/telegram)
   are already executed, so the doc describes deleted modules — with `file:line`
   references to a commit whose layout no longer exists — as if they are still
   present. (DEAD-017, Medium doc-hygiene.)
2. **`SessionType::Scheduled`** is a vestige of the dropped `schedule` feature —
   never constructed by production code, retained only to deserialize legacy
   on-disk sessions (keep-with-reason). (DEAD-020/DEAD-009, Low.)
3. A **stale `#[allow(dead_code)]`** on a handler that is in fact reachable
   (`handle_fork_session`). (Info.)
4. A broader **`#[allow(dead_code)]` population (33 sites)**, mostly test
   fixtures, serde deser-only fields, and upstream-inherited helpers — general
   residue, not dropped-feature specific. (Low, reported as a cluster.)

No confirmed orphaned *modules*, no duplicated dropped-feature implementations,
and no leftover dependencies were found.

---

## Method

### Entry points / roots enumerated
- Process roots: `crates/gosling-cli/src/main.rs` (CLI), `Command` enum in
  `crates/gosling-cli/src/cli.rs:559`; `crates/gosling-server` (HTTP/ACP);
  `ui/desktop/src/main.ts` (Electron); `ui/text` (Ink TUI).
- Library root: `crates/gosling/src/lib.rs` module tree.
- Served/framework roots: ACP dispatch (`crates/gosling/src/acp/server/dispatch.rs`),
  server route registry (`crates/gosling-server/src/routes/mod.rs`).
- Feature roots: `[features]` in `crates/gosling/Cargo.toml:10-66`.

### Searches run (representative)
- Presence of dropped modules: `ls crates/gosling/src/gateway/` (absent),
  `.../providers/local_inference/` (absent), `.../posthog.rs` (present — see
  scope note).
- Leftover deps: `grep -riE "llama-cpp|hf-hub|mlx-rs|mlx-lm|candle|symphonia|rubato|tokenizers"`
  across all `Cargo.toml` (0) and `grep -ciE "llama-cpp|hf-hub|mlx-rs" Cargo.lock` (0).
- Leftover identifiers/strings: `SessionType::Gateway`, `gateway_config*`,
  `LOCAL_LLM_MODEL`, `localInference`, `LocalInference*`, `telegram`, `tg:`
  across `crates/`, `ui/`, `documentation/`, generated schema/openapi.
- `#[allow(dead_code)]` census (`grep -rn "allow(dead_code)" crates/`).
- Legacy/import paths: `session/legacy.rs`, `session/import_formats/`.
- Build/CI: `Justfile`, `.github/workflows/build-cli.yml`.

### Not run (Validation Limits — see end)
`cargo build` / `cargo clippy` (would surface real `dead_code` warnings),
`cargo-udeps`, `cargo tree`. Read-only static pass only.

---

## Findings

### DEAD-GSL-001: `EXTRACTION_PLAN.md` is a partially-executed, now-misleading plan

Severity: Medium
Confidence: Confirmed (that the doc describes removed code as present)
Evidence basis: source-evidenced
Domain: Negative-Space / Dead-Code (DEAD-017 stale doc reference)

Evidence:
- `EXTRACTION_PLAN.md:14-17` — self-describes as "a plan only — no code changes
  accompany it … with file/line references as of commit `c5fbbd7`."
- Workstream A (local inference) is **already executed**: the files the plan
  says to delete do not exist — `ls crates/gosling/src/providers/local_inference/`
  → absent; `grep -ciE "llama-cpp|hf-hub|mlx-rs" Cargo.lock` → `0`; the
  `local-inference`/`cuda`/`vulkan`/`mlx` features the plan quotes at
  `crates/gosling/Cargo.toml:23-44` are gone (actual `[features]` block ends at
  `Cargo.toml:66` with no such entries).
- Workstream B (Telegram/gateway) is **already executed**: `ls
  crates/gosling/src/gateway/` → absent; `SessionType` at
  `crates/gosling/src/session/session_manager.rs:44-52` has no `Gateway`
  variant; `ui/desktop/src/utils/urlSecurity.ts` has no `tg:`/`telegram:` entry;
  the doc's cited `telegram-gateway.md` is absent from `documentation/`.
- Workstream C (telemetry + update) is **NOT executed**: `crates/gosling/src/posthog.rs`
  still exists and is called (`crates/gosling/src/agents/agent.rs:1046,2448…`,
  `session_manager.rs:1375`); `telemetry` feature persists
  (`crates/gosling/Cargo.toml:12`) and its route is mounted
  (`crates/gosling-server/src/routes/mod.rs:29`); `crates/gosling-cli/src/commands/update.rs`
  still exists.
- Counter-search: the plan is referenced nowhere in build/CI/source
  (`grep -rn "EXTRACTION_PLAN"` finds only the file itself); it is documentation,
  not wired into any process.

Observed behavior:
- A prominent top-level 25 KB document presents deleted modules (with precise but
  now-invalid `file:line` anchors) as the live current state, while a third of
  its scope remains pending — with no status markers distinguishing done from
  pending.

Expected boundary:
- Planning docs for completed work are either removed, moved to
  `docs/`/history, or annotated with completion status so a maintainer can trust
  their line references.

Failure mechanism:
- The work was carried out but the plan was never reconciled; its commit anchor
  (`c5fbbd7`) predates the removals.

Break-it angle:
- A maintainer following Workstream A/B "deletions" hunts for
  `crates/gosling/src/gateway/` or `providers/mod.rs:53` and finds nothing,
  wasting time or concluding the tree is inconsistent; a reader trusts it as an
  inventory of *present* phone-home paths and mis-scopes a security review.

Impact:
- Documentation drift; misleads footprint/removal reviewers. No runtime effect.

Operational impact:
- Blast radius: Repo (contributor-facing). Side-effect class: none.
  Reversibility: reversible. Operator visibility: silent. Rerun safety: safe.

Adjacent failure modes:
- The still-pending Workstream C content is the only accurate part; deleting the
  whole file would lose the live telemetry/update removal plan (see Non-goals).

Recommended mitigation:
- Disposition `remove_stale_doc_reference` (partial): split the doc — retain
  Workstream C (still actionable) and remove or mark-as-DONE Workstreams A and B,
  or add a status banner + re-anchor to current HEAD. Behavior test: none
  (docs); a CI grep asserting the doc contains no dangling paths would prove it.

Implementation assessment:
- Complexity: governance_decision. Cost: XS. Cost drivers: docs.
  Nominal implementation agent: human-owner (owner must decide keep-vs-cut of the
  pending workstream). Rationale: pure documentation reconciliation, but the
  keep/cut call is a maintainer decision, not mechanical.

Validation:
- After edit, every `file:line` cited in the doc resolves in the current tree, or
  the section is explicitly marked historical/pending.

Non-goals:
- Do not delete the Workstream C telemetry/update removal plan; it is unexecuted
  and still valid.

---

### DEAD-GSL-002: `SessionType::Scheduled` — vestige of the dropped `schedule` subcommand

Severity: Low
Confidence: Confirmed (never written in production; read-only retention)
Evidence basis: source-evidenced
Domain: Dead-Code (DEAD-020 platform/feature-gated dead branch / DEAD-009 compat retention)

Evidence:
- Enum variant: `crates/gosling/src/session/session_manager.rs:47` (`Scheduled`).
- README claims the `schedule` subcommand was dropped
  (`README.md:43` — "gosling also drops the `recipe`, `schedule`, `gateway`, and
  `local-models` CLI subcommands"); no `schedule`/`Schedule` subcommand exists in
  `crates/gosling-cli/src/cli.rs` (`grep -niE "schedule"` → none).
- **No production constructor**: `grep -rn "SessionType::Scheduled" crates/`
  returns only read/filter sites
  (`chatrecall.rs:65`, `list_sessions.rs:14`, `session_manager.rs:420,489,1858`)
  and one test (`session_manager.rs:2593`). No code assigns `session_type =
  SessionType::Scheduled` on a create path.
- Retention reason is explicit in-source:
  `crates/gosling/src/acp/server/list_sessions.rs:238` — "ACP clients see their
  own (Acp) sessions plus **legacy** User/Scheduled ones."

Observed behavior:
- The variant survives purely to deserialize and filter pre-existing on-disk
  "Scheduled" sessions (created by the old schedule feature or by upstream
  goose); nothing in the fork produces new ones.

Expected boundary:
- Enum arms for a removed feature either continue to serve stored-data
  deserialization (documented) or are removed with a migration.

Failure mechanism:
- The producing subcommand was dropped; the type was correctly kept for backward
  read-compatibility of the serialized `session_type` field.

Break-it angle:
- Removing the variant would break `serde` deserialization of any stored session
  whose metadata records `"session_type":"scheduled"` — i.e., it is **not** safe
  to delete despite being unconstructed. This is the classic serialization
  dynamic-edge that defeats a naive grep-based "unused variant" removal.

Impact:
- Negligible; a single enum arm plus a handful of filter references. Carries a
  small "why is this here?" comprehension cost.

Operational impact:
- Blast radius: Local. Side-effect class: none. Reversibility: n/a (retention).
  Operator visibility: silent. Rerun safety: safe.

Adjacent failure modes:
- `SessionExecutionMode::scheduled()` (`crates/gosling/src/execution/mod.rs:25`,
  used at `execution/manager.rs:416`) is a *separate*, live "background mode"
  concept — do not conflate it with the dropped subcommand.

Recommended mitigation:
- Disposition `document_as_intentional`: add a one-line doc comment on the
  `Scheduled` variant noting it is retained only for legacy-session
  deserialization. Do **not** delete.

Implementation assessment:
- Complexity: local_guardrail. Cost: XS. Cost drivers: none (one comment).
  Nominal implementation agent: codex. Rationale: trivial annotation.

Validation:
- A round-trip test asserting a stored `"session_type":"scheduled"` session still
  deserializes proves the retention is load-bearing.

Non-goals:
- Do not attempt to remove the variant or migrate stored sessions in this slice.

---

### DEAD-GSL-003: Stale `#[allow(dead_code)]` on a reachable handler (`handle_fork_session`)

Severity: Info
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Dead-Code (DEAD-010 residue / misleading suppression)

Evidence:
- `crates/gosling/src/acp/server/fork_session.rs:4` — `#[allow(dead_code)]` on
  `pub(super) async fn handle_fork_session`.
- It **is** reached: `crates/gosling/src/acp/server.rs:2876`
  (`on_fork_session` → `self.handle_fork_session(...)`), which is dispatched from
  `crates/gosling/src/acp/server/dispatch.rs:359-362` on a `ForkSessionRequest`.

Observed behavior:
- A `dead_code` suppression sits on a symbol that is actually live; the attribute
  masks nothing and mis-signals the code as dead.

Failure mechanism / Break-it angle:
- Likely a leftover from when the handler was being wired; the allow was never
  removed once dispatch landed. A future reader may believe fork-session is
  unwired, or the allow may hide a *genuinely* dead sibling later added under the
  same attribute.

Impact:
- Comprehension only; no runtime effect.

Recommended mitigation:
- Disposition `delete_after_confirmed_unused` (of the attribute, not the fn):
  remove the `#[allow(dead_code)]`; a `cargo clippy --all-targets -D warnings`
  run (the repo's own gate) proves no warning appears.

Implementation assessment:
- Complexity: local_guardrail. Cost: XS. Nominal implementation agent: codex.

Validation:
- `cargo clippy --all-targets -- -D warnings` passes with the attribute removed.

Non-goals:
- Do not touch the handler body or dispatch.

---

### DEAD-GSL-004: Broad `#[allow(dead_code)]` population (cluster, low priority)

Severity: Low
Confidence: Likely (each individual site not exhaustively reachability-traced)
Evidence basis: source-evidenced
Domain: Dead-Code (DEAD-001/DEAD-010 cluster)

Evidence:
- 33 `#[allow(dead_code)]` sites total (`grep -rn "allow(dead_code)" crates/ | wc -l`).
- Bucketed by inspection of representative sites:
  - **Test fixtures** (majority): `crates/gosling/tests/acp_fixtures/*`,
    `tests/acp_common_tests/mod.rs:889,1041`, `tests/acp_*_test.rs:1-3`,
    `tests/providers.rs:192` ("only used by tests behind non-default feature
    flags"). Legitimately test-only.
  - **serde deser-only fields**: `agents/validate_extensions.rs:13` (`enabled`),
    `agents/platform_extensions/ext_manager.rs:73` (`context`),
    `plugins/mod.rs:78` (`format`). Fields parsed for schema completeness but not
    read — keep-with-reason (removing them changes deserialization tolerance).
  - **Upstream-inherited helpers**: `acp/server.rs:120` (`ResultExt` trait),
    `acp/server/agent_requests.rs:21` (`agent_request_schema`),
    `providers/githubcopilot.rs:169` ("useful for debugging"),
    `providers/claude_code.rs:197`, `providers/api_client.rs:405`.
  - **Feature-gated**: `gosling-server/src/routes/telemetry.rs:11`
    (`cfg_attr(not(feature="telemetry"), allow(dead_code))`) — correct
    conditional suppression, not residue.
  - **Genuinely stale**: `fork_session.rs:4` — see DEAD-GSL-003.

Observed behavior:
- The suppressions are mostly load-bearing (serde, tests, feature gates), a few
  are upstream-inherited convenience code kept "for debugging," and at least one
  is stale. None are tied to the dropped features.

Recommended mitigation:
- Disposition `keep_status_quo` for serde/test/feature-gated sites; route the
  upstream "kept for debugging" helpers (`githubcopilot.rs:169`, `ResultExt`,
  `agent_request_schema`) to `governance-code-polish` for a governed
  clippy-driven sweep if the footprint goal wants them gone. Each requires a
  per-site reachability confirmation (clippy with the allow removed) before
  deletion — do not bulk-delete.

Implementation assessment:
- Complexity: workflow_protocol. Cost: S. Nominal implementation agent: codex.
  Rationale: mechanical per-site, but must be gated by clippy to avoid removing a
  serde-required field.

Validation:
- Per site: remove the attribute, run `cargo clippy --all-targets -D warnings`;
  if it stays green the symbol is truly unused and removable.

Non-goals:
- Do not remove serde-parsed fields (they widen accepted input) or test fixtures.

---

## Non-findings (checked and held)

These are the seams a dead-code reviewer would most expect to yield residue for a
"dropped-feature" fork. They were checked and are clean:

- **Local-inference dependencies fully purged.**
  `grep -riE "llama-cpp|hf-hub|mlx-rs|mlx-lm|candle|symphonia|rubato|tokenizers"`
  over every `Cargo.toml` → **0**; `Cargo.lock` → **0** for
  `llama-cpp|hf-hub|mlx-rs`. The footprint claim is backed by real dep removal.
- **Local-inference features removed.** `crates/gosling/Cargo.toml:10-66` has no
  `local-inference`/`cuda`/`vulkan`/`mlx` feature; `gosling-cli` /
  `gosling-server` Cargo.tomls carry no passthroughs.
- **No local-inference ACP/UI/schema surface.** No
  `acp/server/local_inference.rs`; `grep -rniE "localInference|LocalInference"`
  over `crates/gosling/acp-meta.json`, `acp-schema.json`, `ui/desktop/openapi.json`,
  `ui/desktop/src/acp/capabilities.ts`, `FeaturesContext.tsx` → none. No
  `ui/desktop/src/components/settings/localInference/` tree.
- **Gateway/Telegram fully removed.** No `crates/gosling/src/gateway/`, no
  `SessionType::Gateway` (`session_manager.rs:44-52`), no `gateway_config*` keys,
  no `tg:`/`telegram:` in `ui/desktop/src/utils/urlSecurity.ts`, no
  `telegram-gateway.md` under `documentation/`. `manager.rs::check_auto_start`
  (flagged dead by the plan) is gone with its module.
- **Build/CI clean.** `Justfile` and `.github/workflows/build-cli.yml` contain no
  `local-inference`/`vulkan`/`cuda`/`mlx`/`libvulkan`/`gateway` references
  (`linux_vulkan_features` helper removed).
- **`toolshim.rs` handles removed values intentionally** — not dead:
  `crates/gosling/src/providers/toolshim.rs:69-71` explicitly rejects
  `"local"`/`"llama.cpp"`/`"llama_cpp"` with a clear "support was removed from
  this build" error; covered by test at `:949`. This is a deliberate
  compat-drift guard (keep-with-reason), the opposite of a dead branch.
- **`session/legacy.rs` is live, not orphaned.** Called from
  `session_manager.rs:917-935` for legacy `.jsonl` session migration.
- **`session/import_formats/` is live.** `detect_format` / `convert_to_gosling_session_json`
  used by CLI import (`crates/gosling-cli/src/commands/session.rs:310-315`) and
  `session_manager.rs:1936`; the `pi.rs` importer maps to `ImportFormat::Pi`
  (`session.rs:315`). Not dead.
- **No duplicated dropped-feature implementations** were found (no parallel
  gosling-vs-goose local-inference or gateway modules coexisting).

Out-of-lens (present but NOT a claimed-dropped feature, so not scored here):
`posthog.rs` + `telemetry` feature/route, `commands/update.rs` (feature-gated at
`cli.rs:746` behind `#[cfg(feature="update")]`). The README does not claim these
were dropped; EXTRACTION_PLAN Workstream C proposes removing them but that work is
unexecuted (see DEAD-GSL-001). Their *telemetry/consent* behavior belongs to the
security/compliance lenses.

---

## Validation Limits (what was NOT done)

- **No compiler-assisted dead-code detection.** `cargo build` / `cargo clippy
  --all-targets -D warnings` / `cargo-udeps` / `cargo tree` were not run
  (read-only, and build is heavy in this environment). The real Rust `dead_code`
  lint and unused-dependency proof were therefore not obtained; DEAD-GSL-004
  buckets are `Likely`, not per-site `Confirmed`.
- **Dependency-consumer proof is static only.** "0 leftover local-inference deps"
  is from `Cargo.toml`/`Cargo.lock` grep, not `cargo tree`; a transitively-pulled
  variant under a renamed crate would be missed (none seen).
- **TypeScript/desktop dead code not deeply swept.** The lens focused on Rust
  dropped-feature residue; a full `knip`/`ts-prune` pass over `ui/desktop` and
  `ui/text` was not performed. Only the specific dropped-feature UI anchors from
  EXTRACTION_PLAN were checked (all absent).
- **`allow(dead_code)` sites not each reachability-traced.** 33 sites bucketed by
  sampling ~10; the un-sampled remainder (mostly `tests/acp_fixtures/*`) is
  assumed test-only from path, not individually confirmed.
- **Commented-out code / orphaned assets** (`assets/`, `documentation/static/`,
  `vendor/`, `services/ask-ai-bot/`, `oidc-proxy/`, `evals/`) were not swept for
  orphans — only dropped-feature-relevant residue was in scope.
- **README internal version inconsistency** (`v0.0.1` provenance vs `gosling
  v1.40.0` in the footprint table, `README.md:23` vs `:31`) is noted but belongs
  to `audit-compliance-posture`, not this lens.

---

## Recommended patch slices (ordered, low-risk)

1. **DEAD-GSL-001** — reconcile `EXTRACTION_PLAN.md`: mark/remove executed
   Workstreams A & B, keep pending Workstream C. (human-owner, XS.)
2. **DEAD-GSL-003** — drop the stale `#[allow(dead_code)]` on
   `handle_fork_session`; prove with clippy. (codex, XS.)
3. **DEAD-GSL-002** — add an intentional-retention comment to
   `SessionType::Scheduled`. (codex, XS.)
4. **DEAD-GSL-004** — optional governed clippy sweep of upstream "debugging"
   helpers, per-site gated. (governance-code-polish, S.)

Residual risk: minimal. The dropped-feature removal itself is materially complete
and dependency-backed; the outstanding items are documentation reconciliation and
cosmetic suppression cleanup, none of which affect runtime behavior or the
footprint claim.
