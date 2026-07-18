# Gate 7 evidence — documentation

Date: 2026-07-18

## Documentation delivered

- Added the Desktop Workspaces user guide covering lifecycle, session pinning, folders, product
  outputs, credential profiles, persistence, recovery, and limitations.
- Linked Workspaces from the documentation index, Desktop navigation, session management,
  configuration files, and custom-distribution guide.
- Added a complete non-secret `GOSLING_WORKSPACE_TEMPLATES` example and corrected secure
  provisioning guidance to use mechanisms that exist in gosling.
- Added explicit Desktop controls for workspace validation, credential-profile status testing,
  and export routing so the documented workflows match the product.

## Commands and results

| Command | Result |
| ------- | ------ |
| `source bin/activate-hermit && cd ui/desktop && pnpm run typecheck` | pass |
| focused workspace-dialog Vitest run | pass: 16 tests |
| `source bin/activate-hermit && cd documentation && npm test` | pass: 15 tests |
| `source bin/activate-hermit && cd documentation && npm run build` | pass: 166 Markdown files exported and static site built |
| `source bin/activate-hermit && cd documentation && npm run typecheck` | baseline failure: unrelated Docusaurus configuration/React namespace and prompt-model typing errors |

## Gate decision

Gate 7 passes. The new source documentation builds into the production site, and the user-facing
controls documented in the guide are covered by focused Desktop tests. The pre-existing
documentation TypeScript failures are recorded for Gate 8 handoff and are not caused by the
Workspaces Markdown changes.
