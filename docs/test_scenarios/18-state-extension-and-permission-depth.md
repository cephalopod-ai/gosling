# 18 — State, Extension, and Permission Depth

These cross-cutting cards close integrity gaps around workspace identity,
portable sessions, release migration, MCP configuration, plugin corruption,
and approval ownership.

---

### SI-01 — Duplicate workspace identity and rename
- Goal: workspace identity is stable even when names collide or change.
- Category: settings / persistence
- Preconditions: Desktop and two distinct fixture folder trees.
- Steps: create workspace `Alpha`; attempt another exact and case-variant name; rename Alpha while one of its chats is open; reuse the old name for a new workspace; relaunch.
- Expected: duplicate policy is explicit; IDs remain distinct; pinned chat follows original ID through rename; reusing a label never steals old sessions or credentials; final sidebar/filter membership is deterministic.
- Observe: leading/trailing whitespace, unicode normalization, and 200-character name.

### SI-02 — Delete workspace with pinned sessions
- Goal: workspace deletion cannot delete or silently repoint session history.
- Category: delete-undo / persistence
- Preconditions: workspace with credential profile, output folders, completed session, active session, and exported baseline.
- Steps: initiate deletion and cancel; confirm and delete while one session is open elsewhere; inspect sessions and files; recreate a workspace with same name/path; relaunch.
- Expected: cancel changes nothing; confirmation states exact effects; sessions remain recoverable with deleted-pin state or are removed only if explicitly promised; files and credential profile follow documented ownership; recreated workspace gets a new identity.
- Observe: new chat behavior from a stale window after deletion.

### SI-03 — Symlinked workspace and reference-folder boundaries
- Goal: workspace validation and artifact routing handle symlinks without path confusion.
- Category: files / boundary
- Preconditions: primary and reference directories with internal symlink, broken symlink, loop, and symlink escaping the disposable parent.
- Steps: validate each as primary/reference/output; start a chat in accepted workspace; request reads through links; save an artifact; move link target and retry.
- Expected: accepted path resolves consistently and is shown honestly; loops/broken links fail boundedly; escape access follows permission policy and is never mislabeled as inside; output lands at the displayed resolved/declared destination.
- Observe: canonical versus user-entered path persistence across relaunch.

### SI-04 — Export format matrix, overwrite, and permissions
- Goal: Markdown, JSON, and YAML exports are complete, parseable, and safely written.
- Category: files / persistence
- Preconditions: session with unicode, multiline content, tool success/failure, attachment metadata, and fake secret marker; existing destination file with known hash/mode.
- Steps: export each format to stdout and file; parse structured formats; target an existing file, directory, read-only parent, and symlink; export by name and ID.
- Expected: formats represent the same ordered turns; stdout contains artifact only; fake secret is absent/redacted; overwrite behavior is explicit and atomic; failed writes preserve old hash; created file permissions are not broader than documented.
- Observe: timestamp/timezone and binary attachment representation.

### SI-05 — Imported-session working-directory trust boundary
- Goal: imported transcripts cannot silently choose an unsafe cwd.
- Category: files / authorization
- Preconditions: valid export declaring a path outside the disposable tree plus controlled safe directory; malformed and foreign JSONL fixtures.
- Steps: import without `--working-dir`; import with safe `--working-dir`; open and request cwd/tool action; move safe directory; import same file twice.
- Expected: imported metadata never grants implicit tool access to an untrusted path; explicit working dir is visible and used; missing dir blocks/relinks; duplicate behavior is non-destructive; foreign format preserves message order without executing embedded content.
- Observe: absolute paths and provider secrets from source are sanitized appropriately.

### SI-06 — Upgrade migration from a prior supported release
- Goal: a supported older config/session store upgrades atomically and remains usable.
- Category: persistence / recovery
- Preconditions: disposable root created by the oldest currently supported release with provider placeholder, sessions, extension, and settings; complete backup/hash; current build.
- Steps: open with current CLI and Desktop separately from cloned backups; inspect migration output; chat/export/list; interrupt one migration on another clone; retry; reopen with old release only if rollback is documented safe.
- Expected: migration is automatic or clearly prompted; entity counts/content match baseline; backup/recovery exists before destructive schema change; interruption is retryable; secrets are not migrated into plaintext; unsupported downgrade is blocked clearly.
- Observe: unknown keys preserved versus intentionally removed with release-note evidence.

### SI-07 — Duplicate MCP install and command environment
- Goal: repeated `mcp install` cannot merge commands, cwd, or env ambiguously.
- Category: settings / files
- Preconditions: two local MCP fixtures with same proposed name but distinct tool markers; cwd containing spaces; harmless env marker.
- Steps: install first with `--cmd`, `--cwd`, and `--env`; reinstall same name with changed values; list/relaunch/use tool; remove once and twice; try empty/whitespace and unicode names.
- Expected: duplicate install rejects or replaces atomically with explicit result; exactly one command/tool marker is active; cwd/env arrive byte-for-byte; first remove clears it and second is a named miss; config remains parseable.
- Observe: command quoting across shell metacharacters without executing unintended fragments.

### SI-08 — MCP secret storage and redaction
- Goal: `--secret` values stay outside ordinary config and every user-facing output.
- Category: authorization / files
- Preconditions: fixture echoes only whether a secret env key exists; unique fake secret; disposable root with keychain enabled and separately unavailable if feasible.
- Steps: install with `--secret KEY=VALUE` and with `--secret KEY` sourced from env; inspect config/list/info/diagnostics/logs; use tool; remove extension; relaunch with source env absent.
- Expected: fixture receives secret when authorized; raw value appears nowhere in config/output/logs; missing secret is named without fallback to another credential; removal follows documented secret cleanup; keychain fallback is explicit and no less restrictive.
- Observe: process listings and crash reports for command/env leakage.

### SI-09 — Malformed skill/plugin and interrupted update
- Goal: extension content failures cannot poison the whole skill/plugin catalog.
- Category: invalid input / recovery
- Preconditions: local git plugin fixtures for valid, missing manifest/skill, malformed frontmatter, duplicate skill names, and an update that can be interrupted.
- Steps: install each URL; list and invoke unaffected valid skill; update valid plugin to new sentinel; interrupt another update; retry; remove/quarantine fixture using documented path.
- Expected: invalid plugin is rejected or isolated with path/reason; valid catalog remains usable; duplicate precedence is explicit; update changes sentinel atomically; interruption leaves old version or recoverable staging, never mixed files.
- Observe: URL credentials and git stderr redaction; auto-update failure at session start.

### SI-10 — Approval scope, persistence, and competing clients
- Goal: an approval decision applies only to its documented tool/session/scope and owner.
- Category: authorization / concurrency
- Preconditions: CLI and Desktop attached to separate sessions; fixture tools with same display name but distinct server IDs; Ask, Always Allow, and Never Allow available.
- Steps: approve once in A; invoke same tool in B; set Always Allow for fixture 1 and invoke fixture 2; relaunch; create simultaneous approvals in two clients; deny one and approve the other.
- Expected: one-time approval does not cross session; server/tool identity prevents name collision; persistence matches the selected scope and survives only when promised; each client resolves only its own request; deny cannot be overridden by the competing approval.
- Observe: approval UI includes arguments, cwd, extension identity, and whether decision will persist.
