# Traceability matrix — Gosling shared shell productization

Status vocabulary: `planned` → `built` → `verified`; or `deferred` / `cut` through an accepted plan-change record. `verified` requires revision-specific observed evidence.

Baseline: `main` at `8627dc31a`
Last updated: Gate 3 implementation, 2026-08-12

## Requirements

| REQ | Pri | Requirement | Design/module refs | Planned implementation | Planned verification | Evidence target | Status |
| --- | --- | --- | --- | --- | --- | --- | --- |
| SHP-REQ-001 | P0 | Versioned, secret-free product profile resolves all shared identities and rejects unsafe/incomplete input | ADR-0007; product profile; build resolver | `ui/desktop/src/shell/profile*.ts`; resolver script; fixture profiles | schema/golden, unknown version/field, path, secret-shape, identity/collision tests | Gates 2–3/8 | built; local profile/hostile tests pass |
| SHP-REQ-002 | P0 | Product profile cannot override runtime provisioning or server authority | ADR-0007; profile/provisioning boundary | profile resolver + existing Rust provisioning validation | profile/provisioning mismatch and environment-override negative tests | Gates 2–4/8 | built for profile boundary; runtime preflight pending Gate 4 |
| SHP-REQ-003 | P0 | Shared Electron production entrypoint owns backend, secure window, renderer, and cleanup | ADR-0008; bootstrap | `shell/bootstrap.ts`; shell Vite entry | integration startup/readiness/cleanup and packaged launch | Gates 4/6/8 | design frozen; implementation planned |
| SHP-REQ-004 | P0 | Shell preload exposes only typed lifecycle, diagnostics, and handoff operations | ADR-0008; preload/IPC | `shell/preload.ts`; `shell/ipcChannels.ts` | allowlist snapshot, malformed/oversized payload, forbidden Desktop IPC tests | Gates 4–6/8 | design frozen; implementation planned |
| SHP-REQ-005 | P0 | Shared renderer deterministically presents every common lifecycle/recovery state | lifecycle reducer; renderer provider | `shell/lifecycle.ts`; `renderer/ShellRuntimeProvider.tsx`; common components | transition table, stale event, action, component-state tests | Gates 4–5/8 | design frozen; implementation planned |
| SHP-REQ-006 | P0 | Packaged renderer establishes authenticated ACP and verifies fixed identity/provisioning before session use | ACP client; compatibility | `shell/acpClient.ts`; generated SDK | child integration and packaged E2E identity/read/create/resume | Gates 4/6/8 | design frozen; implementation planned |
| SHP-REQ-007 | P0 | Extension/tool/skill selection and denied methods are backend-enforced in packaged path | existing Rust shell policy; fixture E2E | neutral provisioning and packaged probes | selected skill/tool positive; omitted/denied negative; server denial observed | Gates 4/6/8 | design frozen; implementation planned |
| SHP-REQ-008 | P0 | Quit, close, startup failure, renderer crash, backend exit, and forced termination leave no orphan | bootstrap; existing host/lease/process registry | lifecycle cleanup adapters/tests | PID/registry tests across normal and failure paths; packaged cleanup | Gates 4/6/8 | design frozen; implementation planned |
| SHP-REQ-009 | P0 | Full Gosling and multiple shells isolate app and runtime identities | ADR-0008/0009; app identity | `shell/appIdentity.ts`; profiles A/B | path/lock/protocol/update identity units + three-app coexistence E2E | Gates 3/6/8 | built identity inputs; coexistence pending Gate 6 |
| SHP-REQ-010 | P0 | Unsupported core/profile/provisioning/method/handoff combinations fail before session use | compatibility policy | `shell/compatibility.ts`; additive metadata only if required | exact/unsupported/newer/missing compatibility matrix | Gates 2/4/6/8 | design frozen; implementation planned |
| SHP-REQ-011 | P0 | Actual packaged artifact proves renderer-to-backend primary workflow and restart | packaged smoke harness | `ui/desktop/e2e/shell-packaged.spec.ts` | package → launch → ACP → session → restart → diagnostics → quit twice | Gates 6/8 | design frozen; implementation planned |
| SHP-REQ-012 | P0 | One resolved profile drives every build/package identity and artifact name | ADR-0007/0009; build/package resolver | Forge adapter; canonical build manifest; verifier | default-Gosling parity, fixture package readback, tamper/collision tests | Gates 3/6/7/8 | built resolver/Forge/manifest; package readback pending |
| SHP-REQ-013 | P0 | Reusable workflows build, attest, and guard profile-specific artifacts | ADR-0009; release adapter | `bundle-shell.yml`; `release-shell.yml`; resolver action/script | workflow static tests, fixture dry run, artifact/readback/checksum/attestation inputs | Gates 7/8 | design frozen; implementation planned |
| SHP-REQ-014 | P0 | Clean Linux CI uses trusted exact V8 archive and executes Rust tests | supply-chain/CI; existing helper | helper hardening/test plus `.github/workflows/ci.yml` fresh prepare before rust-cache | local cold/warm/corrupt/wrong-target/checksum/network/concurrency tests passed; real Linux asset checksum/archive passed; two clean remote CI runs pending | Gates 0–1/8 | built; remote verification blocked |
| SHP-REQ-015 | P0 | Diagnostics are actionable, bounded, private, and redact secrets/content | diagnostics contract | `shell/diagnostics.ts`; existing startup diagnostics extension | sentinel, size, permissions, atomic write, failed export, bundle content tests | Gates 4/6/8 | design frozen; implementation planned |
| SHP-REQ-016 | P0 | Handoff is explicit and uses server-prepared exact versioned envelope | existing handoff DTO/handler; shell ACP/UI | ACP adapter + confirmation surface + protocol launcher | envelope/version/destination/mutation intent/exact refs/malformed URI tests | Gates 4–6/8 | design frozen; implementation planned |
| SHP-REQ-017 | P0 | Neutral fixtures are test-only, non-domain, and mechanically non-publishable | fixture contract; ADR-0009 | `fixtures/shell-products/**`; fixture renderer | source-content negative-space test; publish/update/sign rejection | Gates 3/5/7/8 | built neutral fixtures/hard denial; workflow proof pending |
| SHP-REQ-018 | P0 | Shell satisfies Electron, loopback, path, secret, dependency, and release security baseline | threat model; preload; CSP; release | cross-cutting modules/workflows | security checklist, CSP/navigation/IPC/deep-link/redaction/dependency audits | Gates 2/4–8 | design frozen; implementation planned |
| SHP-REQ-019 | P1 | Distribution asset contract validates required assets per target | package resolver | profile asset matrix + fixture test assets | missing/type/dimension/target mismatch and publishability tests | Gates 3/7/8 | built target asset validation; package readback pending |
| SHP-REQ-020 | P1 | Package matrix covers macOS ARM/x64, Windows x64, and Linux x64 or explicitly gates unsupported targets | package/release matrix | reusable platform jobs and verifier | structural/readback jobs each target; launch/full smoke as supported | Gates 7/8 | design frozen; implementation planned |
| SHP-REQ-021 | P1 | Updater identity/feed/channel are isolated; fixture/unsigned artifacts cannot update | ADR-0009; updater adapter | resolved updater config and release guards | disabled fixture tests, per-profile manifest readback, cross-feed negative tests | Gates 3/6/7/8 | built fixture updater/publisher denial; installed proof pending |
| SHP-REQ-022 | P1 | Every artifact embeds a redacted reproducible build manifest and profile hash | build resolver/package verifier | canonical generated manifest as extra resource | hash/revision/schema/target readback, deterministic output, tamper detection | Gates 3/6/7/8 | built deterministic manifest; embedded readback pending |
| SHP-REQ-023 | P1 | Developer can check/build/package a new profile without editing Forge/workflow source | extension recipe/build scripts | package scripts + `docs/SHELL_PRODUCTS.md` | clean-checkout neutral profile walkthrough | Gates 3/7/8 | built check/resolve recipe; dedicated shell package path pending |
| SHP-REQ-024 | P1 | User can export a redacted diagnostics bundle without generic renderer filesystem authority | diagnostics/preload/UI | narrow main-process save operation + component | chooser/cancel/failure/atomic/permissions/redaction E2E | Gates 4–6/8 | design frozen; implementation planned |
| SHP-REQ-025 | P1 | Shared states are keyboard/screen-reader/min-window/reduced-motion usable | shared renderer/UI | shell components/styles | focus order/restoration, ARIA/live-region, contrast, viewport tests | Gates 5/6/8 | design frozen; implementation planned |
| SHP-REQ-026 | P1 | Installed coexistence matrix observes full Gosling plus two fixture identities | app identity/E2E | fixtures A/B + coexistence harness | concurrent launch, locks, protocols, state, quit/restart/update isolation | Gates 6/8 | design frozen; implementation planned |
| SHP-REQ-027 | P1 | Architecture, profile reference, extension recipe, troubleshooting, release runbook, evidence, and handoff match code | documentation plan | shell docs/ADRs/index/release checklist/plan package | doc commands executed; path/API/diagram/source cross-check | Gate 8 | design frozen; implementation planned |
| SHP-REQ-028 | P1 | Failed upgrade supports rollback without mutating unrelated product state | compatibility/release rollback | profile migration/compatibility + release runbook | old/new profile fixture, failed start, rollback and state-hash tests | Gates 4/6–8 | design frozen; implementation planned |
| SHP-REQ-029 | P1 | Required Rust/SDK/Desktop/profile/package/workflow gates are green at acceptance | quality/evidence contract | CI/scripts/test suites | complete command matrix on final revision; no unexplained red check | Gate 8 | design frozen; implementation planned |
| SHP-REQ-030 | P2 | Optional scaffold emits profile skeleton and asset checklist only | profile extension seam | possible resolver subcommand | generation golden; no secrets/domain code | post-P1 | planned |
| SHP-REQ-031 | P2 | Optional common history/session/diagnostic components require two consumers | renderer extension seam | deferred until two real shells | consumer evidence and abstraction review | post-P1 | planned |
| SHP-REQ-032 | P2 | Optional external release destination adapter preserves least privilege | release extension seam | optional reusable workflow adapter | destination allowlist, permissions, dry run | post-P1 | planned |

