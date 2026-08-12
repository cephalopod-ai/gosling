# Navigation playtest, audit, and repair

## Executive verdict

The thread navigator's ordinary desktop path worked, but the navigation-focused
playtest confirmed five defects at the responsive, long-thread, accessibility,
performance, and history-failure seams. All five were repaired in one local UI
stage and passed the same rendered Chromium scenarios afterward. No Critical or
High defects were found; the repaired surface is ready for normal use with the
validation limits below.

## Scope and method

- Repository: `gosling`, branch `codex/full-audit-repair-20260812`
- Audit baseline: `53444c8fc` (navigation implementation `bfb3a4b23`, approved
  focused playtest harness `53444c8fc`)
- Lenses: `audit-playtest-app`, `audit-design-webapp` (all six gates, narrowed to
  thread navigation), `repair-defect-campaign`, and the local `webapp-testing`
  Chromium workflow
- Involvement: L2 standard, inferred from the specific patch-and-skill request;
  no approval gate was reached
- Effort budget: the complete thread navigator and its producer/consumer seams,
  its component/history tests, nine rendered navigation cases, and all six design
  gates for that surface
- Runtime: Vite-rendered production component in headless Google Chrome on macOS,
  1280x820, 900x820, and 390x820; no credentials, user sessions, or live Gosling
  data were used

The supplied request was treated as a focused draft. The pass preserved its
navigation mission while adding the adjacent failure mechanisms implied by it:
single-turn long answers, 100-turn stress, keyboard ordering, reduced motion,
repeated activation, pagination failure, and responsive behavior.

## Scenario library

- Source: existing `docs/test_scenarios/` library (110 cards), ordered by its
  README
- Selected cards: CH-03/04 (navigation while active and after reload), ST-02
  (navigation stress), SX-04 (hundred-turn history), SX-09 (navigation recovery),
  DT-03 (keyboard/focus), DT-05 (narrow window/long content), and DT-07
  (navigation state)
- The selected cards were exercised only to the extent they apply to the thread
  navigator in the isolated component surface. Provider turns, app relaunch,
  multi-window state, sidebar/settings destinations, artifact panes, and actual
  persisted-session export were not executed in this focused pass.
- Excluded card files and reason: 01 (first-run lifecycle), 03 (workspaces), 04
  (providers), 05 (extensions), 06 (skills/plugins/subagents), 07
  (import/export), 08 (permissions), 09 (CLI), 11 (headless/ACP), 13 (advanced
  CLI), 15 (context/filesystem), 16 (provider/network), 17 (ACP protocol), and 18
  (state/extension/permission depth) do not exercise the requested Desktop thread
  navigator. Within files 02, 10, 12, and 14, non-navigation cards were excluded
  for the same reason.

## Surface and boundary map

| Surface | Input/trigger | Output/state | Boundary | Reviewed |
|---|---|---|---|---|
| Start/latest buttons | pointer or keyboard | scroll position | available whenever history is scrollable | Yes |
| Turn rail | visible user messages | active location and jump | discoverable, bounded, keyboard usable | Yes |
| Progressive history | jump to an unmounted turn | full rendered history | target exists before scroll | Yes |
| Older-history pagination | jump to start | oldest known page | no false arrival after load failure | Yes |
| Responsive rail | 390/900/1280px | reachable navigation | important actions at every width | Yes |
| Motion | OS reduced-motion preference | scroll behavior | no unnecessary smooth motion | Yes |

Authoritative sources for this repair were `AGENTS.md`, `docs/architecture.md`,
`docs/test_scenarios/README.md` plus the selected cards, the semantic token and
930px breakpoint definitions in `ui/desktop/src/styles/main.css`, and the
component contracts/tests in `ui/desktop/src/components`. Pre-repair disposition:
conformant to the component boundary but with the five evidenced UX/runtime
defects below. Post-repair disposition: no new architecture or contract drift.

## Scenario results

