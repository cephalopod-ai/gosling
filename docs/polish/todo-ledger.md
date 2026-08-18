# TODO ledger

Date: 2026-08-17

The source scan found actionable TODOs and intentional `unimplemented!` test
doubles. They were not silently removed because their resolutions require
feature, platform, or test-fixture decisions.

| Area | Representative location | Disposition |
| --- | --- | --- |
| Extension metadata synchronization | `ui/desktop/src/components/settings/extensions/utils.ts` | Requires a Rust/Desktop contract decision. |
| Process-global data directory | `crates/gosling/src/acp/server.rs` | Requires a broader path-lifecycle repair. |
| Responses API fixture coverage | `crates/gosling-test-support/src/session.rs` | Requires test-fixture support. |
| Platform-specific controller | `crates/gosling-mcp/src/computercontroller/platform/mod.rs` | Intentional unsupported-platform behavior. |
| Provider schema/model follow-ups | provider and orchestrator modules | Requires provider compatibility decisions. |

The existing project backlog remains `docs/TODO.md`. This ledger is an audit
index, not a replacement backlog.
