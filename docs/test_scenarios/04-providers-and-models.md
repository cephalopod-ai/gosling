# 04 — Providers and Models

Model selection must stay honest: the UI/CLI claim, the request, and the
billable model should agree. Prefer cheap/local models for thrash scenarios.

---

### PM-01 — Configure provider and model
- Goal: operator can select a provider and model and use them immediately.
- Category: happy path / settings
- Preconditions: disposable home; credentials for at least one provider.
- Steps:
  1. `gosling configure` → Configure Providers → pick provider → enter creds → pick model.
  2. `gosling info -v` (or Settings → Models on Desktop) and confirm selection.
  3. Start a session and capture the provider/model identifier from gosling's run metadata, a controlled provider fixture, or provider-side request log. Asking the model which model it is may be recorded only as a non-authoritative observation.
- Expected: selection persists across relaunch; request evidence names the configured provider/model pair; displayed metadata agrees with request evidence; failure to fetch a model list has a bounded, actionable error.
- Observe: Desktop onboarding vs CLI configure stay consistent for the same home.
- Variations: Tetrate/OpenRouter style multi-model routers if credentials exist; else Not executed — environment unavailable.

### PM-02 — Mid-session model or provider switch
- Goal: changing model mid-life applies on the next turn without wedging the session.
- Category: settings / interruption
- Preconditions: access to ≥2 models (same or different providers).
- Steps:
  1. Start a session on model A; get a short reply; note any model indicator in UI/CLI.
  2. Switch via `/model`, Desktop model picker, or configure — per product surface.
  3. Send a follow-up; confirm the new model is indicated and used.
- Expected: switch is honored on the next run; UI never claims model X while clearly running Y; session stays usable; no zombie streaming state.
- Observe: planner model (if shown) vs chat model labels.
- Variations: switch during an active stream (also covered harder in SX-03).

### PM-03 — Bad / expired API key failure clarity
- Goal: auth failures are user-repairable.
- Category: recovery / error clarity
- Preconditions: ability to set a deliberately wrong key for a test provider without destroying the only production key (use disposable home).
- Steps:
  1. Configure provider with `sk-invalid-playtest-key` (or equivalent).
  2. Start session / `gosling run -t "hi"`.
  3. Read the error; fix the key; retry.
- Expected: clear auth/credential error (not infinite retry spin, not empty hang); after fix, success without deleting the whole config; secrets not echoed in full in the error string.
- Variations: provider binary missing (e.g. `claude` CLI not installed) when using a CLI-backed provider — named prerequisite error.

### PM-04 — Planner vs main model split
- Goal: planning-mode provider/model overrides stay scoped.
- Category: settings / boundary
- Preconditions: config supports `GOSLING_PLANNER_PROVIDER` / `GOSLING_PLANNER_MODEL` (or Desktop equivalent); two models available or one + explicit fallback behavior.
- Steps:
  1. Set planner model different from main chat model in config or settings.
  2. Enter planning workflow (`/plan` or documented plan mode).
  3. Complete or cancel plan; send a normal chat turn.
- Expected: plan steps use planner selection (or documented fallback); normal chat returns to main model; misconfigured planner fails with a named error without corrupting main provider settings.
- Observe: cost/usage indicators if shown — which model is attributed.
- Variations: invalid planner model name only — chat should still work.
