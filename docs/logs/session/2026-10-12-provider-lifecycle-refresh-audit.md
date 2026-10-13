# 2026-10-12 — Provider, lifecycle and refresh audit

## Task

Scan Gosling for Anthropic, Mistral, OpenRouter and OpenAI compatibility, excessive refreshes, redundant work, lifecycle/race errors, lost input handoffs and misleading button behavior.

Baseline: `ed8dbc2d8b90fb1a0cb378b1620f950b7e0181ef` on `work`; clean checkout at the start. Source version remains 1.5.0. Changes are local and have not been packaged, installed, pushed or merged.

## Repairs

- **F01 — Anthropic model discovery**: Read all pages using encoded cursors, request 1,000 models per page, deduplicate IDs, and reject non-terminating pagination or later-page errors. Source: `crates/gosling-providers/src/anthropic.rs:160`.
- **F02 — Anthropic streaming errors**: Surface typed provider errors and malformed frames instead of silently dropping them. Source: `crates/gosling-providers/src/formats/anthropic.rs:798`.
- **F03 — Anthropic truncated responses**: Require message_stop or the compatible DONE sentinel; partial output remains an interrupted response. Source: `crates/gosling-providers/src/formats/anthropic.rs:1014`.
- **F04 — Stream-start retry handoff**: Keep HTTP setup and the first stream item inside the provider retry boundary. Delivered partial work is outside that boundary. Source: `crates/gosling/src/providers/openrouter.rs:10`.
- **F05 — OpenRouter reasoning continuity**: Replay details for all model families and bind them to tool-call IDs so message reformatting cannot attach them to unrelated output. Source: `crates/gosling/src/providers/formats/openrouter.rs:44`.
- **F06 — Mistral model metadata**: Use zai-glm-5-3, declare Large 4 at 1,000,000 tokens, and add current Medium 3.5 and Small 2603 entries. Keep the documented Large 4 alias and existing default. Source: `crates/gosling/src/providers/declarative/mistral.json:27`.
- **F07 — Mistral content decoding**: Decode thinking/text arrays in both response modes, including transitions from chunk arrays to plain text. Source: `crates/gosling-providers/src/formats/openai.rs:150`.
- **F08 — Mistral request handoff**: Replay reasoning as native thinking content and map selected effort to supported Mistral or hosted GLM values, preserving explicit request parameters. Source: `crates/gosling-providers/src/formats/mistral.rs:5`.
- **F09 — Declared reasoning capabilities**: Carry declared reasoning support into runtime configuration and both initial and refreshed inventory; preserve an explicit runtime false value. Source: `crates/gosling/src/providers/inventory/mod.rs:1107`.
- **F10 — OpenAI Off behavior**: Send none for those models; keep the required low minimum for Astra and 6.1 Sol. Source: `crates/gosling-providers/src/formats/openai.rs:1894`.
- **F11 — OpenAI request parameters**: Forward parameters such as text settings, service tier and cache options while retaining app control of model, input, tool definitions and streaming. Source: `crates/gosling-providers/src/formats/openai_responses.rs:696`.
- **F12 — OpenAI tool argument validation**: Reject malformed or non-object arguments before a tool can be dispatched in either response mode. Source: `crates/gosling-providers/src/formats/openai_responses.rs:711`.
- **F13 — OpenAI terminal outcomes**: Require a completed outcome with content, report incomplete status/reason, and fail a stream that ends without completion. Source: `crates/gosling-providers/src/formats/openai_responses.rs:1044`.
- **F14 — Unmanaged background Responses**: Reject unsupported background work before sending the request. Source: `crates/gosling-providers/src/formats/openai_responses.rs:601`.
- **F15 — Navigation refresh and stale data**: Use event-driven, coalesced refreshes, ignore expired lifecycles, and refresh again only when invalidated during an in-flight request. Titles use notifications. Source: `ui/desktop/src/hooks/useNavigationSessions.ts:139`.
- **F16 — Backend startup and cleanup**: Install listeners first, order unregister after register, handle already stopped children, reap the owned Unix process group promptly, and handle Windows taskkill launch errors. Source: `ui/desktop/src/goslingServe.ts:508`.
- **F17 — Process registry update race**: Merge the sweep result through the existing update queue, preserving fresh records and avoiding resurrection of records removed during the sweep. Source: `ui/desktop/src/backendProcessRegistry.ts:265`.
- **F18 — Quit during window cleanup**: Track pending cleanup promises, share them across repeated calls, and make cleanupAll await cleanup already started by closing windows. Source: `ui/desktop/src/goslingServeLeaseRegistry.ts:20`.
- **F19 — MCP adapter supervision**: Wait for child exit or cancellation, kill/reap on cancellation, and notify after supervision completes. Source: `crates/gosling/src/agents/extension_manager/operator_stdio.rs:170`.
- **F20 — Clock refresh**: Update at minute boundaries and pause while hidden; refresh on visibility change. No calendar-year polling was found. Source: `ui/desktop/src/components/Hub.tsx:77`.
- **F21 — Reconnect feedback**: Display the reconnect error and clear it after recovery or a new attempt. Source: `ui/desktop/src/components/BackendConnectionNotice.tsx:25`.
- **F22 — Send now at turn completion**: Close steering atomically with its pending-queue check, fence acceptance to the matching non-cancelled run, close the window on stream drop, and recognize confirmed delivery after run completion. Source: `crates/gosling/src/agents/agent/hooks.rs:139`.
- **F23 — Terminal idle rendering**: Run those 300 ms animation timers only in the loading/saving phases where the spinner is visible. Source: `ui/text/src/configure.tsx:374`.

