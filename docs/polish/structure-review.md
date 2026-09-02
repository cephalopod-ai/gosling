# Structure review

Date: 2026-08-27

The workspace layout matches the documented crate, CLI, Desktop, and text UI
boundaries. No module relocation was applied.

At review time, the remaining source files of at least 2,000 lines included
`agent.rs`, `acp/server.rs`, `session_manager.rs`, `extension_manager.rs`,
provider-format modules, and Desktop `main.ts`. Splitting those files is
intentionally routed to dedicated source-modularization work because it changes
ownership boundaries and creates a substantially larger review surface than a
polish pass. A 2026-09-01 follow-up completed Desktop `main.ts`; the canonical
backlog now tracks the three remaining files in that routed slice.

Generated and vendored assets, including the minified Mermaid runtime, were
excluded from source-structure and comment scans.
