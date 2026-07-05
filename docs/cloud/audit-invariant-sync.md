# Gosling Audit — Invariant Sync Lens

Lens: `audit-invariant-sync` (domain INV). Authority: **audit-only / read-only**.
Builds on `docs/cloud/00-orientation.md`. Scope for this lens: facts that must
stay in sync but are hand-maintained in several places — replicated
lists/registries, a canonical source bypassed by a second path, enums/constants
duplicated across Rust and the TS UI/SDK, provider catalog vs declarative
configs, permission definitions across config & UI, ACP message/type definitions
Rust↔Electron, slash-command lists, CLI subcommand lists, config keys in docs vs
code.

Effort: ~33 tool calls, targeted at cross-language enum/constant duplication.
Everything not enumerated under "Non-Findings" was sampled, not exhaustively
traced — see **Validation Limits**.

---

## 1. Headline

The dominant invariant-sync risk in gosling is **cross-language type
duplication**: the Rust serde types (mostly in `crates/gosling-providers` and
`crates/gosling`) are the wire source of truth, and `ui/desktop/src/types/*.ts`
hand-re-declares the same enums/tagged unions. A generation pipeline exists
(utoipa `ToSchema` derives on the Rust types; `ui/sdk/generate-schema.ts`
produces `ui/sdk/src/generated/types.gen.ts` from `crates/gosling/acp-schema.json`),
but `AGENTS.md` **forbids** the desktop from consuming generated types
("UI Desktop: Use ACP SDK types or local `src/types/*` types. Do not import
generated OpenAPI types/client code from `ui/desktop/src/api`"). The result: a
large family of enums is kept in agreement **by hand, with no parity test, no
codegen, and no type forcing it**.

Every enum pair I checked is *currently aligned* — this is a healthy fork today.
The finding is the **absence of enforcement** (INV-007 / INV-009), not a present
byte-level drift. Two concrete drifts were found and are minor (a stale doc
comment; a parallel dispatch list). Both the ground-truth Rust side and the
consuming TS side are quoted for every pair below.

---

## 2. Invariant-Sync Inventory

Ground-truth source = the Rust serde type (it defines the JSON on the wire).
"Copies" = every hand-maintained re-declaration. "Guard" = anything that forces
agreement.

| Invariant (fact) | Ground-Truth (Rust) | Hand-maintained copy | Must match? | Guard | Delta today |
|---|---|---|---|---|---|
| GoslingMode values | `gosling-providers/src/gosling_mode.rs:24-34` (`auto/approve/smart_approve/chat`) | `ui/desktop/src/types/session.ts:5` **and** `ui/desktop/src/components/settings/mode/ModeSelectionItem.tsx:48-69` (`all_gosling_modes`) | yes | none | aligned (3 copies) |
| SessionType values | `gosling/src/session/session_manager.rs:44-52` (`user/scheduled/sub_agent/hidden/terminal/acp`) | `ui/desktop/src/types/session.ts:26-32` | yes | none | aligned |
| ProviderType values | `gosling/src/providers/base.rs:19-24` (`Preferred/Builtin/Declarative/Custom`) | `ui/desktop/src/types/providers.ts:1` | yes | none | aligned |
| ThinkingEffort values | `gosling-providers/src/thinking.rs:276-284` (`off/low/medium/high/max`) | `ui/desktop/src/types/providers.ts:3` | yes | none | aligned |
| Permission (confirmation result) | `gosling-providers/src/permission.rs:4-12` (`always_allow/allow_once/cancel/deny_once/always_deny`) | `ui/desktop/src/types/permissions.ts:1` | yes | none | aligned |
| MessageContent variants | `gosling-providers/src/conversation/message.rs:268-282` (10 camelCase variants) | `ui/desktop/src/types/message.ts:176-186` | yes | none | aligned |
| ActionRequiredData variants | `gosling-providers/src/conversation/message.rs:197-219` (`toolConfirmation/elicitation/elicitationResponse`) | `ui/desktop/src/types/message.ts:108-127` | yes | none | aligned |
| SystemNotificationType values | `gosling-providers/src/conversation/message.rs:251-257` (`thinkingMessage/inlineMessage/creditsExhausted`) | `ui/desktop/src/types/message.ts:96` | yes | none | aligned |
| DictationProvider values | `gosling/src/dictation/providers.rs:14-20` (`openai/elevenlabs/groq`) | `ui/desktop/src/types/dictation.ts:1` | yes | none | aligned; **stale doc** at `gosling-sdk-types/src/custom_requests.rs:1534` adds phantom `"local"` |
| Built-in slash commands | `gosling/src/agents/execute_commands.rs:19-56` (`COMMANDS`, 9 entries) | dispatch `match` at `execute_commands.rs:126-140` | yes (list ↔ handler) | Rust test `slash_command.rs:50-63` guards the *names list* only | aligned; no guard that every name has a handler |
| PermissionLevel (stored) | `gosling/src/config/permission.rs:19-23` (`always_allow/ask_before/never_allow`) | none in TS (server-only) | n/a | — | single-consumer |
| Permission ↔ PermissionDecision | `gosling-providers/src/permission.rs` ↔ `gosling/src/acp/common.rs:12-53` | exhaustive `From` impls | yes | **compiler + `#[test_case]` (common.rs:191-207)** | enforced ✓ |
| Declarative provider catalog | `src/providers/declarative/*.json` embedded via `include_dir!` (`config/declarative_providers.rs:21`) | — | n/a | directory scan | single source ✓ |
| ACP session-mode list | `acp/response_builder.rs:203-220` iterates `GoslingMode::VARIANTS` | — | n/a | strum-derived | single source ✓ |

---

## 3. Findings

### INV-GSL-001: Rust↔desktop type enums replicated by hand with no drift guard

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Invariant Sync

Evidence (ground truth Rust ↔ hand copy TS, per pair):
- GoslingMode — `crates/gosling-providers/src/gosling_mode.rs:24-34` (`Auto/Approve/SmartApprove/Chat`, `#[serde(rename_all="snake_case")]`) ↔ `ui/desktop/src/types/session.ts:5` (`'auto' | 'approve' | 'smart_approve' | 'chat'`) ↔ a **third** copy `ui/desktop/src/components/settings/mode/ModeSelectionItem.tsx:48-69` (`all_gosling_modes` array of the same 4 keys).
- SessionType — `crates/gosling/src/session/session_manager.rs:44-52` ↔ `ui/desktop/src/types/session.ts:26-32`.
- ProviderType — `crates/gosling/src/providers/base.rs:19-24` ↔ `ui/desktop/src/types/providers.ts:1`.
- ThinkingEffort — `crates/gosling-providers/src/thinking.rs:276-284` ↔ `ui/desktop/src/types/providers.ts:3`.
- Permission — `crates/gosling-providers/src/permission.rs:4-12` ↔ `ui/desktop/src/types/permissions.ts:1`.
- MessageContent (10-variant tagged union) — `crates/gosling-providers/src/conversation/message.rs:268-282` ↔ `ui/desktop/src/types/message.ts:176-186`.
- SystemNotificationType — `crates/gosling-providers/src/conversation/message.rs:251-257` ↔ `ui/desktop/src/types/message.ts:96`.
- ActionRequiredData — `crates/gosling-providers/src/conversation/message.rs:197-219` ↔ `ui/desktop/src/types/message.ts:108-127`.
- Guard absence: no file under `ui/desktop/src` or `ui/sdk/tests` asserts parity against `crates/gosling/acp-schema.json` or the Rust enums (grep for `types.gen|parity|acp-schema` in tests returns only `ui/sdk/tests/client-callbacks.test.ts`, which is unrelated). `AGENTS.md` mandates hand-maintenance: "Do not import generated OpenAPI types/client code from `ui/desktop/src/api`".

Observed behavior:
- ~9 enums / tagged unions that define the Rust→Electron JSON wire contract are re-declared by hand in `ui/desktop/src/types/*.ts`. Nothing (no codegen output consumed, no test, no shared schema) forces the two sides to agree. A generation path exists (utoipa `ToSchema` on the Rust types; `ui/sdk/generate-schema.ts`) but the desktop is explicitly directed away from it.

Expected boundary:
- A value/variant added to a Rust serde enum should either regenerate the TS type, or fail a parity test, before it can silently reach the UI as an unhandled case.

Failure mechanism:
- Two independent edit sites for one fact with no forcing function. Adding a `GoslingMode` variant, a `MessageContent` variant, or a `SessionType` requires editing 2–3 files (for GoslingMode: the Rust enum, `session.ts`, and `all_gosling_modes`); forgetting the TS edit compiles and ships. A new `MessageContent` variant emitted by Rust would deserialize in the desktop as an unmodeled `type`, silently dropped or rendered as unknown.

Break-it angle:
- Add `MessageContent::Reasoning` (camelCase `reasoning`) in Rust and emit it. The desktop `MessageContent` union at `message.ts:176-186` has no `reasoning` arm; TS narrowing routes it to the default/unknown branch — a message block the operator never sees, with no compile-time or test failure anywhere. Same shape for a new `GoslingMode` (mode picker omits it) or `SystemNotificationType` (notification silently unhandled).

Impact:
- Operator-visible truth gaps in the desktop UI on any future Rust enum extension: missing modes, dropped message/notification blocks, mis-rendered tool states. Bounded to the desktop surface (CLI reads Rust types directly and is compiler-checked; TUI uses Rust types too). No data corruption. The blast radius is UI fidelity, which is why this is Medium not High.

Operational impact:
- Blast radius: Workflow (desktop UI). Side-effect class: user-visible. Reversibility: reversible. Operator visibility: silent (the gap is invisible until noticed). Rerun safety: safe.

Adjacent failure modes:
- INV-GSL-002 (dictation doc already drifted from the enum) is the same class caught early.
- A future divergence in `ActionRequiredData` (a permission/elicitation surface) would degrade a *security-relevant* prompt path — escalate to the permission-gating lens if that variant set changes.

Recommended mitigation:
- Remediation pattern: single-source + generated-consumer, or parametrized drift-guard test.
- Minimal repair: add a Vitest/Jest parity test in `ui/desktop` that imports the enum member lists from the generated `ui/sdk/src/generated` (or a small exported constant array) and asserts the hand-written unions/`all_gosling_modes` are set-equal, with a readable diff. Parametrize over the generated list so a new member is auto-covered.
- Local guardrail: convert the union types whose members are also needed at runtime (GoslingMode, SessionType, ThinkingEffort) into `as const` arrays with a derived union, so `all_gosling_modes` and the type share one literal source inside TS.
- Behavior test: assert `new Set(rustGoslingModeValues) === new Set(all_gosling_modes.map(m=>m.key))`.

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Cost drivers: tests, a small export from the generated SDK. Nominal agent: codex. Rationale: mechanical, one test file plus a shared constant; no cross-process semantics.

Validation:
- Test: generated-schema enum members equal each `ui/desktop/src/types` union (fails when Rust adds a variant and TS is not updated).
- Test: `all_gosling_modes` keys equal the GoslingMode union.

Non-goals:
- Do not re-enable `ui/desktop/src/api` generated-client imports (violates `AGENTS.md`); the guard should compare against member lists, not force the desktop to consume the generated client.

---

### INV-GSL-002: Dictation request doc advertises a phantom `local` provider not in the enum

Severity: Low
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Invariant Sync

Evidence:
- `crates/gosling/src/dictation/providers.rs:14-20` — `DictationProvider` enum has exactly `OpenAI`, `ElevenLabs`, `Groq` (`#[serde(rename_all="lowercase")]`).
- `crates/gosling-sdk-types/src/custom_requests.rs:1534` — doc comment on the ACP `DictationTranscribeRequest.provider` field: `/// Provider to use: "openai", "groq", "elevenlabs", or "local"`.
- `ui/desktop/src/types/dictation.ts:1` — `DictationProvider = 'openai' | 'elevenlabs' | 'groq'` (correctly omits `local`; the doc is the outlier).
- Deserialization site `crates/gosling/src/acp/server/dictation.rs:21` parses the string into the 3-variant enum, so `"local"` would fail deserialization.

Observed behavior:
- The wire field is a free-form `String` documented with a 4th value (`"local"`) that no enum variant, dispatch arm, or `PROVIDERS` entry (`providers.rs:33-64`) supports.

Expected boundary:
- The advertised value set should equal the accepted value set (the enum).

Failure mechanism:
- Doc comment drifted from the enum (or a `local` provider was removed and the comment left behind). Nothing links the two.

Break-it angle:
- A client trusting the doc sends `provider:"local"`; the request fails to deserialize into `DictationProvider` at `dictation.rs:21` and errors, rather than doing anything useful.

Impact:
- Minor: a misleading contract for a single unstable ACP method. No data effect.

Operational impact:
- Blast radius: Local. Side-effect class: none (request rejected). Reversibility: reversible. Operator visibility: log-only. Rerun safety: safe.

Adjacent failure modes:
- Same drift class as INV-GSL-001; this is the one instance where a copy already disagrees.

Recommended mitigation:
- Minimal repair: change the field to the typed `DictationProvider` enum (it already derives `Serialize/Deserialize/ToSchema`) instead of `String`, which makes the doc list generated and impossible to drift; or delete `"local"` from the comment.
- Behavior test: assert the request round-trips exactly the `DictationProvider` variants.

Implementation assessment:
- Complexity: local_guardrail. Cost: XS. Cost drivers: modules. Nominal agent: codex.

Validation:
- Test: every `DictationProvider` variant deserializes from the request; `"local"` is rejected (documents the true set).

Non-goals:
- Do not add a real local provider here.

---

### INV-GSL-003: Slash-command `COMMANDS` list and dispatch `match` are parallel with a guard only on the list

Severity: Low
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Invariant Sync

Evidence:
- `crates/gosling/src/agents/execute_commands.rs:19-56` — `static COMMANDS` declares 9 built-in commands (`prompts, prompt, compact, clear, skills, doctor, goal, grind, status`), used to *advertise* commands (`list_commands()` → `slash_command.rs:7-18` → ACP `available_commands`).
- `crates/gosling/src/agents/execute_commands.rs:126-140` — the *dispatch* `match command { ... }` handles the same 9 names, with a catch-all `_ => handle_skill_command(...)`.
- `crates/gosling/src/slash_commands/slash_command.rs:50-63` — a test hard-codes and asserts the 9 *names*, guarding the advertised list, but nothing asserts every advertised name has a dedicated dispatch arm.

Observed behavior:
- Membership of the built-in command set is hand-maintained in two places (the `COMMANDS` table and the `match` arms). The name-list test is itself a third hard-coded copy (a drift risk the skill warns about — a hard-coded test list is "a fourth copy to drift").

Failure mechanism / break-it angle:
- Add a `CommandDef { name: "review", ... }` to `COMMANDS` but forget the `match` arm: the command is advertised to ACP clients, and when invoked it silently falls through to `handle_skill_command("review", ...)` — treated as a skill lookup, not the intended built-in, with no failing test (the names-list test still passes because it too was updated to include `review`, but the dispatch gap is unguarded).

Impact:
- Low: a mis-routed built-in command; graceful-ish fallthrough to skill handling. Rust-internal, no cross-language surface.

Operational impact:
- Blast radius: Local. Side-effect class: user-visible (command behaves as skill). Reversibility: reversible. Operator visibility: silent. Rerun safety: safe.

Recommended mitigation:
- Give `CommandDef` an explicit handler (function pointer / enum dispatch) so the table *is* the dispatch, removing the second copy; or add a test that every `COMMANDS` name resolves to a non-fallthrough handler.
- Replace the hard-coded name-list test with one that derives expectations from `COMMANDS` (parametrized), not a literal vector.

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Cost drivers: modules, tests. Nominal agent: codex.

Validation:
- Test: for every `COMMANDS` entry, dispatch does not hit the skill catch-all.

Non-goals:
- Do not redesign the command system.

---

## 4. Non-Findings (checked and held)

- **Declarative provider catalog — single source.** `config/declarative_providers.rs:21` embeds the whole `src/providers/declarative/` directory via `include_dir!`; a new provider JSON auto-registers (`register_declarative_providers`, `:449-461`). No hand-maintained provider list to drift. (INV-001/010 denied for declarative providers.)
- **ACP session-mode list — derived, not duplicated.** `acp/response_builder.rs:203-220` builds the mode list by iterating `GoslingMode::VARIANTS` (strum) and pulling descriptions from `EnumMessage`; the Rust side has one source. The desktop receives modes at runtime via ACP for session config (the hand-copies in §3 are for the *settings* screen and the type union, which is the actual gap).
- **Permission ↔ PermissionDecision mapping — enforced.** `acp/common.rs:31-53` provides exhaustive `From` impls both directions (compiler forces coverage of new variants) and `#[test_case]` at `common.rs:191-207` asserts every variant maps. This is the model the type enums in INV-GSL-001 lack.
- **Built-in slash commands delivered to ACP clients at runtime**, not re-declared in the desktop (`list_acp_commands` → `available_commands`). No TS copy of the command names.
- **CLI subcommands — single clap source.** `crates/gosling-cli/src/cli.rs:430-561` derives parsing/help from one `Subcommand` enum; no parallel hand list. (A separate `documentation/automation/cli-command-tracking/` pipeline exists to keep docs in sync — a deliberate guard, not a drift.)
- **Generated ACP SDK is single-sourced.** `ui/sdk/src/generated/{types,zod,client}.gen.ts` is generated from `crates/gosling/acp-schema.json` (`ui/sdk/generate-schema.ts:20-40`); `PermissionLevel` and the custom request/response types flow from Rust → schema → TS with no hand copy. The gap is only the desktop types the mandate keeps hand-written.
- **Enum value alignment verified for all pairs in §2** — every Rust serde value equals its TS literal *today* (GoslingMode, SessionType, ProviderType, ThinkingEffort, Permission, MessageContent, ActionRequiredData, SystemNotificationType, DictationProvider). These are legitimate "must-match" invariants that currently hold; the finding is the missing enforcement, not a present drift.

### Legitimate divergence checked and cleared

- `Permission` (`AlwaysAllow/AllowOnce/Cancel/DenyOnce/AlwaysDeny`, snake_case) vs `PermissionDecision` (`AllowAlways/AllowOnce/RejectAlways/RejectOnce/Cancel`, `acp/common.rs:12-18`) use *different variant names* by design — one is the internal decision type, the other the stored/wire permission. This is a class-appropriate divergence with an explicit, tested bidirectional mapping (`common.rs:31-53,191-207`). **Not** drift.
- `ProviderSetupMethodDto::Local` (`custom_requests.rs:1014`) is unrelated to dictation `DictationProvider`; the `local` names are coincidental, not a shared fact. Not drift.

---

## 5. Break-It Review (summary)

- Added-member trace: a new `MessageContent`/`GoslingMode`/`SystemNotificationType` variant in Rust requires 1 (Rust) + 1–2 (desktop TS) manual edits; **zero** gates fail if the TS edits are skipped → latent shotgun surgery (INV-010) even though copies match today. This is the mechanism behind INV-GSL-001.
- Round-trip: the Rust↔TS boundary is serialize-only from Rust's view (Rust emits, TS reads); the asymmetry risk is an *unhandled incoming variant*, covered above.
- Registry bypass: no consumer re-derives the declarative-provider set or the mode set by hand — those are single-sourced (non-findings).
- Compiler-enforced Rust-internal invariants (the `From` impls, `GoslingMode::VARIANTS`) held under the "add a variant" test.

---

## 6. Validation Limits (what was NOT reviewed)

- `crates/gosling-sdk-types/src/custom_requests.rs` (~1600 lines) and `custom_notifications.rs` were **sampled**, not diffed field-by-field against `ui/sdk/src/generated/types.gen.ts`; because that path *is* generated, drift is unlikely, but I did not run the generator to confirm the checked-in `types.gen.ts` is current vs `acp-schema.json`. A stale committed `types.gen.ts` would be an INV-013-style generated-artifact drift I did not test.
- `ui/text` (Ink/TUI) types were not compared against Rust; it largely consumes Rust types directly (Rust crate), but its own local TS/config surfaces were not inventoried.
- Full field-level parity *within* each aligned struct (e.g. every field of `Session`, `ProviderMetadata`, `ModelConfig`) was not exhaustively diffed — only enum/variant membership was verified for the pairs in §2.
- `gosling-server` OpenAPI schema vs any consumer was not built or diffed.
- Config-key duplication in Markdown docs vs code was only cursorily checked (a dedicated `documentation/automation/cli-command-tracking/` pipeline exists for CLI docs; other config-key docs under `docs/`, `README.md`, `CUSTOM_DISTROS.md` were not line-diffed against `config/base.rs`).
- No code was executed; all findings are `source-evidenced` static reads. Confidence is capped accordingly for anything depending on runtime/generator behavior.
