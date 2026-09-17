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
key. Explicitly copying or branching a session clones its session-scoped input entries with new
opaque IDs. Linked files keep pointers to their original paths; pasted text and images retain
their stored payloads. Each session owns its entries independently, so removing an input or deleting
the source session does not remove a branch's inputs. Project inputs continue to use the shared key.

The Electron main process owns native file selection. A renderer requests `linkFile` with a user
gesture, scope, generation, and active session ID; it cannot submit or receive a path. Main sends the
selected path directly to the authenticated Rust method. Pasted text and images cross only narrow,
bounded typed operations. Safe list responses never contain a source path or stored payload.

Inline prompt submission carries at most 16 opaque library IDs. Main asks Rust to resolve only those IDs
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
parsing; PDF object and page counts are checked before text extraction. PDF extraction that exceeds
its budget fails rather than returning partial text. Linked files remain in place and resolve on demand; a missing file is
visible as `missing` and cannot be attached.

Bounds are enforced independently at Desktop and Rust boundaries: 64 items per scope, 256 KiB per
pasted text item, 5 MiB per image, 20 MiB per linked file, 512 KiB total selected source text,
10 MiB total inline images, and 16 inline items. Aggregate accounting measures source payload
bytes rather than the labels Gosling adds to resolved prompt blocks. `session.library.read` gates
listing and resolution;
`session.library.write` gates add, link, and remove operations and requires read capability.

### Complete source compilation (2026-09-16)

The full Desktop Inputs pane supports selecting all saved inputs and **Compile all inputs**.
Compilation uses a separate typed ACP method with explicit opaque IDs, at most 128 across the
two visible scopes. It resolves only items accessible to the active session. It builds all sources
before atomically publishing a new `compiled-inputs-<uuid>.md` in the session's working directory,
with private temporary-file permissions and no overwrite of existing files. The file is registered
in Outputs; registration does not grant access to any other path. No input-library row is changed.
Compilation participates in the session operation gate and is denied while planning is open.

The document includes a source index, stable per-compilation source labels, input IDs, UTF-8
byte counts, SHA-256 content hashes, complete pasted text and existing citations. Linked documents
contain extracted text rather than their original layout; images are retained as data URIs and
require separate image inspection. Extraction failures, missing sources, unsupported/scanned
documents without extractable text, and a compilation exceeding 32 MiB fail explicitly without
publishing partial content. Existing per-linked-file and parser-complexity limits still apply.

Desktop preflights known pasted-text/image byte totals. Selections above the inline count or byte
limit use compilation instead; linked-file extraction can also trigger that fallback. The prompt
receives a bounded manifest pointing to the file, not the complete source payload. It tells the
agent to read every indexed source in sections, retain citations, distinguish conflicts/gaps, and
report unavailable file-reading tools. This supplies material for the user's requested synthesis;
it does not prove that an agent read everything or that a report is complete. Input-preparation
errors preserve the selection and original local message and offer retry with current inputs
instead of model-switch recovery. The explicit compilation button includes unchecked inputs too.

This method is implemented in the full Desktop. Provisioned shell consumers retain their existing
inline workflow until their declared frontend integration explicitly adopts compilation.

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
