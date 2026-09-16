# Session evidence

`session/` contains dated, task-scoped execution records. The repository's
established convention is a flat `YYYY-MM-DD-slug.md` filename rather than
monthly buckets. These files are retained evidence: do not treat them as the
active backlog and do not rewrite historical results to match current state.

Current work belongs in [`docs/TODO.md`](../TODO.md), with the active-only view
in [`docs/polish/active-todo-ledger.md`](../polish/active-todo-ledger.md).

## Tracking a new log

`.gitignore` ignores `docs/logs/session/*` and re-admits each log by name, so a
new log needs its own negation line added alongside the existing ones:

```
!docs/logs/session/YYYY-MM-DD-slug.md
```

Without that line the file stays untracked and never reaches the commit, even
though `AGENTS.md` requires the log — the evidence silently does not land. Logs
committed before the ignore rule arrived in `18658bb73` remain tracked and need
no entry.