## Files changed

- `.gitignore`
- `crates/gosling-providers/src/anthropic.rs`
- `crates/gosling-providers/src/formats.rs`
- `crates/gosling-providers/src/formats/anthropic.rs`
- `crates/gosling-providers/src/formats/mistral.rs`
- `crates/gosling-providers/src/formats/openai.rs`
- `crates/gosling-providers/src/formats/openai_responses.rs`
- `crates/gosling-providers/src/openai.rs`
- `crates/gosling-providers/tests/provider_contract_regressions.rs`
- `crates/gosling/src/acp/server/active_runs.rs`
- `crates/gosling/src/agents/agent.rs`
- `crates/gosling/src/agents/agent/hooks.rs`
- `crates/gosling/src/agents/agent/reply_stream.rs`
- `crates/gosling/src/agents/agent/tests.rs`
- `crates/gosling/src/agents/extension_manager/operator_stdio.rs`
- `crates/gosling/src/config/declarative_providers.rs`
- `crates/gosling/src/providers/declarative/mistral.json`
- `crates/gosling/src/providers/formats/openrouter.rs`
- `crates/gosling/src/providers/inventory/mod.rs`
- `crates/gosling/src/providers/openrouter.rs`
- `crates/gosling/src/providers/provider_registry.rs`
- `docs/INDEX.md`
- `docs/logs/session/2026-10-12-provider-lifecycle-refresh-audit.md`
- `ui/desktop/src/backendProcessRegistry.test.ts`
- `ui/desktop/src/backendProcessRegistry.ts`
- `ui/desktop/src/components/BackendConnectionNotice.test.tsx`
- `ui/desktop/src/components/BackendConnectionNotice.tsx`
- `ui/desktop/src/components/Hub.tsx`
- `ui/desktop/src/goslingServe.test.ts`
- `ui/desktop/src/goslingServe.ts`
- `ui/desktop/src/goslingServeLeaseRegistry.test.ts`
- `ui/desktop/src/goslingServeLeaseRegistry.ts`
- `ui/desktop/src/hooks/useChatSession.test.tsx`
- `ui/desktop/src/hooks/useChatSession.ts`
- `ui/desktop/src/hooks/useNavigationSessions.lifecycle.test.tsx`
- `ui/desktop/src/hooks/useNavigationSessions.ts`
- `ui/text/src/configure.tsx`
- `ui/text/src/extensions.tsx`
- `ui/text/src/onboarding.tsx`

## Validation

- `vitest run --maxWorkers=3 --testTimeout=10000` — 214 files; 1,655 passed; 1 skipped.
- `cargo test -p gosling-providers --features rustls-tls` — 518 passed across unit and integration suites, including 14 new provider-contract regression cases.
- `cargo test -p gosling --lib providers:: --no-default-features` — 482 passed.
- `cargo test -p gosling --lib steer --no-default-features` — 7 passed, including simultaneous acceptance/closure and existing mid-turn delivery tests.
- `cargo test -p gosling --lib operator_stdio_tests --no-default-features` — 4 passed, including real child exit and reaping.
- `vitest run goslingServeLeaseRegistry.test.ts goslingServe.test.ts backendProcessRegistry.test.ts` — 33 passed; included in full desktop suite.
- `tsc --noEmit in ui/desktop and ui/text` — Both passed.
- `eslint <changed TS/TSX files> --max-warnings 0` — Passed.
- `cargo clippy -p gosling -p gosling-providers --all-targets --no-default-features -- -D warnings` — Passed.
- `cargo fmt --all; git diff --check; changed Desktop Prettier` — Passed; terminal extensions.tsx has pre-existing unrelated Prettier differences confirmed against HEAD.

## Remaining work and limits

