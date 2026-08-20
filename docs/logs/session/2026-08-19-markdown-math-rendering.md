# 2026-08-19 — Markdown math rendering repair

## Task

Repair Desktop assistant-message formatting after an operator screenshot showed ChatGPT-style
LaTeX display delimiters rendered as literal bracket lines, raw equation source left visible, and
long prose reaching the message boundary without resilient containment.

## Evidence and cause

- `MarkdownContent` already used `react-markdown`, `remark-math`, and KaTeX, so a renderer or
  framework replacement was unnecessary.
- `remark-math` was deliberately configured with `singleDollarTextMath: false`, which preserved
  shell variables and ordinary dollar text but recognized only dollar-delimited math.
- Markdown consumed `\[` / `\]` and `\(` / `\)` escapes before KaTeX could see them, reproducing
  the operator's literal `[` / `]` blocks and raw commands.
- The message surface had width bounds, but its wrapping helper used the weaker `break-word`
  behavior and intermediate message wrappers did not all declare a zero minimum width.

## Repair

- Normalize ChatGPT-style display and inline LaTeX delimiters to the already-supported math syntax
  before Markdown parsing.
- Leave fenced code and inline code unchanged, and retain the existing single-dollar safeguard for
  shell commands and currency-like text.
- Strengthen message and paragraph containment with zero minimum widths, `overflow-wrap: anywhere`,
  and a bounded horizontally scrollable KaTeX display surface.
- Add screenshot-shaped component regressions for two display equations, inline math, and protected
  code content.

No dependency, message schema, persistence format, IPC surface, link policy, HTML policy, or shell
contract changed. No prior repository issue, TODO, or defect-ledger entry named this defect; this
session log is its source and closure record.

## Validation

- Focused `MarkdownContent` suite: 28/28 passed.
- Chromium preview at 760 × 900: two display equations and one inline equation rendered with no
  KaTeX error; page and message `scrollWidth` stayed within `clientWidth`; long prose wrapped.
- `cargo fmt --all -- --check`: passed.
- Desktop typecheck, ESLint, and i18n checks: passed.
- Full Desktop Vitest: 118 files passed, 992 tests passed.
- `git diff --check`: passed.

## Residual limits

- Single-dollar inline math remains intentionally literal because treating every `$...$` span as
  math would regress shell commands and ordinary currency text. Inline math is supported through
  `\(...\)` and display math through `\[...\]` or `$$...$$`.
- The Chromium proof exercised the real component and stylesheet in a temporary Vite preview, not a
  provider-backed end-to-end conversation. The supplied screenshot remains the before evidence.