| Scenario | Before | After |
|---|---|---|
| Five-turn pointer navigation and active marker | Pass | Pass |
| One short turn that fits | Pass (navigator omitted) | Pass (still omitted) |
| One long turn | Fail: no start/latest controls | Pass |
| 900px tablet and 390px mobile | Fail: navigator absent | Pass, no horizontal overflow |
| Hundred-turn history | Fail: ~6px targets, 100 Tab stops, 1,919 rect reads/20 scroll probes | Pass: 24px targets, roving Tab stop, 149 rect reads |
| Keyboard | Partial: every marker in Tab order | Pass: start → active turn → latest; Arrow/Home/End within turns |
| Reduced motion | Fail: requested `smooth` | Pass: requested `auto` |
| Four rapid latest activations | Pass | Pass; final distance from bottom 0px |
| Older-history load failure | Source-confirmed false jump risk | Pass: failed load returns false and does not jump |

Evidence screenshots and the JSON console result are generated by
`ui/desktop/navigation_playtest.py`; the final local evidence directory for this
run was `/tmp/gosling-navigation-playtest-final`.

## Findings and closure

### WFG-NAV-001: Thread navigation disappears at tablet and mobile widths

Severity: Medium
Confidence: Confirmed
Evidence basis: runtime-observed
Domain: Workflow-GUI
Primary gate: 5 (secondary: 1)

Evidence: baseline `bfb3a4b23`, `ThreadNavigator.tsx:186-190`, declared
`className="... hidden ... md:flex"`; the rendered 900px and 390px cases reported
`navigation_visible: false`.

Expected boundary: narrowing the window must not remove the only controls for
moving between thread locations. Observed behavior: the entire `nav` vanished
below the 930px `md` breakpoint. Mechanism: responsive display classes hid the
control rather than adapting it. Break-it result: both tablet and mobile widths
reproduced the loss; the control returned at 1280px.

Impact: users in a narrow window or split pane lost start, latest, and turn
navigation. Operational impact: long-thread recovery required manual scrolling
and the same conversation behaved differently solely because of window width.
Adjacent state: content still rendered and produced no console error; this was a
reachability defect rather than data loss.

Mitigation and implementation: keep the rail available at every width, reserve
content space, and verify no body overflow. Complexity/cost/agent:
`operator_ux`, S, Codex. Validation: post-repair 900px and 390px probes are
visible and have `scrollWidth == clientWidth`. Non-goal: redesigning the broader
mobile conversation chrome. Closure: resolved 2026-08-12.

### WFG-NAV-002: A single long answer has no start/latest navigation

Severity: Medium
Confidence: Confirmed
Evidence basis: runtime-observed
Domain: Workflow-GUI
Primary gate: 1

Evidence: baseline `bfb3a4b23`, `ThreadNavigator.tsx:182-184`, declared
`if (turns.length < 2) { return null; }`; the long-answer case measured 1,548px of
content in a 756px viewport with zero navigation landmarks.

Expected boundary: a thread that exceeds its viewport should retain start/latest
navigation even when it contains one user turn. Observed behavior: the same
single-turn thread lost all controls once the answer became long. Mechanism: turn
count was used as a proxy for scrollability. Break-it result: a short single turn
correctly needed no controls, while a long one exposed the defect.

Impact: users could not jump across a long first answer. Operational impact: a
common first-response review became a manual full-page scroll with no indication
that navigation was available elsewhere. Adjacent state: multi-turn navigation
continued to work.

Mitigation and implementation: observe viewport/content size and render
start/latest controls when the thread actually scrolls, while continuing to omit
them for a short fitting turn. Complexity/cost/agent: `local_guardrail`, XS,
Codex. Validation: long and short single-turn regressions pass. Non-goal:
generating artificial turn markers for assistant-only content. Closure: resolved
2026-08-12.

### WFG-NAV-003: Hundred-turn navigation collapses targets and scales poorly

Severity: Medium
Confidence: Confirmed
Evidence basis: runtime-observed
Domain: Workflow-GUI
Primary gate: 4 (secondary: 6)

Evidence: baseline `bfb3a4b23`, `ThreadNavigator.tsx:86-93`, ran
`turns.forEach(...)` with a selector and geometry read per turn, while
`ThreadNavigator.tsx:208-225` mapped every button into one `justify-evenly`
track with `h-4 ... shrink`. The runtime probe measured 100 targets at roughly
6.03px, 100 sequential turn Tab stops, and 1,919 `getBoundingClientRect` calls
over 20 scroll probes.

