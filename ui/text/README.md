# gosling ACP TUI

Early stage and part of gosling's broader move to ACP

https://github.com/repo-makeover/gosling/issues/6642
https://github.com/repo-makeover/gosling/discussions/7309

## Running

The TUI launches the gosling ACP server by spawning `gosling acp`. Which binary it spawns is resolved by `@repo-makeover/gosling-sdk`:

1. the `GOSLING_BINARY` environment variable, if set, otherwise
2. the platform's prebuilt `@repo-makeover/gosling-binary-*` package (an optional dependency of the workspace `@repo-makeover/gosling-sdk`).

```bash
cd ui/text
pnpm install   # links the workspace @repo-makeover/gosling-sdk and its matching binary packages
pnpm start     # tsx src/tui.tsx — runs against the released binary, no Rust build
```

The TUI uses the workspace `@repo-makeover/gosling-sdk`, so ACP schema changes stay aligned across the desktop app, SDK, and text UI.

### Building gosling from local source

To test local Rust changes, run the dev launcher directly. It builds a debug binary (`cargo build -p gosling-cli` → `target/debug/gosling`) from the workspace root and points the TUI at it via `GOSLING_BINARY`:

```bash
node scripts/dev-start.mjs
```

If your changes touch the ACP schema, rebuild the workspace SDK before running the TUI so the local binary and TypeScript types stay matched.

To run any other prebuilt binary, set `GOSLING_BINARY=/path/to/gosling` and use `pnpm start`.

### Custom server URL

To connect to an already-running server instead of spawning a binary:

```bash
pnpm start -- --server http://localhost:8080
```
