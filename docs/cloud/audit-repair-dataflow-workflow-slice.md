# Audit + Repair — Dataflow/Workflow slice of 8 new commits (713f1eef2..9d9df730f)

Follow-up to the merged 35-lens stress-test campaign (`99-master-report.md`,
`repair-campaign-log.md`). Executed by `pipeline-analyst` (Dataflow & Workflow
Analyst) against a bounded slice of the 8 commits landed 2026-07-05: documentation
tooling/scripts, and the frontend (TS/React) I/O mapping for the new
session-resume-paging feature (introduced in `23786c1d3`).

**Authority: audit + bounded repair.** Fixes applied directly to the working tree
(uncommitted); no `git commit`. Schema/contract files (`crates/gosling/acp-schema.json`,
`crates/gosling-sdk-types/src/custom_requests.rs`) and execution-runtime files are
out of scope — read-only, flagged CROSS-CUTTING where relevant.

Files in scope: `documentation/scripts/goose-compat.js(+.test.js)`,
`documentation/src/utils/goose-compat.ts`, `documentation/src/utils/mcp-servers.ts`,
`documentation/src/utils/skills.ts`, `documentation/src/pages/skills/detail.tsx`,
`documentation/src/pages/skills/types/index.tsx`, `documentation/src/types/server.ts`,
`documentation/AGENTS.md`, `documentation/GOOSE_COMPATIBILITY.md`, root `AGENTS.md`,
`ui/desktop/src/acp/chatSessionController.ts(+tests)`, `chatSessionStore.ts`,
`sessions.ts`, `ui/desktop/src/hooks/useChatSession.ts(+Types)`,
`ui/desktop/src/components/BaseChat.tsx`, `ui/sdk/src/generated/*`, `README.md`
(non-execution-manager hunks), `ui/desktop/package.json` (version bump sanity check).

---

## 1. Findings — Fixed

### DWF-001: Silent failure on paginated history-load error

Severity: Medium · Confidence: Confirmed · Domain: Workflow-GUI (lying status)

Evidence: `ui/desktop/src/hooks/useChatSession.ts` — `loadOlderMessages` caught
fetch/pagination errors, logged `console.warn`, cleared the loading flag, and
returned. Nothing reached the user: the "load more" affordance would simply stop
spinning with no error surfaced, indistinguishable from "no more history."

Fix applied — added the same `toastError` dynamic-import pattern already used by
this file's `onMessageUpdate` catch block:
```ts
} catch (error) {
  console.warn('Failed to load older session messages:', error);
  acpChatSessionActions.setHistoryPageState(sessionId, { loading: false });
  const { toastError } = await import('../toasts');
  toastError({ title: 'Failed to load older messages', msg: errorMessage(error) });
}
```

### DWF-002: Stale-ref re-entrancy guard on `loadOlderMessages`

Severity: Medium · Confidence: Confirmed · Domain: Dataflow-state-transition (non-durable read before a mutating dispatch)

Evidence: `loadOlderMessages`'s entry guard read `getCurrentSnapshot()`, which
prefers a `useRef` cache (`snapshotRef.current`) populated only on React re-render,
over the always-fresh `acpChatSessionStore.getSnapshot(sessionId)`. Combined with an
un-throttled scroll listener (`BaseChat.tsx`), a fast scroll could fire
`loadOlderMessages` twice before the first dispatch's re-render lands, both reads
seeing `historyLoading: false` and issuing duplicate concurrent page fetches
against the same cursor.

Fix applied — guard now reads the store directly, dropping the stale-ref dependency:
```ts
const currentSnapshot = acpChatSessionStore.getSnapshot(sessionId); // was getCurrentSnapshot()
```
`acpChatSessionStore` was already imported in the file; no new import needed.

### DWF-003: Case-sensitive Goose→gosling rebrand leaves mixed-case source text unconverted

Severity: Low · Confidence: Confirmed · Domain: Dataflow-integrity (branding/provenance normalization)

