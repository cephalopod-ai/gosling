# Gosling Desktop playtest report — 2026-07-19

Authority: `audit-playtest-app` report-only playtest. Scope was capped at 500
deterministic permutations; the matrix definitions are in
`2026-07-19-gosling-desktop-playtest-matrix.json`.

## 1. Summary

The current checkout launched successfully in a disposable Electron profile.
Workspace creation, explicit new-chat selection, basic streaming, queueing,
credential-form clearing, archive recovery, provider-outage recovery, and
restart persistence passed in the exercised cases. Two reproducible artifact
workflow defects were found: an artifact link opened the correct Outputs pane
but the preview was denied, and the pane close target was blocked by the
titlebar drag region. Both findings were repaired and re-verified in the same
isolated profile on 2026-07-19.

No package installation, GUI reinstall, source modification, commit, or push
was performed during the initial playtest. The subsequent repair is recorded
in `docs/build/sessions/2026-07-19-playtest-repair.md`. All credentials were
fake/transient; no external site was used.

## 2. Repository understanding and run method

Gosling is a Rust backend with an Electron/Vite desktop renderer. The tested
desktop path was `ui/desktop`; the app was launched directly through the
existing Electron Forge command to avoid the repository helper that installs
packages and builds a release binary. The isolated runtime root was
`/tmp/gosling-playtest.zVEbFP`.

## 3. Inputs and outputs discovered

- Input surfaces: onboarding provider setup, Workspaces sidebar, New Chat
  workspace selector, workspace editor, credential-profile modal, session
  actions, model picker, chat input, Message Queue, artifact links.
- Outputs: sidebar workspace/session rows, pinned workspace header, right-side
  Outputs workbench, archive-folder settings state, error/resend state.
- Disposable fixture: `Outputs/report.md` under the isolated workspace root.

## 4. Execution coverage

| Status | Count | Notes |
| --- | ---: | --- |
| Matrix definitions generated | 500 | Stable capped permutation space; see matrix file. |
| Interactive representatives executed | 10 | Cards PLY-001 through PLY-010. |
| Pass/recovery | 8 | Includes queue, archive setup recovery, and outage recovery. |
| Initial confirmed failures | 2 | PLY-GOS-001 and PLY-GOS-002. |
| Repaired and re-verified | 2 | Both original reproductions pass after repair. |
| Unexecuted permutations | 490 | Not represented as passes; queued for follow-up. |

## 5. Findings

### PLY-GOS-001 — artifact link opens pane but preview is denied (RESOLVED)

- Severity: High
- Confidence: Confirmed, runtime-observed
- Location: chat artifact link → right-side Outputs workbench; renderer file-access approval boundary.
- Reproduction: create a workspace whose primary folder is
  `/tmp/gosling-playtest.zVEbFP/work`, add output
  `/tmp/gosling-playtest.zVEbFP/work/Outputs`, send an artifact reference to
  `/private/tmp/gosling-playtest.zVEbFP/work/Outputs/report.md`, then click
  `Open report.md in Outputs`.
- Expected: the linked file is opened and its content is rendered.
- Actual: the correct pane/tab opens, then displays `Preview unavailable` and
  `Renderer file access denied for path outside approved roots`, with the
  fallback `/tmp/.../report.md` path shown.
- Impact: the direct artifact affordance does not complete the promised
  view-in-sidebar workflow; users still need to locate/open the file manually.
- Evidence: exact UI strings and the accessible artifact button were observed
  in the isolated run; the file existed and was inside the configured output.
- Suggested next test: trace canonical-path normalization and approved-root
  registration from `ArtifactMessageLinks` through the renderer file bridge;
  test both `/tmp` and macOS `/private/tmp` forms.
- Repair (2026-07-19): canonical existing workspace output roots now participate
  in artifact-file authorization, and the pane retries the bounded route-
  publication race before presenting an error. The original persisted
  `/private/tmp` reproduction rendered the report content after restart.
- Closure evidence: `ui/desktop/src/main.ts`,
  `ui/desktop/src/utils/artifactFileAccess.ts`, and the ArtifactPane retry
  regression; isolated Electron replay passed.

### PLY-GOS-002 — Outputs close target is intercepted by titlebar drag region (RESOLVED)

- Severity: Medium
- Confidence: Confirmed, runtime-observed
- Location: Outputs pane header close button and AppLayout titlebar drag region.
- Reproduction: open an artifact link so Outputs is visible; activate
  `Close outputs pane` with a pointer-equivalent click.
- Expected: the pane closes. Keyboard Enter on the focused close control should
  provide an accessible fallback.
- Actual: pointer activation was intercepted by the overlapping
  `.titlebar-drag-region`; the visible close target had a bounding box inside
  the drag region. Enter did not close the pane in this run.
- Impact: once an artifact pane is opened, users can be trapped in the pane or
  forced to use another navigation path.
- Suggested next test: assert hit-testing/z-index separation between titlebar
  drag and interactive controls at multiple window sizes.
- Repair (2026-07-19): the Outputs pane stacking context is above the native
  titlebar drag overlay and its interactive chrome is marked `no-drag`. The
  original pointer-equivalent click now activates the close control.
- Closure evidence: `ui/desktop/src/components/Layout/AppLayout.tsx`,
  `ui/desktop/src/components/artifacts/ArtifactPane.tsx`, and the pane-control
  regression; isolated Electron replay passed.

## 6. Passed/recovered behaviors

- Onboarding and local custom-provider setup worked with no real credentials.
- Workspace create/validate/save worked and persisted a sidebar row.
- New Chat explicitly accepted Playtest Workspace and used its primary folder.
- Session header retained the selected workspace.
- Slow streaming kept the current generation active while a follow-up appeared
  in Message Queue.
- Credential profile modal offered password input; cancel removed the field and
  did not retain the sentinel.
- Archive without a configured folder surfaced an actionable Settings state.
- Provider outage surfaced a visible network error and resend instruction.
- Relaunch with the same user-data root restored the workspace and prior thread.

## 7. Partial or not-confirmed areas

- Full alternate-provider/model switch was not credited as pass/fail. The first
  fixture advertised a different inventory than the configured model and
  produced `RequestError: Invalid params`; this was test-fixture mismatch.
- The queue item was observed, but the send-now/steer action was not credited
  without a clean compatible fixture and a captured control activation.
- Successful archive-file write and native Save a copy were not executed to
  avoid opening a native file chooser in the disposable run.

## 8. Process hygiene

The dev Electron process, its backend lease, and all local mock-provider
listeners were stopped after testing. Ports 9237 and 45678–45683 were checked
and left unused. The pre-existing installed Gosling process/backend was not
terminated. No source files were changed by the playtest.

## 9. Recommended follow-up

1. Rerun the unexecuted queue-steer, alternate-model, native-save, and missing-
   folder permutations with compatible disposable fixtures.
2. Re-run the 500-case matrix after repairs; report the executed count
   separately from the generated count.
