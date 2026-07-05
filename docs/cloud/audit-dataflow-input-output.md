# Audit — Dataflow: Input Ingestion & Output Generation (IOP lens)

Lens: `audit-dataflow-input-output` v3.1. Scope: how untrusted bytes enter and
leave gosling — file read/write, archive/extension package handling, path
traversal, format/extension confusion, malformed-input handling, large-response
handling, session import, skills/plugins loading, provider response parsing.
**Authority: audit-only / read-only.** Builds on `00-orientation.md`.

Evidence discipline per `evidence_discipline.md`: every Confirmed cites a line
actually read. Severity is scored from mechanism, independent of confidence.

---

## 1. Surface / boundary inventory

| Surface | Direction | Format | Source trust | Validation point | Sink | Size bound |
|---|---|---|---|---|---|---|
| Large tool response → temp file | out | text | LLM/MCP result (untrusted) | threshold check | `$TMP/gosling_mcp_responses/*.txt` | 200 KB char cap → file |
| Self-update archive extract | in | zip/tar.bz2 | GitHub release (attested) | `enclosed_name`/`validate_entry_path` + sigstore | binary dir | none on ratio (mitigated by attestation) |
| Session import (CLI file) | in | gosling/ClaudeCode/Codex/Pi JSON(L) | file/remote (untrusted) | `detect_format` + serde | `SessionManager::import_session` | **none** |
| Session import (nostr deeplink) | in | encrypted JSON | remote relay (untrusted) | nip44 decrypt + serde | same | **none** |
| Skill load / supporting file | in | md/any | working-dir & home (untrusted repo) | `canonicalize().starts_with(root)` | context window | n/a |
| Plugin install (git) | in | plugin.json + tree | arbitrary git URL (operator) | name/relative-path validation | `~/.../plugins/<name>` | none |
| Foreign-format converters | in | JSONL | untrusted transcript | `filter_map(...ok())` | native Session JSON | none |

Boundary map: findings live at the **converter parse loop** (import_formats),
the **plugin git clone**, and the **whole-file read** on import. The extract and
temp-write boundaries are hardened (non-findings §4).

---

## 2. Findings

### IOP-GSL-001: Foreign-session importers silently drop unparseable JSONL lines and report full success

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Input-Output-Path

Evidence:
- `crates/gosling/src/session/import_formats/claude_code.rs:19-27`
  ```rust
  let lines: Vec<Value> = content.lines()
      .filter(|l| !l.trim().is_empty())
      .filter_map(|l| serde_json::from_str::<Value>(l).ok())   // corrupt lines dropped
      .collect();
  if lines.is_empty() { return Err(anyhow!("Claude Code import: no parseable JSON lines")); }
  ```
- `crates/gosling/src/session/import_formats/codex.rs:32-39` — identical `filter_map(...ok())` + only-empty error.
- `crates/gosling/src/session/import_formats/pi.rs:58-63` — same pattern for body lines.
- `crates/gosling-cli/src/commands/session.rs:317-325` — after convert, prints `Detected format`, then unconditionally `println!("Session imported:")`. No skipped-line count is surfaced.

Observed behavior:
- Any JSONL transcript line that fails to parse (truncation mid-write, corruption, a schema the converter does not recognise) is silently discarded. The import only fails if **zero** lines parse; otherwise it reports success.

Expected boundary:
- A partial parse must be surfaced: either abort, or report how many records/lines were dropped so the operator knows the imported session is not a faithful copy.

Failure mechanism:
- `filter_map(|l| ...ok())` conflates "line absent" with "line invalid". The success path (`Session imported`) is reached regardless of how many messages were lost.

Break-it angle:
- Truncate a Claude Code `.jsonl` mid-line, or flip a byte in the middle: the importer still says "Session imported" with the surviving prefix. A transcript where only the header parses imports as an almost-empty session that claims to be the full conversation.

Impact:
- Operator believes an imported session is complete when messages (including tool calls/results that establish what the agent previously did) are missing. Downstream: a user resumes a session with silently-elided history and makes decisions on a false record.

Operational impact:
- Blast radius: Workflow
- Side-effect class: file (new session persisted)
- Reversibility: reversible (re-import)
- Operator visibility: silent (no drop count)
- Rerun safety: safe

