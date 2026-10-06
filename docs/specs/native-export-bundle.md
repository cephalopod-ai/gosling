# Native session acquisition bundle

Status: implementation pending validation, 2026-10-06. This is a bounded
acquisition lane for Muninn and other explicit local exporters. Ordinary
native JSON import/export retains its 16 MiB round-trip limit.

`gosling session export --session-id ID --format json-pages --output DIRECTORY`
exports one consistent native snapshot, including internal messages and
`plan_history_v1`, through owner-only UTF-8 fragments. Default secret redaction
applies before any file or hash is produced. Nostr sharing is unavailable in
this lane. The total snapshot bound is 256 MiB, each part at most 1 MiB, and
the manifest permits at most 512 parts. These bounds do not permit silent
truncation or bypass ordinary session import limits.

The directory contains `intent.json`, `part-000000.jsonfrag` and subsequent
parts, then `manifest.json` published last. Manifest schema
`gosling.session-export-bundle.v1` declares session identity, full content
SHA-256, total bytes, redaction/completion flags and ordered part indices,
filenames, offsets, lengths and SHA-256 digests. Concatenating part bytes in
order reconstructs the exact native JSON snapshot. Only the final manifest
admits a bundle as complete.

Rerunning at the same directory verifies already written files and resumes
missing parts without overwriting any existing bytes. A changed snapshot,
corrupt file, symlink or unrelated file requires a fresh output directory.
Files use owner-only permissions and durable no-clobber atomic publication.
Importers must validate every part and the full digest before publishing any
memory; plans remain historical evidence and do not transfer approval authority.

Validation command: `cargo test -p gosling --test session_export_bundle` plus
existing native transfer/plan-history tests, CLI parsing, fmt and clippy.
Rust build/test execution awaits explicit operator authorization under AGENTS.md.
