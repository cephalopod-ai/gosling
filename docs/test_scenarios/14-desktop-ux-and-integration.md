# 14 — Desktop UX and Native Integration

Desktop must remain operable across onboarding, windows, keyboard input,
artifacts, archived sessions, external backends, and macOS integration. Record
screenshots and both renderer/backend logs for every failure.

---

### DT-01 — Onboarding interruption and resume
- Goal: an unconfigured user can leave and resume onboarding without a dead end.
- Category: first launch / interruption
- Preconditions: fresh disposable root and Desktop installation.
- Steps: launch; advance one screen; go back; quit on each major step in separate fresh runs; relaunch; finally configure a test provider and create a chat.
- Expected: every relaunch shows either the first incomplete step or an explicit restart choice; Back never loses already validated non-secret input unexpectedly; completion transitions once to a usable chat; no duplicate provider entries.
- Observe: keyboard focus, default button, and secret-field clearing after quit.

### DT-02 — Window close versus application quit
- Goal: macOS close, reopen, quit, and relaunch have distinct, honest lifecycle behavior.
- Category: lifecycle / persistence
- Preconditions: Desktop with two chats, one actively streaming from a controllable delayed fixture.
- Steps: close the active window with the red control; reopen via Dock/menu; close all windows; use `Cmd+Q`; relaunch and reopen both sessions.
- Expected: close behavior matches app convention and does not orphan an invisible unbounded run; quit terminates backend children within 10 seconds; relaunch shows honest terminal state and persisted completed history.
- Observe: Dock indicator, menu enablement, and approval dialogs owned by a closed window.

### DT-03 — Keyboard-only navigation and focus
- Goal: all primary Desktop workflows are reachable without a pointer.
- Category: accessibility / navigation
- Preconditions: Desktop configured with one session and one workspace.
- Steps: from launch, use Tab/Shift-Tab, arrows, Enter, Space, Escape, and documented shortcuts to create/open a chat, send text, stop a run, visit each navigation item, open/close a dialog, and return to composer.
- Expected: focus is always visible; order follows visual/logical order; no keyboard trap; Escape closes only the top modal; sending and stopping work; focus returns to the invoking control after close.
- Observe: screen-reader names for icon-only controls using macOS Accessibility Inspector if available.

### DT-04 — Shortcut rebinding, conflicts, and persistence
- Goal: user-defined shortcuts validate conflicts and survive relaunch.
- Category: settings / persistence
- Preconditions: Desktop Keyboard settings; record defaults and real system-level shortcuts to avoid.
- Steps: assign an unused combination; invoke it; try a duplicate app shortcut, a reserved macOS shortcut, and an incomplete chord; reset one binding; quit and relaunch.
- Expected: valid binding works exactly once; conflicts are rejected or require explicit resolution; incomplete input cannot erase a binding; reset restores documented default; final values persist.
- Observe: global versus app-local scope and behavior when focus is in the composer.

### DT-05 — Narrow window, resize, and long-content layout
- Goal: Desktop remains usable over its supported size range.
- Category: boundary / navigation
- Preconditions: session containing a long unbroken URL, wide code block, long tool name, deep list, and long workspace/model names.
- Steps: resize gradually from large to minimum width/height; toggle sidebar and artifact pane; scroll history; open settings and approval dialog at minimum size; return to large size.
- Expected: no overlapping controls, unreachable buttons, horizontal page escape, blank panel, or lost content; intended panes scroll independently; layout recovers after expansion.
- Observe: text truncation has accessible full-name affordance where selection depends on it.

### DT-06 — Artifact preview type matrix
- Goal: supported artifacts preview safely and unsupported content fails clearly.
- Category: files / boundary
- Preconditions: small known fixtures for Markdown, JSON, HTML, image, PDF, SVG, empty file, unknown binary, and a missing path.
- Steps: open each fixture through the artifact links/workbench; switch tabs rapidly; use Save a copy where offered; compare source hash before and after preview.
- Expected: supported formats render the intended content; active content cannot execute privileged app actions; malformed/unknown/missing files show a bounded error; preview never mutates source; saved copy hash matches source where no conversion is promised.
- Observe: large-file warning and renderer console errors.

### DT-07 — Artifact workbench state across navigation and relaunch
- Goal: pane open state, width, tabs, and active selection restore without stale-file confusion.
- Category: persistence / navigation
- Preconditions: three artifact tabs from two sessions/workspaces; record tab order and pane width.
- Steps: resize and select the middle tab; navigate away and back; close one tab; quit/relaunch; move one source file before another relaunch.
- Expected: navigation preserves current state; closed tab stays closed; relaunch restores only documented state; moved source becomes a named missing-file state and is not replaced with another file of the same basename.
- Observe: state isolation across multiple Desktop windows.

### DT-08 — Archive and restore session lifecycle
- Goal: archiving changes visibility, not history integrity.
- Category: delete-undo / persistence
- Preconditions: three completed sessions including one pinned workspace session and one open in another window.
- Steps: archive one from history; inspect Active and Archived tabs; attempt to open the archived session from stale UI; restore it; archive the cross-window session; relaunch.
- Expected: membership changes exactly once; archive preserves ID/history/exportability; restore returns the same ID; stale views refresh or explain state; cross-window action cannot create a duplicate or zombie.
- Observe: counts, selection after removal, and CLI list agreement.

### DT-09 — External backend authentication and reconnect
- Goal: Desktop can switch to a remote gosling backend and recover from bad settings.
- Category: settings / recovery
- Preconditions: local `gosling serve` on a test port with known secret and certificate mode; embedded backend healthy.
- Steps: configure correct URL/secret and connect; send a marker turn; change to wrong secret, unreachable port, malformed URL, and wrong TLS expectation one at a time; restore correct settings; switch back to embedded backend.
- Expected: correct remote works; each fault is distinguished and bounded; secret is never displayed after save; reconnect succeeds without resetting unrelated settings; sessions are attributed to the backend that owns them.
- Observe: retry cadence and whether an old authenticated socket survives credential replacement.

### DT-10 — Native notifications and denied permission
- Goal: task notifications respect app setting and macOS permission state.
- Category: settings / interruption
- Preconditions: controllable delayed completion; Desktop notification toggle; ability to reset permission for the test app if safe.
- Steps: enable notifications and complete a backgrounded task; repeat in foreground; deny macOS permission and retry; disable in-app notifications and retry; click a delivered notification.
- Expected: eligible background completion sends at most one notification; foreground/disabled behavior matches the setting; OS denial produces no retry storm and offers an actionable settings path; clicking focuses the correct session.
- Observe: cancelled/failed runs use accurate wording and never claim success.