Adjacent failure modes:
- IOP-GSL-002 (untrusted `cwd` from same transcript).
- Same silent-skip pattern across all three converters, so a single fix should be shared.

Recommended mitigation:
- Remediation pattern: completeness accounting. Count total non-blank lines vs parsed lines in each `convert`; thread a `skipped` count out and have `handle_session_import` print `imported N messages (M lines skipped)`, or fail when `skipped > 0` unless `--lenient`.
- Behavior test: transcript with 1 valid + 1 corrupt line imports and reports skipped=1 (or errors).

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: 3 converters + CLI print + tests
- Nominal implementation agent: codex
- Rationale: mechanical change in 3 sibling files plus the CLI surface; narrow validation surface.

Validation:
- Test: corrupt-middle-line fixture → converter returns a non-zero skipped count and the CLI does not print bare "Session imported".

Non-goals:
- Do not redesign the importers or add new formats.

---

### IOP-GSL-002: Imported/shared session `working_dir` and full conversation are trusted verbatim

Severity: Medium
Confidence: Confirmed (mechanism); impact requires session resume
Evidence basis: source-evidenced
Domain: Input-Output-Path

Evidence:
- `crates/gosling/src/session/import_formats/claude_code.rs:29-33` — `cwd` harvested from a transcript line, `.unwrap_or("")`.
- `crates/gosling/src/session/import_formats/pi.rs:194-199` — `working_dir = if cwd.is_empty() { … } else { cwd }`.
- `crates/gosling/src/session/session_manager.rs:1936-1946` — `import_session` calls `create_session(import.working_dir.clone(), …)` and `replace_conversation(import.conversation)`; no validation of `working_dir` and no provenance mark on the imported conversation.

Observed behavior:
- The imported session's operating directory is taken directly from untrusted import content (a file the user was handed, or a nostr-shared session from a relay). The entire conversation (user/assistant/tool messages) is stored verbatim.

Expected boundary:
- An imported session is candidate evidence. `working_dir` should be validated/normalised (and ideally reset to a user-chosen dir on import), and imported conversation content should carry an "imported / untrusted" provenance so later context assembly treats it as data, not the operator's own history.

Failure mechanism:
- No gate between parse and canonical `create_session`. `working_dir` and message bodies flow straight into persisted state.

Break-it angle:
- Craft a shared session whose `cwd` points at a sensitive directory and whose conversation embeds injection text; when the recipient resumes it, the agent starts in that directory with attacker-authored "history" in context.

Impact:
- Untrusted content becomes the agent's operating directory and in-context history — a prompt-injection / mis-scoping vector. (Primary injection analysis belongs to the `security`/`state-transition` lenses; recorded here as the ingestion boundary.)

Operational impact:
- Blast radius: Workflow (Local escalation if agent acts in the injected dir)
- Side-effect class: file / process (on later agent action)
- Reversibility: reversible before resume
- Operator visibility: log-only
- Rerun safety: safe

Adjacent failure modes:
- IOP-GSL-001 (same source), nostr remote fetch (IOP-GSL-004).

Recommended mitigation:
- Normalise/validate `working_dir` on import (reject non-existent or force operator confirmation) and tag imported conversations with provenance consumed by `security/`.
- Behavior test: import with `cwd:"/etc"` does not silently become the session working dir.

Implementation assessment:
- Complexity: workflow_protocol
- Cost: M
- Cost drivers: import path + provenance plumbing + security-lens coordination
- Nominal implementation agent: claude
- Rationale: touches persistence + trust model; cross-lens.

Validation:
- Test: imported `working_dir` is validated; imported messages carry provenance.

Non-goals:
- Do not build the full injection defense here.

---

### IOP-GSL-003: Plugin `git clone` runs the source URL without a `--` end-of-options separator

Severity: Medium
Confidence: Confirmed (missing `--`); Plausible (exploit path)
Evidence basis: source-evidenced
Domain: Input-Output-Path

Evidence:
- `crates/gosling/src/plugins/mod.rs:292-301`
  ```rust
  Command::new("git").arg("clone").arg("--depth").arg("1")
      .arg(source)          // user/metadata-controlled, no `--` before it
      .arg(destination)
  ```
- `crates/gosling/src/plugins/mod.rs:231-232` — the update path re-derives `source` from `metadata.source` (read from the on-disk `INSTALL_METADATA` json) and clones it again.

