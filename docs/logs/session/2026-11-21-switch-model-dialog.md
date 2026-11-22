# 2026-11-21 — Desktop model switch dialog

The reported dialog clipped its title and confirmation controls. The renderer
reproduced this at 680 × 540 with a queued cross-provider handoff. The original
dialog measured about 906 px high and began 183 px above the viewport. It also
displayed row IDs, token estimates, and tool diagnostics before the user opened
any details.

`SwitchModelModal.tsx` caps the dialog to the window and scrolls the form between
the title and actions. Shorter warnings stay next to the continuity label.
**Handoff details** exposes checkpoint coverage, redactions, and tool continuity
before confirmation. `Select.tsx` lets fixed dropdowns respect the space
available in the viewport. The help link and model controls stay accessible.
ADR-0019 and `docs/architecture.md` still describe the same preview and atomic
transition contract. The patch changes renderer presentation and localization.

## Validation

- `pnpm test:run src/components/settings/models/subcomponents/SwitchModelModal.test.tsx src/components/ui/dialog.test.tsx` passed all 20 tests. The updated preview test covers visible warnings, keyboard focus, collapsed and expanded details, and preview invalidation after editing the token limit.
- Chromium loaded the source renderer with a fixture for the provider and ACP
  preview. At 680 × 540 the revised dialog measured 508 px high and started at
  16 px. Checks at 480 × 400, 320 × 300, and 980 × 860 also kept the title and
  actions visible after scrolling. Dropdowns accepted keyboard input, exposed
  clickable options, and closed on Escape. Both switch confirmation paths reached
  the fixture handler and returned focus to the opener.
- Desktop typecheck, scoped ESLint and Prettier, Cargo formatting, localization
  compilation, and locale validation passed. The full extraction check still
  reports the three pre-existing `settings.tools.autoApprove.*` omissions.
  A scoped comparison verified all six edited or added switch messages against
  fresh extraction. All 21 localization synchronization tests passed.

To replay in Desktop, resize the window to about 480 × 400, review a model
switch, then expand **Handoff details** and scroll through it. Cancel and the
confirmation action should stay in view. Packaged Electron and live provider
transitions were outside this replay.
