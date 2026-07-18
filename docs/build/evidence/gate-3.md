# Gate 3 evidence — Workspaces foundation

Date: 2026-07-18

## Implemented

- Canonical versioned Workspace, folder, output, binding, profile, validation, and session
  context DTOs in `gosling-sdk-types`.
- Typed workspace/profile custom request and response definitions for the planned ACP surface.
- Gosling workspace module boundary re-exporting the canonical domain contract.
- Real tests for JSON round-trip, editable projection, and renderer-facing profile shape.

## Commands and results

| Command | Result |
|---|---|
| `source bin/activate-hermit && cargo test -p gosling-sdk-types workspace` | pass: 3 passed, 0 failed |
| `source bin/activate-hermit && cargo check -p gosling` | pass |
| `source bin/activate-hermit && cargo fmt --check` | pass |
| `git diff --check` | pass |

Toolchain: Rust/Cargo 1.92.0 through the repository Hermit environment.

## Structure check

New source files are below the 800-line hard limit (`workspace.rs`: 531 formatted lines),
and the new Gosling module boundary is extracted rather than appended to a large existing file.

