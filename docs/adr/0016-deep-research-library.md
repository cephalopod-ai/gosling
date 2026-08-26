# ADR-0016: Durable Deep Research library

Date: 2026-08-26
Status: implemented and locally validated
Related: ADR-0011, ADR-0013, ADR-0015

## Context

Deep Research reports and tutorials were ordinary session artifacts. Their metadata survived in the
session inventory, but the files could remain scattered across workspace output folders and were not
browsable as a reusable body of prior research. Reusing the input Library would blur the distinction
between operator-supplied evidence and model-produced documents. Expanding the Outputs inventory into
a directory scan would also violate ADR-0013's session-provenance boundary.

## Decision

The Desktop owns a separate Research Library directory. Its effective default is
`Documents/Gosling Research Library`; the operator may change it only through a native directory
chooser. The persisted renderer setting is readable but cannot be written through generic setting
IPC, so a compromised renderer cannot turn the library preference into an arbitrary directory grant.
The Electron main process creates and grants the effective root when the library is used.

The right artifact pane exposes a third, boxed-count `Library` tab. Main performs a bounded metadata
scan of the configured root: at most 500 document-like files, six directory levels, using the same
configured display extensions as Outputs. Hidden entries and symbolic links are excluded. Selection
then uses the existing artifact preview/open guard. This is a distinct browse contract; scanned files
are never inserted into the session Outputs inventory or the input Library.

Every Deep Research session receives the library root as an additional working directory and a
session system instruction that routes user-facing reports, tutorials, appendices, and exported data
summaries there. The instruction permits relevant prior reports as optional secondary context, labels
them potentially stale, and requires verification of load-bearing claims against current primary
evidence. It explicitly forbids treating model agreement or a prior report as independent
corroboration. Scratch files and caches remain outside the library.

## Consequences

- Research deliverables live in an operator-visible filesystem location independent of session
  retention and remain browsable from Gosling.
- Future research can consult relevant prior work without silently elevating it to source-of-truth
  status or auto-injecting the entire library into model context.
- The library is not a content-addressed archive. Operators can edit, move, or delete its files, and
  the next bounded listing reflects that filesystem state.
- The agent instruction is the destination-routing contract; Gosling does not copy arbitrary session
  artifacts after the fact or rewrite files the user produced elsewhere.

## Rejected alternatives

| Alternative | Reason rejected |
| --- | --- |
| Scan workspace Outputs globally | Loses session provenance and turns unrelated workspace files into research inventory. |
| Store generated reports in the input Library database | Conflates supplied evidence with generated analysis and duplicates filesystem documents. |
| Let renderer code submit a library path directly | Creates a generic persistent directory-grant primitive. |
| Auto-attach every prior report to every research prompt | Causes unbounded context growth and turns stale generated work into implicit authority. |
