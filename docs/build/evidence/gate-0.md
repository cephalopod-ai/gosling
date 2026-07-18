# Gate 0 evidence — Workspaces orientation

Date: 2026-07-18
Baseline: `9b9571febf06f7fc6dfddea32267b5c0d325b369`

## Instructions and architecture read

- Read root `AGENTS.md`, `CLAUDE.md`, `CONTRIBUTING.md`, `SECURITY.md`, and `README.md`.
- Read `ui/desktop/README.md`; no nested Desktop or crate instruction file applies.
- Searched every symbol named in the request, including sidebar/layout/context,
  directory chooser, settings IPC, session creation, working-directory, config,
  secret storage, keyring, and session database surfaces.
- Inspected the actual Gosling fork rather than relying on upstream Goose paths.

## Current architecture

- Desktop is a React renderer over ACP. `AppLayout` owns the persistent navigation,
  chat-session container, and artifact workbench; `NavigationPanel` renders the
  current session list.
- `ConfigContext` manages ACP-backed provider and extension configuration, while
  `ModelAndProviderContext` supplies global defaults and per-session model/provider
  switching.
- New Desktop sessions currently pass a working directory and enabled extensions to
  the ACP new-session request. No workspace identifier exists.
- `SessionManager` owns SQLite `sessions.db`, whose schema version is 21 and whose
  session model has no workspace or credential-profile fields.
- `Config` owns configuration and secret persistence. It uses the Gosling OS keyring
  by default and a permission-hardened atomic file fallback when needed.
- Provider instances are constructed per session, but their constructors read
  credentials from global `Config`; resume may silently fall back to the globally
  configured provider.
- Electron already supplies directory selection, reveal/open, ensure-directory,
  settings IPC, typed renderer events, and canonical file-access helpers.
- Rust custom request DTOs in `gosling-sdk-types` feed the generated Gosling UI SDK;
  Desktop must consume those or local types and must not import generated OpenAPI code.

## Intended change areas

- `crates/gosling-sdk-types`: canonical workspace/credential DTOs and custom requests.
- `crates/gosling`: workspace domain/store/service, ACP handlers, secure profile scope,
  session schema/model/resume changes, non-secret agent context, and tests.
- `ui/sdk`: generated SDK output through the existing generator.
- `ui/desktop`: Workspace context, sidebar/editor/header/session filtering, folder IPC,
  multi-window refresh, types, and tests.
- `docs`: intent, contracts, ADRs, traceability, evidence, and handoff.

## Gate conclusion

The feature is feasible without changing CLI behavior or introducing a second secret
store. Gate 0 identified two architecture findings that constrain the implementation:
extract workspace persistence from the existing large session manager, and make
credential selection explicit at provider-construction time. See
`../audits/gate-0-audit.md`.