Expected boundary: a large thread should preserve operable targets, bounded
keyboard traversal, and scroll work proportional to visible landmarks. Observed
behavior: the rail compressed all targets and made every one a sequential Tab
stop while scanning every turn per animation frame. Mechanism: flex shrinking,
default button tab order, and a full linear geometry scan. Break-it result: the
five-turn case held, but the 100-turn case reproduced all three failures.

Impact: targets became difficult to click, keyboard traversal became impractical,
and scrolling performed avoidable layout reads. Operational impact: the feature
degraded most on the long investigative threads for which it was intended.
Adjacent state: active-location selection and ordinary turn jumping still worked.

Mitigation and implementation: scroll the marker track with fixed 24px targets,
use a roving Tab stop plus Arrow/Home/End, and binary-search ordered mounted
landmarks. Complexity/cost/agent: `operator_ux`, S, Codex. Validation: the same
probe measured 24px targets and 149 rectangle reads; Tab order and Arrow/Home/End
passed. Non-goal: virtualizing the marker buttons themselves. Closure: resolved
2026-08-12.

### WFG-NAV-004: Navigation ignores reduced-motion preference

Severity: Low
Confidence: Confirmed
Evidence basis: runtime-observed
Domain: Workflow-GUI
Primary gate: 4

Evidence: baseline `bfb3a4b23`, `ThreadNavigator.tsx:170`, and
`BaseChat.tsx:400-429` passed `behavior: 'smooth'` on every navigation path; a
reduced-motion browser context observed `smooth`.

Expected boundary: the OS/browser reduced-motion preference should suppress
nonessential animated navigation. Observed behavior: turn, start, latest, and
progressive-target jumps always requested smooth motion. Mechanism: literal
scroll behavior at each call site. Break-it result: the browser media emulation
changed the preference but not the requested behavior.

Impact: navigation could trigger unwanted motion for motion-sensitive users.
Operational impact: no data or location corruption, but an accessibility
preference was ignored consistently. Adjacent state: focus and final scroll
destinations remained correct.

Mitigation and implementation: route thread navigation through a shared
motion-aware scroll behavior. Complexity/cost/agent: `local_guardrail`, XS,
Codex. Validation: unit and rendered tests now observe `auto` under reduced
motion and `smooth` otherwise. Non-goal: changing unrelated application
animations. Closure: resolved 2026-08-12.

### WFG-NAV-005: Failed history loading still jumps to a false “start”

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Workflow-GUI
Primary gate: 1 (secondary: reliability)

Evidence: baseline `bfb3a4b23`, `useChatSession.ts:240-261`, converted a page
exception to `false` but exposed no result from `loadAllOlderMessages`; baseline
`BaseChat.tsx:400-404` then unconditionally called
`scrollToPosition({ top: 0, behavior: 'smooth' })`.

Expected boundary: “jump to start” must move only after the true oldest page is
known, or stay put after a load error. Observed behavior: a toast disclosed the
failure but the UI still moved to the earliest loaded page as though it were the
real beginning. Mechanism: the loader discarded its completion outcome and the
consumer assumed success. Break-it result: a rejected history-page request
preserved `historyHasMore` yet still reached the unconditional scroll path.

Impact: the active location lied about being the thread start. Operational
impact: users could miss older context and retry from an unexpected position.
Adjacent state: the cursor remained available and the error toast was correct.

Mitigation and implementation: return whether the oldest page was actually
reached and suppress the jump on failure while retaining the cursor for retry.
Complexity/cost/agent: `workflow_protocol`, S, Codex. Validation: success and
rejected-page tests at the real hook/store boundary pass. Non-goal: changing the
history backend or retry policy. Closure: resolved 2026-08-12.

## Six-gate scorecard