Observed behavior:
- The plugin source string is passed as a positional to `git clone` with no `--` guard. git parses leading-dash tokens anywhere before the first `--` as options, so a `source` such as `--upload-pack=…` (or an `ext::` transport via `-c protocol.ext.allow=always`) is interpreted as a git option rather than a repo URL.

Expected boundary:
- Untrusted repo strings must be passed after `--` (`git clone --depth 1 -- <source> <dest>`), and/or rejected if they start with `-`.

Failure mechanism:
- Argument injection: a value beginning with `-` becomes a git flag. git's `ext::`/`--upload-pack` machinery can execute a command during clone — a code-execution primitive.

Break-it angle:
- Install a plugin whose "source" is `--upload-pack=touch /tmp/pwned` or an `ext::sh -c …` URL. The clone step, not the plugin content, runs it. On auto-update the tampered `INSTALL_METADATA.source` re-triggers it without a fresh operator prompt.

Impact:
- Local command execution during plugin install/update. Operator-initiated for the first install (self-inflicted), but the update path widens it to a stored-value trigger.

Operational impact:
- Blast radius: Local
- Side-effect class: process
- Reversibility: irreversible (side effects of the executed command)
- Operator visibility: silent
- Rerun safety: unsafe (auto-update re-runs)

Adjacent failure modes:
- Any other `Command::new("git")` construction lacking `--` (grep the crate).

Recommended mitigation:
- Insert `.arg("--")` before `source`; additionally reject `source` beginning with `-`.
- Behavior test: `source = "--upload-pack=…"` is rejected or treated as a repo name, not an option.

Implementation assessment:
- Complexity: local_guardrail
- Cost: XS
- Cost drivers: one line + one test
- Nominal implementation agent: codex
- Rationale: one-line hardening with a clear regression test.

Validation:
- Test: leading-dash source rejected; normal URL still clones.

Non-goals:
- Do not restrict legitimate URL schemes beyond the dash guard.

---

### IOP-GSL-004: Session import reads the whole file / whole decrypted payload with no size cap

Severity: Low
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Input-Output-Path

Evidence:
- `crates/gosling-cli/src/commands/session.rs:306-308` — `fs::read_to_string(&input)` with no length limit.
- `crates/gosling/src/session/nostr_share.rs:226-236` — remote event fetched from a relay and decrypted into a `String`, returned straight to `import_session` with no bound.
- `crates/gosling/src/session/session_manager.rs:1936-1937` — `convert_to_gosling_session_json(json)` then `serde_json::from_str` over the entire payload.

Observed behavior:
- The full import payload is materialised in memory before parsing; a relay-served nostr session is unbounded remote input.

Expected boundary:
- Cap import size before materialisation; caps on remote-fetched content especially.

Failure mechanism:
- Whole-file/whole-response read on caller-supplied input with no cap (IOP-013). serde_json's default recursion limit bounds nesting depth, but not total size.

Break-it angle:
- A multi-GB import file or a large relay event forces a large allocation (self-DoS).

Impact:
- Memory pressure / OOM on the operator's machine. Local, recoverable.

Operational impact:
- Blast radius: Local
- Side-effect class: none (pre-persist)
- Reversibility: reversible
- Operator visibility: UI-visible (crash/slow)
- Rerun safety: safe

Adjacent failure modes:
- IOP-GSL-002 (same untrusted payload).

Recommended mitigation:
- Cap import bytes (e.g. read with a `take(limit)` / stat-then-reject); tighter cap for the nostr path.
- Behavior test: oversize import rejected before parse.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: two ingest sites + tests
- Nominal implementation agent: codex

Validation:
- Test: import over the cap errors without allocating the whole file.

Non-goals:
- Do not add streaming JSON parsing.

---

## 3. Break-it checklist results

- `.json` that is really an archive / archive with `../` entries → **self-update extract is guarded** (§4); no other untrusted archive-extract path found in-crate.
- Zip bomb / 10^6-entry archive → self-update archive is sigstore-attested before extract (`update.rs:279`), so a hostile archive can't reach the extractor; **held**.
- Formula-leading export cells (`=`,`+`,`-`,`@`) → **no CSV/XLSX export writer found** in the audited crates; not applicable (record as not-reviewed for UI export, §5).
- Filename with separators/NUL for output path → temp-response filename is timestamp-derived, not caller-derived (`large_response_handler.rs:83-86`); **held**.
- Truncate input mid-record → importer reports success (**IOP-GSL-001, Confirmed**).
- Same payload via CLI vs UI → only the CLI import path (`session.rs`) was traced; UI/desktop import parity **not reviewed** (§5).
- Provider/OCR hallucinated-but-structured output becomes canonical → imported conversation trusted verbatim (**IOP-GSL-002**).