- **R01 — OpenAI state fidelity** (Provider gap): Reasoning enums retain visible summaries but do not retain encrypted_content, and assistant phase is not represented for replay. Current stateless tool continuations therefore do not preserve every native Responses output item. Next step: Add a provider-owned replay representation that survives persistence, compaction and provider transitions; verify full item order and state against real API fixtures. Source: `crates/gosling-providers/src/formats/openai_responses.rs:56`.
- **R02 — Windows orphan process identity** (Process gap): Recorded PID and arguments are trusted when deciding whether a Windows PID is a Gosling backend. PID reuse can make that identity unreliable. Next step: Verify the running executable, command line and process start identity before reclaiming a recorded PID; perform native Windows replay. Source: `ui/desktop/src/backendProcessRegistry.ts:161`.
- **R03 — Separate-process registry writers** (Race candidate): The registry update queue serializes writers inside one Node process, but provides no file lock between separate Electron processes sharing userData. Next step: Use a cross-process registry lock or separate registries by owning instance. Reproduce two-writer and owner-crash cases. Normal single-instance behavior limits this exposure. Source: `ui/desktop/src/backendProcessRegistry.ts:23`.
- **R04 — Terminal shutdown edge cases** (Process gap): The terminal shutdown helper treats child.killed as if the child has exited; repeated signals can end the parent before the existing grace period. Normal cleanup sends TERM without its own escalation deadline. Next step: Share one awaited shutdown operation for normal exit and signals, distinguish signalled from exited, and test slow or TERM-ignoring ACP children. Source: `ui/text/src/tui.tsx:1616`.
- **R05 — Model-specific effort and dated aliases** (Compatibility follow-up): The UI offers generic effort choices, while adapters clamp some selections. Older o-series/pro Off behavior and dated models normalized to current canonical aliases need further conformance checks. Next step: Expose effective supported levels per model and use provider-native capability/lifecycle data for dated IDs. Verify older-model requests rather than inferring full compatibility from current aliases. Source: `crates/gosling-providers/src/canonical/name_builder.rs:9`.
- **R06 — Large-session costs** (Optimization candidate): Two conversation clone sites and whole-message JSON streaming checkpoints remain documented cost candidates. Their existence is not evidence that they dominate normal turn latency. Next step: Profile long sessions and checkpoint serialization before changing conversation ownership or durability ordering. Preserve recovery guarantees. Source: `docs/logs/session/2026-09-19-hotpath-perf-fixes.md:21`.
- **R07 — Steer recovery after failed turns** (Recovery follow-up): Closing the normal-completion race does not make the in-memory steering queue durable. A steer still pending when a provider fails needs explicit reconciliation before restart, retry or cancellation semantics can be certified end to end. Next step: Define durable accepted/delivered/rejected states and replay failed-turn, disconnect and restart cases without silently dropping or duplicating input. Source: `crates/gosling/src/acp/server/active_runs.rs:328`.

- Scope: selected checkout, provider wire formats, provider metadata/inventory, common agent handoffs, Desktop actions/refresh and process ownership, plus terminal timer/shutdown inspection. This is not an exhaustive native playtest of every button.
- Official provider documentation was checked on October 12, 2026. No authenticated live provider calls, provider-account model inventory, billing tests or installed desktop rebuild were performed. The cloud environment has no configured provider credentials.
- No installed/running Gosling app was available here. Runtime process evidence comes from disposable Linux children, not a scan of the user's computer. Native macOS and Windows behavior remains unverified.
- The initial unwrapped baseline tests left eight defunct test helpers under the container's non-reaping PID 1 (one esbuild and seven sleep processes). Later subprocess tests used a subreaper and reaped their children. No live Gosling backend remained. These container artifacts are separate from product lifecycle evidence and prevent a blanket zero-zombie claim.
- Existing safeguards inspected: provider inventory becomes stale after 24 hours; active-run status checks are every 15 minutes; terminal viewport animation is already gated to visible splash/loading; provider setup retry exhaustion is excluded from core mid-stream retry; tool execution prevents replay of a failed stream after side effects.
- Shutdown and steering fixes were tested in controlled regressions. Failed-turn steer durability, full native reasoning-state replay, and cross-process/native identity checks remain follow-ups listed below.
- One early focused Desktop run timed out under parallel compilation; its isolated rerun and both wider Desktop passes succeeded. Old Mistral assertions were replaced with assertions for the actual native request shape. Intermediate compile errors were corrected before successful final validation.

## Provider sources

- [Anthropic models pagination](https://platform.claude.com/docs/en/api/models/list)
- [Mistral reasoning and content chunks](https://docs.mistral.ai/studio/conversations/reasoning)
- [Mistral Large 4](https://docs.mistral.ai/models/mistral-large-4-0)
- [Mistral hosted GLM 5.3](https://docs.mistral.ai/models/zai-glm-5-3)
- [Mistral Small 4](https://docs.mistral.ai/models/mistral-small-4-0-26-03)
- [OpenRouter reasoning continuity](https://openrouter.ai/docs/guides/best-practices/reasoning-tokens)
- [OpenAI GPT-6 model and effort rules](https://developers.openai.com/api/docs/guides/latest-model)
- [OpenAI stateless reasoning replay](https://developers.openai.com/api/docs/guides/reasoning)


## 2026-10-13 R04 repair pointer

R04's terminal shutdown defects now have local repairs in [2026-10-13-grok-findings-repair.md](2026-10-13-grok-findings-repair.md), tracked as FSR-GROK-003. Normal Ink exit and text completion/failure share an idempotent grace/kill/reap operation with signals; child.killed no longer implies exit. Terminal typecheck and three TUI tests pass, including successful/failed text runs with a TERM-ignoring ACP fixture. Interactive /exit and repeated-signal replay remain unverified. The original R04 observation above is historical; the linked record is the current qualified disposition.
