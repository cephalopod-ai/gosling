# 10 — Settings, Config, and Navigation

Desktop settings and sidebar navigation must persist and stay consistent with
CLI-visible config. If Desktop is unavailable, run the config.yaml cards via
CLI and mark pure UI cards Not executed.

---

### ST-01 — Desktop settings persist across relaunch
- Goal: App/Chat/Keyboard settings survive quit.
- Category: settings / persistence
- Preconditions: Desktop; disposable home.
- Steps:
  1. Open Settings → App: change theme and a navigation option (sidebar style/position/mode if safe).
  2. Chat: change mode or a non-destructive toggle; Keyboard: change a non-global shortcut if comfortable.
  3. Quit Desktop fully; relaunch; re-open Settings.
- Expected: values persist; global shortcuts that claim "immediate" work; app shortcuts that require restart say so and then work after restart.
- Observe: Settings vs `config.yaml` agreement for keys that are dual-managed.
- Variations: Reset sidebar items to defaults; confirm Workspaces section expand state independence.

### ST-02 — Sidebar navigation stress
- Goal: all major destinations are reachable and survive deep links/reloads.
- Category: navigation / empty state
- Preconditions: Desktop running.
- Steps:
  1. Visit Home, Chat, Extensions, Skills (if present), Settings tabs (App, Chat, Providers, etc.), Session History, Workspaces.
  2. Toggle sidebar open/closed; switch style list/tile; try overlay mode.
  3. Open a second window; navigate independently; close one window mid-chat in the other.
- Expected: no blank panels, infinite spinners, or unhandled exception overlays; unknown internal routes fail soft; multi-window does not corrupt the other's session state.
- Observe: keyboard shortcuts from docs (`Cmd/Ctrl+N`, `+T`, `+,`, sidebar toggle).
- Variations: resize below 700px width (list style icon collapse per docs).

### ST-03 — Invalid config.yaml values
- Goal: bad typed settings fail closed without destroying the home.
- Category: invalid input / recovery
- Preconditions: backup of disposable config; CLI.
- Steps:
  1. Set `GOSLING_MAX_TURNS: "plenty"` (string); try `gosling run -t hi`.
  2. Set `GOSLING_AUTO_COMPACT_THRESHOLD: 5` (out of 0–1 range if enforced); retry.
  3. Set `GOSLING_MODE: "yolo"`; retry.
  4. Restore known-good config; confirm recovery.
- Expected: each bad value is rejected or ignored with a warning — never a boot loop that requires deleting the home; good restore works immediately.
- Observe: Desktop still launches and shows a config error banner if CLI fails.
