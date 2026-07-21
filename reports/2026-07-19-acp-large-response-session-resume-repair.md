# ACP large-response session-resume repair — 2026-07-19

Skill: private catalog `repair-defect-priority`  
Agent/model: Codex, GPT-5  
Repository: `cephalopod-ai/gosling`  
Branch/baseline: `main` at `baeb74de1bd05c11629dc0c240efd92502a39f12`  
Input finding: user reproduction showing historical sessions failing with
`ACP WebSocket receive buffer exceeded its limit`

## Selected patch batch

- P0 reliability regression: valid large ACP responses closed the shared desktop connection and
  prevented historical sessions from loading.
- Eligible domains: reliability and performance.
- Files: `ui/desktop/src/acp/createWebSocketStream.ts` and its adjacent test.
- No higher-priority item was deferred. Feature work and unrelated lint debt were excluded.

## Root cause and patch

Commit `1f9867b66` bounded the desktop adapter to 1,000,000 characters per ACP message and
8,000,000 buffered characters. The user's configured Featherless inventory contains 22,329
models, making `_gosling/unstable/providers/list` a valid 3,157,158-character response. That
response crossed the single-message limit, closed the shared connection with WebSocket code 1009,
and caused the concurrent `session/load` request to surface the transport error.

Direct compacted replay proved the historical sessions were not oversized: the affected PVE2
session replayed 110 frames / 357,110 characters, and the Mycelium session replayed 103 frames /
540,286 characters. Their largest replay frame was 67,304 characters.

The patch aligns the single-message limit with the existing 8,000,000-character aggregate receive
budget. It remains bounded, retains the 1,024-message queue limit, and distinguishes a single
oversized message from aggregate receive-buffer exhaustion. A regression test accepts a
representative 3.2-million-character provider response while the existing test continues to reject
a response one character above the hard limit.

## Verification

- `pnpm test:run src/acp/createWebSocketStream.test.ts`: passed, 3 tests.
- `pnpm test:run src/acp`: passed, 14 files / 119 tests.
- `pnpm run typecheck`: passed.
- Targeted ESLint on the two touched files: passed with zero warnings.
- Targeted Prettier check: passed.
- `cargo fmt --check`: passed.
- `pnpm run package`: passed; production Electron bundle created.
- Packaged bundle `codesign --verify --deep --strict`: passed after signing the completed bundle
  with the repository entitlements.
- Packaged and installed backend SHA-256 matched `target/release/gosling`:
  `35ff36d4885c8409c31e818b2ec07b8dec8fe48123fe939108614dd2dcc515c4`.

The repository-wide desktop lint command remains red on pre-existing findings outside this patch:
four DOM-global errors in `components/ui/scroll-area.test.tsx` and four hook dependency warnings in
`components/Hub.tsx` and `hooks/useNavigationSessions.ts`.

## Regression review

- Message parsing, session serialization, database schema, provider data, and credential handling
  are unchanged.
- The transport remains bounded against both a single runaway frame and queued-message growth.
- The error now identifies whether the per-message or aggregate-buffer limit was crossed.
- The changed frame allowance applies only within the pre-existing aggregate budget; it does not
  make the WebSocket queue unbounded.

## Installation and status

The fixed application replaced `/Applications/Gosling.app` atomically. The prior installed bundle
is recoverable at
`/Applications/.gosling-session-resume.DDKXT4/Gosling.previous.app`.

Final GUI resume verification is pending the one-time macOS Keychain authorization caused by the
new ad-hoc development signature. Current status: `completed_with_partial_verification`.

## Residual risks and follow-up

- Provider inventories are still returned eagerly, and the desktop currently makes redundant
  provider-list requests during startup. Pagination or a metadata-only provider-list mode would
  reduce startup CPU, memory, and transport traffic, but is a separate API/performance change.
- A future valid response above 8,000,000 characters will still fail closed and should be addressed
  through pagination or streaming, not by making the transport unbounded.


## Provider inventory residual update - 2026-07-20

Redundant and concurrent provider-inventory startup requests are source-repaired with cache/in-flight coalescing and mutation invalidation. Provider pagination or metadata-only transport remains future API work. The 8,000,000-character ACP response ceiling remains intentional fail-closed behavior.