---

## 4. Non-findings (checked and held)

- **Large tool-response temp write** — `crates/gosling/src/agents/large_response_handler.rs:77-124`: destination dir forced to `0o700` (`restrict_to_owner`), refuses if the dir is a symlink (`reject_symlink`, l.105-113), and the file is opened `O_CREAT` `0o600` (`open_owner_only`, l.116-124). Filename is timestamp-derived, not caller-controlled. World-readable leakage and symlink-redirect are both closed. Held.
- **Self-update archive extraction** — `crates/gosling-cli/src/commands/update.rs`: zip uses `entry.enclosed_name()` and bails on unsafe paths (l.343-346); tar uses `validate_entry_path` rejecting absolute + `..` (l.365-374) and validates `link_name()` for both symlink and hardlink escape (l.399-404). Provenance is verified via sigstore attestation and **fails closed** before extract (`verify_provenance` l.185-224, called l.279; test l.878-882). Zip-slip, tar-slip, symlink escape, and unattested-archive all closed. Held.
- **Skill supporting-file load** — `crates/gosling/src/skills/client.rs:143-160`: the requested file must be in the pre-discovered `supporting_files` list AND `canonicalize().starts_with(canonical_skill_dir)`; symlinks resolve outside and are refused with "resolves outside the skill directory". `/path` traversal via the `skill/subpath` syntax is contained. Held.
- **Plugin install path construction** — `open_plugins.rs:203-244` validates the plugin name (lowercase/digit/`-`/`.`, no `..`, no `--`, must start/end alnum) before `install_root.join(name)`; component paths must start `./` and reject absolute/`..` (`validate_relative_plugin_path` l.388-409). `copy_dir_all` (`plugins/mod.rs:372-386`) copies only `is_dir`/`is_file` entries, so symlinks in a hostile checkout are silently skipped rather than followed. Held.
- **Format detection depth** — `import_formats/mod.rs:90-144` sniffs the first parsed line's `type`/marker fields; serde_json's built-in recursion limit bounds nesting so a deeply-nested import cannot blow the stack. Held (size is still uncapped → IOP-GSL-004).

---

## 5. Validation limits (NOT reviewed / capped)

- **UI/desktop import parity** — only the CLI import (`gosling-cli/.../session.rs`) was traced. Whether `ui/desktop` imports through the same `convert_to_gosling_session_json` validator (IOP-015) is **not verified**; IOP-GSL-001/004 confidence for the UI path is capped.
- **Provider streaming/response parsing** — `crates/gosling-providers/src/formats/*` and `crates/gosling/src/providers/formats/*` (bedrock/vertex/google/etc.) were **not read**; malformed provider-response handling (IOP-006/008) for those is unreviewed.
- **Dictation audio input** — `dictation/providers.rs` not examined; audio ingest bounds unknown.
- **Config parsing** — `config/base.rs` YAML/JSON load bounds not traced.
- **Gemini plugin format** (`plugins/formats/gemini.rs`) and **hooks loading/execution** (`hooks/`) not examined for this lens.
- **CSV/XLSX export** — no export writer found in audited crates; any spreadsheet export in `ui/` is unreviewed (IOP-007 not exercised).
- No tests were run (read-only). All findings are `source-evidenced` static analysis; runtime OOM (IOP-GSL-004) is reasoned, not reproduced.

## 6. Findings table

| ID | Title | Severity | Confidence |
|---|---|---|---|
| IOP-GSL-001 | Importers silently drop bad JSONL lines, report success | Medium | Confirmed |
| IOP-GSL-002 | Imported `working_dir`/conversation trusted verbatim | Medium | Confirmed (mechanism) |
| IOP-GSL-003 | `git clone` source without `--` separator (arg injection) | Medium | Confirmed / Plausible |
| IOP-GSL-004 | Unbounded whole-file/remote read on import | Low | Confirmed |
