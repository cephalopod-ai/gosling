# ADR-0015: Shell project/session input library

Date: 2026-08-18
Status: implemented and locally validated; exact-revision CI and packaged lifecycle evidence pending

## Context

The generic shell needs reusable reference material that is not necessarily inside its selected
working directory: an operator-selected document, text/data file, or image. Giving the renderer a
path or a generic filesystem bridge would violate ADR-0011 and ADR-0014. Treating these inputs as
Outputs would also violate ADR-0013 because Outputs are descriptive metadata and grant no read
authority.

## Decision

Gosling owns a separate durable input library in the session database. An item has an opaque ID,
name, kind, MIME type, byte size, status, and either `session` or `project` scope. Session scope is
visible only to one session. Project scope is keyed by project ID, then workspace ID, then a digest
of the canonical working directory, so sessions in the same project share it without exposing that
key.

The Electron main process owns native file selection. A renderer requests `linkFile` with a user
gesture, scope, generation, and active session ID; it cannot submit or receive a path. Main sends the
selected path directly to the authenticated Rust method. Pasted text and images cross only narrow,
bounded typed operations. Safe list responses never contain a source path or stored payload.

Prompt submission carries at most 16 opaque library IDs. Main asks Rust to resolve only those IDs
for the active session. Stored or linked images become standard ACP image blocks. Text files,
office documents, and pasted text become labeled ACP text blocks. Linked-file suffixes select an
expected supported type, but Rust verifies image/PDF/PostScript signatures, parses JSON and
newline-delimited JSON, validates Office containers, and requires valid non-binary UTF-8 or
BOM-marked UTF-16 text both when linking and when resolving. A replaced file whose content no
longer matches its recorded type cannot be attached.

PDF text extraction occurs in Rust through `lopdf`. Microsoft Word, Excel, and PowerPoint files in
legacy and Open XML forms are parsed through `office_oxide`; Open XML archives also have entry-count
and expanded-byte ceilings. BMP, TIFF, ICO, TGA, and portable anymap inputs are decoded with
dimension/allocation bounds and normalized to PNG for provider compatibility. Files with uncommon
suffixes are accepted only when their bytes safely decode as text. Input bytes are bounded before
parsing; PDF object and page counts are checked before text extraction; all extracted text stops at
the resolved-text budget. Linked files remain in place and resolve on demand; a missing file is
visible as `missing` and cannot be attached.

Bounds are enforced independently at Desktop and Rust boundaries: 64 items per scope, 256 KiB per
pasted text item, 5 MiB per image, 20 MiB per linked file, 512 KiB total selected source text,
10 MiB total selected images, and 16 selected items. Aggregate accounting measures source payload
bytes rather than the labels Gosling adds to resolved prompt blocks. `session.library.read` gates
listing and resolution;
`session.library.write` gates add, link, and remove operations and requires read capability.

## Consequences

- A shell can use reference material outside its tool working directory without gaining arbitrary
  filesystem authority.
- Project references persist across sessions; session references do not leak to sibling sessions.
- Linked content reflects the current file and becomes unavailable if it moves or disappears.
- Pasted payloads are copied into the private session database and should be treated with the same
  sensitivity as conversation history.
- Outputs and the input library remain distinct contracts: listing an Output still grants no read
  access.

## Rejected alternatives

| Alternative | Reason rejected |
| --- | --- |
| Expose a file path or generic `readFile` preload API | Gives a compromised renderer ambient filesystem authority. |
| Reuse the Outputs inventory | Converts metadata discovery into implicit file-read authorization. |
| Copy every selected file into the project | Mutates operator files and creates unclear ownership and cleanup rules. |
| Let the renderer extract PDFs | Sends source bytes and parsing authority across the least-trusted boundary. |

## Dependency record

`lopdf` 0.42 is added to the `gosling` crate with default features disabled for bounded local PDF
text extraction. `office_oxide` 0.1.8 supplies pure-Rust DOC/DOCX, XLS/XLSX, and PPT/PPTX parsing.
The existing `image` dependency enables BMP, TIFF, ICO, TGA, and portable anymap decoding in
addition to PNG, JPEG, GIF, and WebP. `lopdf` and `image` already existed elsewhere in the
workspace.