Evidence: `documentation/scripts/goose-compat.js` and
`documentation/src/utils/goose-compat.ts` — `convertGooseText`/`convertGooseCommand`
used case-sensitive regexes (`/\bgoose\b/g`, etc.). `documentation/AGENTS.md:5,21`
mandates gosling is "ALWAYS" written lowercase. External Goose-catalog text with
`Goose`/`GOOSE` capitalization (title case, sentence case, all-caps) would pass
through unconverted, violating the branding invariant on a boundary explicitly
designed to enforce it.

Fix applied — regexes made case-insensitive (`gi` flag) in both files; removed the
now-redundant `\.goose\b` → `.gosling` rule (subsumed by the case-insensitive
`\bgoose\b` rule and was never doing independent work — confirmed by test
coverage, no behavior change).

### DWF-004: Silent drop of catalog entries with a missing `id`

Severity: Low · Confidence: Confirmed · Domain: Dataflow-integrity (silent data loss)

Evidence: `dedupeById` (`.js`) / `dedupeAndSortById` (`.ts`) skip any item lacking
`id` with a bare `continue` — no signal that an entry was discarded. A malformed
upstream Goose catalog entry (or a local catalog entry with a typo'd/missing `id`
field) vanishes from the rendered list with zero operator-visible trace.

Fix applied — same `continue`, now preceded by `console.warn("goose-compat:
dropping catalog entry with missing id", item)` in both the `.js` and `.ts`
implementations.

### DWF-005: Fallback-catalog logic didn't account for the dedupe step reducing a non-empty fetch to empty

Severity: Low · Confidence: Confirmed · Domain: Dataflow-pipeline-graph (fallback path never triggers when documented)

Evidence: `documentation/src/utils/mcp-servers.ts` (`fetchMCPServers`) and
`documentation/src/utils/skills.ts` (`fetchSkillsManifest`) checked only
`data.length === 0` (raw fetch result) before returning, then ran
`dedupeAndSortById` on the result. `GOOSE_COMPATIBILITY.md` documents falling back
to the Goose catalog "when local gosling catalogs are unavailable or empty" — but
if a raw non-empty local catalog fully collapsed to zero entries after DWF-004's
dedupe/missing-id filtering, the code still returned that empty array instead of
falling through to the next catalog, silently breaking the documented fallback
contract.

Fix applied — check emptiness *after* normalization, before returning, in both files:
```ts
const normalized = dedupeAndSortById(data.map(catalog.normalize));
if (normalized.length === 0) continue;
return normalized;
```
(`skills.ts` required an explicit `: Skill[]` annotation on `normalized` — see
Self-caused-and-fixed regression below.)

### DWF-006 (test coverage): No regression tests existed for DWF-003/DWF-004 mechanisms

Fix applied — added 2 tests to `documentation/scripts/goose-compat.test.js`:
`"rewrites Goose branding regardless of source casing"` and `"drops catalog
entries with a missing id and warns instead of silently discarding them"`.
Suite: 12 → 14 tests, all passing.

---

## 2. Self-caused-and-fixed regression (reported per honesty mandate)

