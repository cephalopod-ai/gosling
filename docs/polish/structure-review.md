# Structure review

Date: 2026-08-27

The workspace layout matches the documented crate, CLI, Desktop, and text UI
boundaries. No module relocation was applied.

The remaining source files of at least 2,000 lines include `agent.rs`,
`acp/server.rs`, `session_manager.rs`, `extension_manager.rs`, provider-format
modules, and Desktop `main.ts`. Splitting those files is intentionally routed to
dedicated source-modularization work because it changes ownership boundaries
and creates a substantially larger review surface than a polish pass. The
canonical backlog already tracks the highest-priority four-file slice.

Generated and vendored assets, including the minified Mermaid runtime, were
excluded from source-structure and comment scans.