| Gate | Verdict after repair | Coverage | Evidence |
|---|---|---|---|
| 1 — Product/workflow | Pass | Full for thread navigation | pointer jumps, single-long-turn controls, failure truth, and rapid actions passed |
| 2 — Front-end handoff | Partial | Partial | existing semantic tokens/states were reused; no separate design artifact was in scope |
| 3 — Web standards | Pass | Full for component markup | semantic `nav` and native buttons; zero rendered console errors |
| 4 — Accessibility | Pass | Full for navigation | named controls, visible focus classes, roving keyboard model, 24px targets, reduced motion |
| 5 — Device/browser resilience | Partial | Partial | Chrome desktop/tablet/mobile passed; Firefox/Safari/Edge were not run |
| 6 — Production readiness | Pass | Full for the changed surface | 100-turn measured probe, pagination failure regression, unit/type/lint suites |

## Ranking and repair stage

Priority score order: WFG-NAV-003 (6.0, quick win), WFG-NAV-001 (6.0, quick
win), WFG-NAV-002 (6.0, quick win), WFG-NAV-005 (4.0, quick win), WFG-NAV-004
(3.0, quick win). They shared the same component/history path and therefore
formed one repair stage. All heavily edited source files were below 1,000 lines;
no file met the campaign's modularization threshold.

The stage added behavior regressions for long/short single turns, roving
keyboard focus, reduced motion, pagination success/failure, and a reusable
Chromium playtest harness with explicit assertions. No source finding or TODO
predated this discovery, so this report and the repo-native session log are the
closure records; no stale in-code marker was found.

Repair checkpoint: `92b800e18` (`fix(desktop): harden thread navigation edge
cases`).

## Non-findings and break-it results

- Pointer jumping to a normal turn held, including Unicode/emoji preview text.
- Active location follows scroll and the last turn wins at the bottom.
- Progressive rendering still materializes an unmounted target before jumping.
- Four rapid latest activations were idempotent and ended exactly at the bottom.
- The repaired rail produced no horizontal page escape at 390px.
- Inactive marker contrast in the isolated dark theme measured 16.23:1; no
  color-only current-location claim is required because `aria-current` is set.

## Skill escalation

| Finding | Primary lens | Secondary lens | Why |
|---|---|---|---|
| WFG-NAV-001/002 | Workflow/GUI | Device resilience | reachability depended on width/content |
| WFG-NAV-003 | Accessibility | Performance | the same long-thread design caused target and per-frame scaling defects |
| WFG-NAV-004 | Accessibility | Web standards | motion behavior crossed component call sites |
| WFG-NAV-005 | Workflow/GUI | Reliability | UI location truth depended on pagination failure semantics |

## What would falsify my strongest conclusion?

The conclusion that navigation now survives long histories would be falsified by
an actual Electron session whose Radix viewport orders or positions turn landmarks
differently from this Vite-rendered component surface, or by a browser where
`scrollend`/scroll behavior diverges enough to leave stale selection state. A future
full Electron pass with a seeded 100-turn session in Safari/WebKit-equivalent and
native macOS accessibility inspection is the highest-value follow-up.

## Validation limits and residual risks

Completed validation:

- final focused Chromium playtest assertions: pass
- Desktop full Vitest: 88 files / 581 tests passed
- Desktop typecheck, ESLint, and i18n validation: pass
- targeted Prettier for every changed TS/TSX file: pass
- `cargo fmt --all -- --check`: pass
- `cargo clippy --all-targets -- -D warnings`: pass
- `git diff --check`: pass

The repository-wide `pnpm run format:check` remains red on 61 pre-existing,
unrelated files; none is part of this change and every changed TS/TSX file passed
the targeted formatter check.

- This was a focused component playtest, not the full Electron application or full
  110-card library.
- Current Chrome was tested; Firefox, Safari/WebKit, Edge, touch hardware, screen
  reader output, zoom, and CSS-disabled fallback were not executed.
- The history-failure seam was verified in the real hook/store boundary with a
  deterministic rejected page, not an external backend outage.
- The 100-turn timing includes two animation frames per sample; rectangle-read
  count is the comparison metric, not an end-user latency benchmark.

Final confidence: High for the repaired navigation surface, Medium for cross-browser
and full-Electron equivalence.