While applying DWF-005 to `skills.ts`, introducing an intermediate
`const normalized = dedupeAndSortById(data.skills.map(manifest.normalize));`
(replacing a direct `return dedupeAndSortById(...)`) caused TypeScript to lose the
contextual typing that let it infer the exported generic `dedupeAndSortById<T>`'s
`T` as `Skill` — inferring a narrower `{id, name}` instead, since
`manifest.normalize` is a union of two differently-typed normalize functions
across the `manifests` array. Result: a new `TS2322` error at `skills.ts:118`
that did not exist on the original committed baseline (confirmed via
`git stash` / `npx tsc` / `git stash pop` before/after comparison — zero mentions
of `skills.ts` in the stashed baseline's tsc output).

Fixed by adding an explicit annotation restoring the lost context:
`const normalized: Skill[] = dedupeAndSortById(...)`. Re-verified clean (see §4).

---

## 3. Findings — Dispositioned (not fixed; reason + recommended owner)

### DWF-D1: `_meta.summary` field-name mismatch — `coverageThroughRowId` vs `coveredThroughRowId`

Severity: Low · Confidence: Confirmed · **CROSS-CUTTING → dataflow-architect**

Evidence:
- `crates/gosling/src/acp/server/load_session.rs:276-278` builds an ad-hoc
  `_meta.summary` JSON blob with keys `"coverageThroughRowId"` and
  `"coverageThroughTimestamp"` (manual string literals — note "coverage", not
  "covered").
- Every other reference to this concept — the Rust struct field
  (`covered_through_row_id`, `crates/gosling/src/session/session_manager.rs:131`
  and 10+ other sites), the ACP schema (`crates/gosling/acp-schema.json:3700,3725`),
  and the generated SDK (`ui/sdk/src/generated/types.gen.ts:1606`,
  `zod.gen.ts:1547`) — uses `coveredThroughRowId`.
- Checked: no code in my assigned frontend surface (`ui/desktop/src/`) reads
  either key from `load_session`'s response `_meta.summary` today — this blob is
  not yet consumed by the desktop client, so the mismatch is currently latent, not
  live-breaking.

Why not fixed by me: `load_session.rs` is ACP-server backend code adjacent to the
schema/contract surface (`acp-schema.json`, `custom_requests.rs`) explicitly
reserved for dataflow-architect's engagement, not one of my assigned files.
Flagging rather than editing per task instructions.

Recommendation: rename the two keys in `load_session.rs` to match the canonical
`coveredThroughRowId`/`coveredThroughTimestamp` before any frontend code starts
consuming this blob — otherwise the first consumer will silently read `undefined`.

### DWF-D2: `_meta.summary` blob has no frontend consumer yet (feature gap, not a bug)

Severity: N/A (feature gap) · **Recommended owner: whoever owns the compacted-resume UI (concurrency-engineer or a follow-up frontend ticket)**

The backend emits session-summary metadata (`status`, coverage-through position,
covered message count, `updatedAt`) alongside the compacted history load, but
none of my assigned frontend files (`chatSessionStore.ts`, `useChatSession.ts`,
`BaseChat.tsx`) read or surface it — there's no UI indicator today that a
session's early history was compacted/summarized rather than fully loaded. This
looks like an intentional half-shipped feature (backend ready, frontend pending)
rather than a defect. Not fixed — it's new-feature-shaped, not a bounded bug;
also blocked on DWF-D1 being resolved first (fixing the frontend to read the
wrong key would just bake in the mismatch).

### DWF-D3: `response.messages as Message[]` unchecked cast in ACP session loading

Severity: Low · Confidence: Plausible · **Recommended owner: needs a live app run to verify safely**

Evidence: `ui/desktop/src/acp/sessions.ts` casts the wire response's `messages`
array directly to `Message[]` without a runtime shape check. If a future/older
ACP server version's compacted-load response omits a field the `Message` type
requires, this fails silently at the type level (cast, not validation) and only
surfaces downstream as a rendering error. Not fixed: verifying the actual runtime
shape of `messages` across ACP server versions needs the app running against a
real backend, which this environment cannot do (per task constraints on
live-build-required items). Recommend a runtime schema check (zod, given
`ui/sdk/src/generated/zod.gen.ts` already exists) at this boundary in a follow-up.

### DWF-D4: Possible `messageId` collision risk in steer-message reconciliation

Severity: Low · Confidence: Plausible (not confirmed) · **Recommended owner: needs further investigation, low priority**

Evidence: `useChatSession.ts`'s `onSteerQueuedMessage` checks
`currentMessages.some((message) => message.id === response.messageId)` before
adding a locally-optimistic steer message, to avoid duplicating a message the
server already echoed back. If two steer calls raced and the server assigned
colliding or reused IDs (not observed, not proven possible from the code read
alone — would require tracing the server-side ID generator, which is out of my
assigned surface), a message could be silently deduplicated away. Flagged as
plausible-only; not fixed, as no concrete reproduction was found and the server
ID-generation code is outside my assigned slice.

---

## 4. Verification (exact tool output)

### `documentation` project

```
$ cd documentation && node --test scripts/*.test.js
```
Result: **14 tests, 14 pass, 0 fail** (was 12/12 before my 2 new regression tests).

```
$ cd documentation && npx tsc
```
Before/after diff of full tsc output (`git stash` baseline vs. working tree),
byte-for-byte: **IDENTICAL** — zero new errors, zero errors fixed, from any file
outside my touched set. My touched files (`goose-compat.ts`, `mcp-servers.ts`,
`skills.ts`) produce **zero** tsc errors in the after-state (confirmed via
targeted grep of the diffed output for each filename).

Pre-existing (unrelated, untouched) errors remain unchanged in:
`docusaurus.config.ts`, `src/components/Card/index.tsx`, `src/pages/extension.tsx`,
`src/pages/extensions/detail.tsx`, `src/pages/prompt-library/detail.tsx`,
`src/pages/prompt-library/index.tsx`, `src/pages/skills/detail.tsx` (2 pre-existing
errors — `Cannot find namespace 'JSX'` + a markdown-renderer `inline` prop — both
unrelated to the diff in this file I was asked to review), `src/theme/BlogListPage/index.tsx`,
`src/theme/BlogPostItem/Header/Info/index.tsx`, `src/theme/Root.tsx`, `src/utils/prompts.ts`.

### `ui/desktop` project

```
$ cd ui/desktop && npx tsc --noEmit
```
Result: passes clean (no errors) on the whole project after the `useChatSession.ts`
fix.

```
$ cd ui/desktop && ../node_modules/.bin/vitest run
```
Result: **Test Files 2 failed | 42 passed (44)**, **Tests 3 failed | 379 passed (382)**.

All 3 failures confirmed **pre-existing on the unmodified baseline** via
`git stash` / `vitest run` / `git stash pop` — identical failures reproduce on
the stashed original tree, with no file I touched (`useChatSession.ts` is the
only `ui/desktop` file I modified) anywhere near the failing code paths:
- `src/acp/__tests__/sessions.test.ts > returns session info refreshed after
  loading the ACP session` — asserts `client.loadSession` called without a
  `_meta.gosling` field that the original (pre-existing, un-modified-by-me)
  `sessions.ts` implementation actually sends. Pre-existing test/implementation
  mismatch from the original session-resume-paging commit (`23786c1d3`); I did
  not touch `sessions.ts`.
- `src/components/settings/auth/AuthSettingsSection.test.tsx` × 2 (VPS / Supabase
  secret profile save) — unrelated component, outside my assigned slice, outside
  the 8-commit diff entirely. Confirmed failing identically on the stashed
  baseline.

No network-dependent step (`pnpm install`) was required for either verification
pass — both projects' `node_modules` were already present and used as-is.

---

## 5. Summary

| Category | Count |
|---|---|
| Fixed (bounded, verified) | 5 findings (DWF-001–005) + 1 test-coverage addition (DWF-006) |
| Self-caused regression found & fixed before reporting | 1 (skills.ts generic-inference loss) |
| Dispositioned — CROSS-CUTTING to dataflow-architect | 1 (DWF-D1) |
| Dispositioned — feature gap, no owner-assignment bug | 1 (DWF-D2) |
| Dispositioned — needs live app / further investigation | 2 (DWF-D3, DWF-D4) |
| Pre-existing test failures confirmed unrelated (not fixed, not mine) | 3 |
| Files modified | `documentation/scripts/goose-compat.js`, `documentation/scripts/goose-compat.test.js`, `documentation/src/utils/goose-compat.ts`, `documentation/src/utils/mcp-servers.ts`, `documentation/src/utils/skills.ts`, `ui/desktop/src/hooks/useChatSession.ts` |
| Working tree state | All fixes uncommitted; no `git commit` issued |
