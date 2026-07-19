# Gosling Desktop playtest plan — 2026-07-19

Authority: `audit-playtest-app` (report-only)

Scope: Gosling Desktop workspace creation/loading, new-chat selection, chat
generation, archive/save surfaces, credential-profile forms, model switching,
artifact routing/viewing, prompt queue/steering, and restart persistence.

Safety boundary: this run used a disposable Electron user-data root and
temporary folders under `/tmp/gosling-playtest.zVEbFP`. A local OpenAI-compatible
fixture supplied deterministic responses; no real provider credentials or
external websites were used. No package installation, GUI reinstall, source
repair, commit, or push was performed.

## Repository and launch contract

- Repository: `cephalopod-ai/gosling`, branch `main`, clean before the run.
- Renderer: `ui/desktop` Electron/Vite desktop app.
- Launch: `GOSLING_PATH_ROOT=/tmp/gosling-playtest.zVEbFP ENABLE_PLAYWRIGHT=true PLAYWRIGHT_DEBUG_PORT=9237 GOSLING_ALLOWLIST_BYPASS=true pnpm --dir ui/desktop exec electron-forge start -- --user-data-dir=/tmp/gosling-playtest.zVEbFP/electron`.
- Existing dependencies were used as-is. `pnpm install` and reinstall/build of the packaged GUI were deliberately not run.

## Deterministic permutation space

The matrix is generated in `2026-07-19-gosling-desktop-playtest-matrix.json`.
The dimensions intentionally cover the requested workflows:

| Dimension | Values |
| --- | --- |
| workflow | create-project, new-chat, archive-save, credentials, model-switch, artifact, queue-steer, workspace-load, recovery, security-boundary |
| workspace state | default, valid-primary, multi-folder, optional-missing, required-missing |
| provider state | configured-local, configured-alternate, setup-required, unavailable |
| prompt mode | initial, queued, steered, resend, cancel |
| artifact kind | document, image, presentation, download |

The full Cartesian space is intentionally capped at 500 cases in stable
lexicographic order. The matrix contains all 500 case definitions; only the
cases listed in the report's execution table were run interactively during
this pass. The remaining cases are executable follow-ups, not implied passes.

## Walkthroughs

1. Create a disposable provider, create a workspace with a primary folder and
   output destination, validate, save, and confirm the row appears in the
   sidebar.
2. Open New Chat, choose the workspace explicitly, submit a prompt, and verify
   the session header retains the pinned workspace.
3. Generate a document reference, follow the artifact link, inspect the
   right-side Outputs pane, preview content, and close the pane.
4. Open session actions, exercise archive/save affordances, and verify the
   missing archive-folder recovery state.
5. Open workspace credential profiles, inspect metadata-only listing, enter a
   sentinel into the password field, cancel, and verify the value is cleared.
6. Open the in-session model switcher, cancel safely, then exercise a configured
   alternate model when the provider inventory is compatible.
7. Start a deliberately slow response, queue a follow-up, inspect the queue,
   and attempt the send-now/steer control without interrupting the generation.
8. Stop the fixture, relaunch the same disposable user-data root, and verify
   workspaces and the prior session remain readable.

