# Deep Research file compatibility repair

- Date: 2026-08-27
- Task: accept traditional office, text/data, PDF/PostScript, and image files as
  Deep Research Initial Inputs.
- Skill: `repair-defect-nodejs` for the Desktop intake repair, with the same
  bounded repair applied to its Rust ACP ingestion boundary.
- Baseline: `main` at `ed3a862861a81d06d19b3488c5a62075e1be6b77`;
  initial worktree clean.
- Involvement: L2 standard, inferred; no hard-gated external action was needed
  for the source repair.

## Contract baseline

- `AGENTS.md` governs execution and validation.
- ADR-0015 is the active input-library contract. Its renderer path isolation,
  opaque IDs, content validation, 16-item limit, 20 MiB linked-file limit,
  text/image aggregate limits, and resolve-on-demand behavior remain active.
- ADR-0016 remains unchanged: generated Research Library deliverables are a
  separate contract from operator-supplied Initial Inputs.
- Pre-repair disposition: the implementation conformed to ADR-0015's narrow
  PDF/JSON/UTF-8/image set, but that declared set did not cover the requested
  traditional document and image compatibility. The operator explicitly
  authorized the compatibility amendment.

## Changes

- Expanded both Desktop file selectors to DOC/DOCX, XLS/XLSX, PPT/PPTX,
  PDF/PostScript, JSON/JSONL, broad UTF text/source formats, SVG, and common
  raster formats including BMP, TIFF, ICO, TGA, and portable anymaps. The
  picker permits uncommon files while Rust accepts them only if they safely
  decode as text.
- Added content-validated pure-Rust Office extraction for legacy and Open XML
  Microsoft formats. Open XML entry count and expanded bytes are bounded.
- Added UTF-8 BOM and BOM-marked UTF-16 decoding, structured JSON/JSONL
  validation, and PostScript header validation.
- Added bounded image decoding. BMP/TIFF/ICO/TGA/portable anymap inputs
  normalize to PNG before provider submission; existing PNG/JPEG/GIF/WebP
  inputs retain their valid format.
- Added DR-09 and regression tests for picker coverage, office extraction,
  image normalization, structured text, type mismatch rejection, and UTF-16.

## Validation

- `cargo test -p gosling shell_library_formats::tests -- --nocapture`: pass,
  7 tests.
- `cargo test -p gosling`: pass, including 1,760 library unit tests and all
  runnable integration and documentation tests; existing ignored tests remain
  ignored.
- Desktop `pnpm test:run`: pass, 134 files and 1,072 tests.
- Desktop targeted input tests: pass, 2 files and 10 tests.
- Desktop typecheck and i18n validation: pass.
- `cargo clippy --all-targets -- -D warnings` and the post-adjustment
  `cargo clippy -p gosling --all-targets -- -D warnings`: pass.
- `office_oxide` resolves to 0.1.8 under `Cargo.lock` with the declared
  `MIT OR Apache-2.0` license. `cargo-deny` is not installed in the Hermit
  environment, so its optional policy check could not run.
- `cargo fmt --all` and `git diff --check`: pass.

## Record closure and drift

- No pre-existing defect ledger entry or in-code TODO named this compatibility
  gap. Closure is recorded here and in DR-09.
- ADR-0015 and the Default Shell architecture description were updated as the
  operator-authorized contract amendment; ADR-0016 is unaffected.
- Post-repair drift disposition: intentional authorized amendment complete.
  Main-owned path isolation, session/project scoping, resolve-on-demand, and
  all existing item and byte bounds remain intact; structured and binary
  formats add content validation and archive/image expansion bounds.
