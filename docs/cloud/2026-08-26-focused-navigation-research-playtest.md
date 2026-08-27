# Focused Navigation and Deep Research Playtest

**Date:** 2026-08-26  
**Target:** `/Users/eric/Work/vscode/forked/gosling`  
**Branch / HEAD:** `main` / `82e676be3809739ebc079f87811b7139d9234751`  
**Result:** Partial pass — the focused GUI paths passed; a credential-free profile could not execute a live Deep Research model turn.

## Executive summary

The current-source Electron app passed the focused GUI playtest for the information-architecture change. Skills and Extensions are absent from the primary sidebar, both are available as Settings tabs, keyboard navigation moves between them, Escape returns to the prior surface, and the Settings view does not create document-level horizontal overflow at a 700 × 700 viewport.

The zero-Initial-Inputs Deep Research intake also rendered correctly and did not require a file selection. The disposable profile stopped before session launch because the required `math_mcp` extension was not configured. No provider credentials were added, so the model-side no-file prompt and invalid-`source` delegate fail-fast behavior were not exercised as a live turn. Those paths remain supported by the targeted prompt and Rust schema/normalization tests, but this report does not count automated checks as GUI playtest proof.

No renderer console errors or uncaught page errors occurred.

## Scope and method

This was a focused pass over the changes requested immediately before the playtest. It used the standing scenario library where applicable and added one run-specific variation because no card currently covers a zero-input Deep Research launch.

| Scenario | Coverage | Result |
| --- | --- | --- |
| ST-02, sidebar/settings navigation variation | Primary destinations, Settings entry, Skills/Extensions relocation | Pass |
| SK-01, catalog-view variation | Open installed Skills list from Settings | Pass for listing; skill invocation not run |
| DT-03, keyboard variation | Arrow-key Settings tab selection and Escape exit | Pass |
| DT-05, narrow-layout variation | Settings and research intake at 700 × 700 | Pass |
| DR-ZERO-01, run-specific | Open Deep Research with no Initial Inputs | Partial pass; intake passed, live turn blocked by clean-profile configuration |

Excluded from this focused run: all unrelated scenario cards, plugin installation/update, destructive or networked extension actions, provider-backed chat completion, session archive/report persistence, and the full automated test suites.

## Environment and data controls

- macOS Electron development build from current source
- Light theme
- Desktop viewport: 1880 × 1600
- Narrow viewport: 700 × 700
- Fresh temporary `GOSLING_PATH_ROOT` and Electron user-data directory created by the E2E fixture
- No credentials, user sessions, user files, or provider accounts used
- Temporary profile removed by the fixture after each run

## Observed results

### Primary navigation and Settings

- Primary sidebar contained New Chat, New Research, and Session History, with Settings pinned at the bottom.
- No primary-sidebar Skills or Extensions buttons were present.
- Settings exposed Skills and Extensions as tabs.
- Skills loaded three installed catalog entries with no catalog error.
- Extensions rendered the default extension cards, including Skills and Summon.
- Focusing Skills and pressing ArrowRight selected Extensions.
- Pressing Escape closed Settings and returned to the main surface.

### Narrow layout

- At 700 × 700, `document.body.scrollWidth` and `document.body.clientWidth` were both 700 pixels.
- The Settings tab strip remained horizontally scrollable and the selected Skills tab remained reachable.
- The Deep Research form remained usable through vertical scrolling; no content overlapped the sidebar.

### Deep Research with no Initial Inputs

- New Research opened the Deep Research intake.
- The Initial Inputs control was available but no input file was required to render or use the intake.
- The disposable profile displayed one actionable launch blocker: `Deep Research requires configured extension: math_mcp.`
- Because the required extension and a provider credential were intentionally absent, the session was not submitted and no delegate call was made.

### Runtime signals

- Renderer console errors: 0
- Uncaught page errors: 0
- Final route: `http://localhost:5173/#/research`

## Findings

### PT-01 — E2E launcher masks an incompatible pnpm failure

**Severity:** Low, test-infrastructure usability  
**Status:** Open

Launching the focused Playwright test outside the documented Hermit environment selected pnpm 10.6.4 while the desktop package requires pnpm 10.30.0 or newer. `pnpm run start-gui` exited immediately, but the Electron fixture continued polling the CDP port for 120 seconds and ultimately reported only `ECONNREFUSED`.

The same playtest passed when started after `source ../../bin/activate-hermit` (pnpm 10.30.3). The fixture should surface an early child-process exit and its stderr so an environment error fails immediately instead of consuming the full connection timeout.

### PT-02 — No standing scenario covers zero-input research and delegate payload rejection

**Severity:** Coverage gap  
**Status:** Open

The scenario library has no card that explicitly verifies both of these behaviors:

1. A Deep Research session with no Initial Inputs must not initiate workspace-file discovery.
2. An ad-hoc delegate request must omit `source`; an invalid or blank `source` must fail once without retry churn.

This run covered the GUI intake only. A future scenario should use a disposable deterministic provider/extension fixture so the system prompt, tool payload, single-failure behavior, and absence of file-inspection calls can be observed without real credentials.

## Evidence

Playwright result: `1 passed (24.3s)` using the repository Hermit toolchain.

Generated evidence is in `ui/desktop/test-results/focused-playtest/`:

- `01-primary-sidebar.png`
- `02-settings-skills.png`
- `03-settings-extensions.png`
- `04-settings-narrow.png`
- `05-zero-input-research-intake.png`
- `observations.json`

The evidence directory is intentionally ignored by Git. This Markdown report is the durable record.

## Disposition

The relocated Skills/Extensions UI is acceptable for the tested desktop and narrow-window paths. The live zero-input and delegate fail-fast workflow remains partially validated because the credential-free playtest environment could not launch the required model turn. No source fix was made during this playtest.