## Gate exit traceability checks

| Gate | Forward trace required | Reverse trace required | Status rule |
| --- | --- | --- | --- |
| 0 | Confirm all IDs still represent operator intent | No implementation expected | all remain planned |
| 1 | SHP-REQ-014 maps to exact helper/workflow/tests | Every V8/CI edit maps to 014/018/029 | 014 may become verified for final SHA only after Gate 8 rerun |
| 2 | Every P0/P1 maps to ADR/module/file/test | Every proposed module serves at least one requirement | design refs complete |
| 3 | 001/002/009/012/017/019/021/022/023 built | Every profile/resolver/Forge change traced | targeted rows built, not verified |
| 4 | 003–010/015/016/018/024/028 built | Every bootstrap/preload/ACP/diagnostic path traced | targeted rows built |
| 5 | 005/015/016/024/025 built | Every shared component/action traced; no domain drift | targeted rows built |
| 6 | 003–012/015–018/021/024–026/028 observed in package | Package harness/fixtures contain no untraced capability | eligible rows verified with exact artifact evidence |
| 7 | 012/013/019–023/028/029 built and dry-run observed | Every workflow permission/input/artifact traced | no publish claim without readback |
| 8 | Every P0/P1 acceptance criterion observed | Every significant landed file maps to ID/change record | no `built` P0/P1 remains |

## Deferred/cut log

None at planning baseline. Any P0/P1 deferral or cut requires an operator-accepted plan-change record stating residual risk and impact on the definition of complete.
