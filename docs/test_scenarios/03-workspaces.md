# 03 — Workspaces (Desktop)

Workspaces define primary folders, product outputs, and credential-profile
bindings for new Desktop chats. CLI continues to use process cwd + global
provider config; mark CLI-only passes **Not applicable** for this file when
Desktop is unavailable.

---

### WS-01 — Create workspace and pin new chat
- Goal: create a workspace and confirm new chats use it.
- Category: happy path / settings
- Preconditions: Desktop running; disposable directories for primary + optional reference folders.
- Steps:
  1. Sidebar → Workspaces → Add workspace; name it `Playtest Alpha`.
  2. Set primary working folder to disposable dir A; save after Validate.
  3. Click the workspace row; confirm the Chats list contains only Alpha sessions and the global **New Chat** workspace selector remains unselected.
  4. Use the `+` action next to Alpha; confirm New Chat opens with Alpha preselected; ask `What is your working directory?`
  5. Click another workspace row; confirm the open chat still shows the pinned Alpha workspace.
- Expected: validation catches a missing primary folder before save; row clicks filter chats without changing future-chat defaults; only the row-level new-chat action preselects Alpha; the created chat pins Alpha and its header remains truthful.
- Observe: **All workspaces** restores the unfiltered chat list without changing the New Chat selector.
- Variations: use **Show its chats** and **New chat in this workspace** from the row menu; they match the direct row and `+` actions.

### WS-02 — Credential profile bind and secret non-echo
- Goal: secrets stay out of the renderer and logs; bind/unbind is explicit.
- Category: settings / files
- Preconditions: Desktop; test API key that can be rotated/revoked; disposable home.
- Steps:
  1. Manage credential profiles → New profile for a test provider; enter a secret; save.
  2. Confirm UI shows configured/metadata only — never the raw secret after save.
  3. Bind the profile to a workspace; start a chat that needs the provider.
  4. In the active chat composer, open the credential control; confirm it names the pinned profile, lists other profiles as new-chat choices, and opens Manage credential profiles.
  5. Select another workspace chat filter and reopen the composer control; confirm the active chat still names its original pinned profile.
  6. Edit the profile (should show "Configured — enter a replacement"); cancel without saving; confirm secret fields clear.
- Expected: the active-chat credential control is always present (compact key icon at narrow widths), shows the pinned profile truthfully, and provides direct manager access; no secret appears in UI, clipboard side-effects, or routine logs; missing profile remains named as unavailable and fails closed with relink, not a silent substitute of another profile.
- Observe: keyring vs `secrets.yaml` fallback messaging if keyring is disabled.
- Variations: delete a profile still referenced by a workspace — confirmation + visible relink-required state.

### WS-03 — Missing primary folder / relink
- Goal: deleted or moved folders produce recoverable, named errors.
- Category: recovery / files
- Preconditions: workspace whose primary folder is under a disposable parent you control.
- Steps:
  1. Create workspace pointing at `…/playtest-primary`; start a successful chat.
  2. Quit Desktop; rename/move the primary folder; relaunch.
  3. Try to start a new chat on that workspace; try to resume the old session.
  4. Relink or recreate the folder; confirm recovery.
- Expected: new chat blocked with a clear missing-folder/relink error; historical session remains openable or fails with the same honesty; app does not crash or silently use `$HOME`.
- Observe: optional reference/output missing → warning vs hard-disable (docs: warning for optional).

### WS-04 — Artifact save routes to product outputs
- Goal: Desktop artifact router places saves/exports in the workspace product folders.
- Category: files / persistence
- Preconditions: workspace with distinct document/code/export output folders that exist; a session that can produce a saveable artifact (or use session export / Save a copy).
- Steps:
  1. Produce or obtain an artifact in chat (e.g. ask agent to write a short markdown doc, or export session).
  2. Use **Save a copy** / export / native download paths available in the UI.
  3. Select another workspace chat filter mid-flight; save again from the original session.
- Expected: saves go to the **pinned** session workspace destinations by product type; collision-safe names; unavailable/deleted pin does not silently redirect to another workspace; native download failure shows a warning rather than a false success.
- Observe: router never moves the original generated file — copies only.
- Variations: missing output folder with "Allow explicit creation if missing" — confirm before create.
