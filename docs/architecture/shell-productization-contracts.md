# Shell productization implementation contracts

Status: accepted Gate 2 design contract
Date: 2026-08-12
Authority: ADR-0007, ADR-0008, ADR-0009 and existing Rust shell contracts

This document freezes inputs, boundaries, state, and acceptance oracles before implementation. It
contains no claim that the modules already exist or conform.

## Product-profile schema v1

The source JSON object is strict: every property below is required unless marked optional, and
unknown properties fail validation.

```text
schemaVersion: 1
product
  id: lower kebab identifier, 3..48 characters
  displayName: trimmed operator-facing name, 1..64 characters
  version: semver string
  runtimeNamespace: lower kebab identifier, 3..64 characters
  protocolScheme: lower ASCII scheme, 3..32 characters
  executableName: portable leaf, 1..64 characters
  macosBundleId: reverse-DNS identifier, <=255 characters
  windowsAppId: dotted identifier, <=128 characters
  linuxPackageName: lower package identifier, <=64 characters
  flatpakId: reverse-DNS identifier, <=255 characters
provisioningPath: repository-relative JSON path under an approved profile/fixture root
compatibility
  goslingVersion: exact bundled Gosling package version
  goslingRevision: `current` for this checkout or an exact 40-character source revision
  provisioningSchemaVersion: 1
  handoffSchemaVersion: 1
  requiredMethods: sorted unique list of required shell ACP method names
assets
  root: repository-relative non-symlink directory
  iconBase: repository-relative path stem under assets.root
  requiredTargets: non-empty subset of macos-arm64, macos-x64, windows-x64, linux-x64
update
  enabled: boolean
  channel: lower kebab identifier
  owner: optional repository owner
  repository: optional repository name
distribution
  publishable: boolean
  artifactPrefix: lower portable identifier, 3..64 characters
  releaseDestination: optional approved destination key, never a free-form URL
  signingPolicy: none or required
```

Canonicalization recursively sorts object keys, preserves array order only where semantic, sorts
`requiredMethods` and `requiredTargets`, emits UTF-8 JSON without insignificant whitespace, and
computes lowercase SHA-256 over those bytes. `goslingRevision: current` is resolved from the clean
checkout's exact HEAD only in the generated manifest; an explicit 40-character revision must match
HEAD. Dirty checkouts may validate profiles but cannot produce a publishable manifest. The generated
manifest adds profile hash, target, architecture, resolved core revision, and known schema versions;
it never adds secrets.

### Rejected content

- keys or values shaped as password, token, secret, API key, credential value, private key, cookie,
  authorization header, or raw server secret;
- workspace/profile/provider/model/extension/skill selection, denied-method policy, domain adapter,
  prompt, action, resource payload, or handoff content;
- absolute, traversal, NUL-containing, out-of-root, or symlink-escape paths;
- duplicate normalized protocol, namespace, package/bundle/app ID, executable, update channel, or
  artifact prefix across approved profiles;
- `publishable: true` with fixture IDs, updater disabled/incomplete, missing target assets,
  `signingPolicy: none`, or no approved release destination;
- `publishable: false` with updater enabled or a release destination.

## Fixture identities

| Field | Fixture A | Fixture B |
| --- | --- | --- |
| product ID | `gosling-shell-fixture-a` | `gosling-shell-fixture-b` |
| display name | `Gosling Shell Fixture A` | `Gosling Shell Fixture B` |
| version | `0.0.0-test` | `0.0.0-test` |
| namespace | `shell-fixture-a` | `shell-fixture-b` |
| protocol | `gosling-fixture-a` | `gosling-fixture-b` |
| executable | `gosling-shell-fixture-a` | `gosling-shell-fixture-b` |
| macOS bundle ID | `io.github.repo_makeover.gosling.fixture.a` | `io.github.repo_makeover.gosling.fixture.b` |
| Windows app ID | `Gosling.Shell.Fixture.A` | `Gosling.Shell.Fixture.B` |
| Linux package | `gosling-shell-fixture-a` | `gosling-shell-fixture-b` |
| Flatpak ID | `io.github.repo_makeover.Gosling.FixtureA` | `io.github.repo_makeover.Gosling.FixtureB` |
| update channel | `fixture-a-disabled` | `fixture-b-disabled` |
| artifact prefix | `gosling-shell-fixture-a` | `gosling-shell-fixture-b` |
| publish/update/sign | false/false/none | false/false/none |

Fixtures contain generic connectivity/status/probe text only. Domain nouns, payload
interpretation, actions, prompts, branding claims, and production destinations fail the negative-space audit.

## Module contracts and dependency direction

| Module | Owns | Must not own | Allowed dependencies | Primary oracle |
| --- | --- | --- | --- | --- |
| profile types/resolver | raw parsing, strict validation, canonical hash, collision/asset report | Electron, ACP, secrets, domain semantics, publishing | Node standard library | golden/hostile profile tests |
| Forge/workflow adapter | mechanical projection of resolved manifest | independent identity defaults, policy, profile parsing duplication | resolver output, Forge/workflow APIs | default parity and package readback |
| app identity | pre-ready app paths/name/partition/protocol derivation | config/credential storage, session policy | resolved profile, Electron app adapter | ordering/path/coexistence tests |
| shell bootstrap | lifecycle sequencing and generation ownership | profile parsing rules, domain behavior, renderer UI | resolved profile, host, lifecycle, IPC, ACP | call-order/failure cleanup tests |
| minimal host / `goslingServe` | child launch/readiness/TLS/process cleanup | Electron app identity, renderer state, updater | process/fs/net adapters | existing + failure-path tests |
| compatibility | pure expected/actual comparison | network/session creation/migration | profile contract, generated ACP metadata | table-driven matrix tests |
| lifecycle reducer | legal state transitions and stale-generation rejection | process side effects, UI rendering | typed events only | exhaustive transition tests |
| shell IPC/main handlers | payload/sender/state validation and actions | raw renderer trust, broad filesystem/settings API | bootstrap, diagnostics, handoff | malformed/oversized/negative-space tests |
| shell preload | frozen typed bridge | ACP URL/secret, identity override, arbitrary IPC/filesystem/settings | shell channel constants | surface snapshot/reverse trace |
| ACP adapter | authenticated initialize/provision/read/session/handoff calls | hand-written canonical DTOs, policy decisions | generated SDK/types | call-order/integration tests |
| diagnostics | allowlisted bounded redacted snapshot/export | arbitrary log collection, secret/profile dump | lifecycle/host/package summaries, atomic writer | sentinel/size/permission tests |
| shared renderer provider | display state and invokes allowed actions | Electron/process/filesystem/policy/domain semantics | preload bridge, presentation components | state/action/UI tests |
| fixture renderer | neutral acceptance controls | domain behavior or publish controls | shared renderer interface | packaged smoke/negative-space audit |
| package verifier | post-build artifact/readback comparison | mutation or metadata repair | generated manifest, platform readers | tamper/readback tests |
| release adapter | guarded build/attest/upload sequencing | profile authority, free-form destination, fixture promotion | resolver/verifier output, pinned actions | workflow dry-run/security audit |

Dependency direction is `raw profile → resolver → build/runtime adapters`; and
`main bootstrap → narrow preload → renderer`. Renderer never imports Node/Electron main/process/Forge
modules. Packaging never implements runtime policy. Rust remains provisioning and session authority.

## Application and runtime path matrix

All path derivation occurs before Electron readiness/locking. `<id>` is the validated profile ID or
namespace; exact platform roots use Electron/OS APIs rather than string-literal home assumptions.

| Surface | Full Gosling | Focused shell | Sharing policy |
| --- | --- | --- | --- |
| protected config and credential catalog | canonical Gosling root | canonical Gosling root via backend runtime | intentionally shared, backend-only |
| Electron userData | Gosling app root | product-specific `<id>` root | isolated |
| browser session partition | `persist:gosling` | `persist:gosling-shell-<id>` | isolated |
| logs/diagnostics | Gosling userData logs | shell userData logs | isolated |
| process registry | Gosling userData registry | shell userData registry | isolated |
| cache/temp | Gosling app namespace | shell product namespace | isolated |
| protocol | `gosling` | profile protocol | isolated |
| single-instance lock | Gosling app identity | profile app identity | isolated |
| backend runtime namespace/data/state | Gosling default | profile runtime namespace | isolated |
| updater channel/feed | Gosling updater | disabled or profile-specific | isolated |

A path matrix mismatch is a Gate 6 blocker; no fallback silently converts an isolated path to a
shared one or vice versa.

## Narrow preload and IPC allowlist

The renderer bridge exposes exactly these operations; later additions require ADR/change-control
review and a typed negative-space test.

| Operation | Direction | Input bound | Output/event bound | Main behavior |
| --- | --- | --- | --- | --- |
| `runtime.read` | invoke | none | allowlisted snapshot <=64 KiB | returns current generation/state only |
| `runtime.retry` | invoke | expected generation integer | typed result <=8 KiB | rejects stale/illegal request; full cleanup then fresh generation |
| `runtime.stop` | invoke | expected generation integer | typed result <=8 KiB | idempotent bounded stop |
| `diagnostics.save` | invoke | expected generation + explicit user gesture | result path/status <=8 KiB | native save dialog, atomic private bounded export |
| `handoff.prepare` | invoke | session/capability/question/references/mutation intent, total <=64 KiB | server-prepared summary <=64 KiB | calls canonical ACP method; does not open destination |
| `handoff.confirm` | invoke | handoff ID + expected generation | status <=8 KiB | confirms current prepared envelope and opens allowlisted full-Gosling protocol |
| `external.open` | invoke | allowlisted HTTP(S) support URL <=2 KiB | status <=8 KiB | parses and opens only configured support origins |
| `runtime.changed` | event | main only | snapshot <=64 KiB | generation-fenced lifecycle update |

Explicitly absent: arbitrary file read/write/delete/list, settings get/set, directory chooser,
clipboard, notifications, updater operations, app restart, generic logging, raw ACP URL, MCP proxy
URL, server secret, profile/provisioning/namespace mutation, shell command, or arbitrary IPC channel.

## Compatibility contract

The first release uses exact bundled-core compatibility; semver ranges and external core discovery
are unsupported.

| Check in required order | Expected | Actual source | Failure code |
| --- | --- | --- | --- |
| profile schema | 1 | resolver | `PROFILE_SCHEMA_UNSUPPORTED` |
| profile identity vs provisioning | exact ID/name/version | resolved profile + provisioning read | `IDENTITY_MISMATCH` |
| bundled Gosling version/revision | exact version and resolved build revision (`current` → manifest HEAD) | embedded manifest/package | `CORE_MISMATCH` |
| provisioning schema | 1 | Rust provisioning response | `PROVISIONING_SCHEMA_UNSUPPORTED` |
| required ACP methods | every exact method available | authenticated initialization/custom-method capability | `METHOD_UNAVAILABLE` |
| handoff schema | 1 when handoff is used | embedded expected + prepared envelope | `HANDOFF_SCHEMA_UNSUPPORTED` |
| provisioning validation | `valid: true` | server response | `PROVISIONING_INVALID` |

Current ACP initialization does not enumerate required custom methods, while the Rust server already
derives the authoritative list through `GoslingAcpAgent::custom_method_schemas`. Gate 4 must expose
that canonical generated capability metadata additively before the renderer compatibility check; it
must not probe by creating a session or maintain an independent TypeScript list. Failure includes
expected/actual non-secret values and occurs before session create/resume.

## Lifecycle and error contracts

### States

`booting → validating → ready ↔ busy`; recoverable failures enter `degraded`,
`relink_required`, `incompatible`, or `offline`; ownership shutdown enters `stopping`; unrecoverable
contract/internal failures enter `fatal`. Every state carries generation, entered timestamp,
allowlisted reason code, allowed actions, and no raw error object.

Legal recovery:

- `degraded|offline → stopping → booting` through retry with a fresh generation/secret;
- `relink_required → stopping` and then full Gosling handoff or retry after external relink;
- `incompatible → stopping` and diagnostic/handoff only;
- any live state → `stopping → stopped` on expected quit;
- unexpected child exit/transport loss → `offline` or `fatal` based on cleanup integrity.

Events from older generations are ignored and counted in diagnostics. Illegal transitions produce
an internal-bug diagnostic and do not mutate state. `stop` is idempotent; retry cannot overlap stop.

### Error taxonomy

| Category | Representative codes | User actions |
| --- | --- | --- |
| profile/build | invalid schema/path/asset/identity/collision | fix reviewed profile; no launch |
| provisioning/reference | missing/invalid workspace, profile, extension, skill, policy | open diagnostics or full Gosling |
| credential relink | missing/unavailable/mismatched credential profile | explicit full-Gosling relink handoff |
| compatibility | core/schema/method/identity/handoff mismatch | stop, diagnostics, install compatible artifact |
| environment/startup | binary/resource/port/TLS/readiness/process-registry failure | retry, diagnostics, quit |
| policy denial | server-denied custom method/action | explain denial; no retry-as-bypass |
| transport/backend | disconnect, early exit, crash, forced cleanup | retry after complete cleanup or quit |
| package integrity | embedded manifest/resource/identity mismatch | stop; artifact invalid |
| internal bug | illegal transition/unexpected invariant | stop, diagnostics, report |

Messages expose stable codes and actionable non-secret summaries; raw child stderr, profile contents,
paths, credentials, prompts, server secret, and auth query/header values never enter renderer state.

## Diagnostic contract

Export is JSON with schema version, timestamp, product/profile hash and safe identity, package
version/revision/target, lifecycle state/reason/generation, non-secret compatibility expected/actual,
provisioning issue codes/paths (not values), child exit code/signal, bounded event names, and checks
for resource/process-registry cleanup. Each string is bounded, event counts are capped, total output
is capped at 1 MiB, and home paths are replaced with `<home>` where included. Writes use a native
save dialog, temporary file + fsync + rename, owner-private mode where supported, and no silent
overwrite. The UI warns the user to inspect before sharing.

## Threat model and mandatory controls

| Boundary/input | Threat | Required control | Proving test |
| --- | --- | --- | --- |
| profile/path/assets | traversal, symlink escape, secret/policy injection | strict schema, approved roots, canonical containment, secret/domain-key rejection | hostile profile corpus |
| renderer/IPC | identity/process/file/updater confused deputy | separate preload, frozen allowlist, sender/type/size/state checks | surface snapshot + malformed/oversized calls |
| loopback ACP | secret theft, remote endpoint, TLS substitution | generated secret, loopback-only URL, authenticated ACP, pinned dev TLS | auth/host/fingerprint negative tests |
| child process | orphan/stale registry, stale event, wrong binary | single owner, packaged resource lookup, generation fencing, bounded graceful/forced cleanup | crash/kill/retry/PID tests |
| diagnostics/logs | secret/content/path exfiltration or unbounded output | allowlisted schema, sentinels, path redaction, byte/count limits, private atomic write | sentinel/size/permission/failure tests |
| deep link/handoff | forged origin/reference, arbitrary scheme, implicit mutation | server-prepared envelope, schema/ID/current-generation check, explicit confirm, scheme/action allowlist | malformed/mismatched/no-confirm tests |
| package/update | Gosling fallback assets/feed, identity substitution | complete profile assets, embedded manifest/hash, platform readback, updater default off | missing/tamper/cross-profile tests |
| workflow/release | input injection, fixture promotion, cross-publication, secret exposure | source-controlled profile, no `eval`, generated artifact names, privileged re-resolution/hash match, least privilege | hostile inputs/permissions/dry run |
| signing credentials | untrusted code access | protected environment/human approval; no fork-PR signing | platform control readback/manual gate |

Unsupported threat model: hostile local administrator or compromised OS/keychain; malicious Gosling
core binary already trusted by the package; arbitrary third-party domain adapter semantics. These do
not weaken renderer, profile, release, or server policy boundaries.

## Gate 3–8 acceptance handoff

Implementation may change file names through an L1 plan record, but not these boundaries without an
L2/L3 review. Required audit oracles are:

1. profile golden/hostile/collision and deterministic hash tests;
2. preload surface snapshot and reverse registration trace;
3. compatibility and lifecycle exhaustive tables plus no-session-on-failure integration;
4. crash/stop/retry/process-registry and three-product path/lock/protocol tests;
5. diagnostic sentinel/size/permission/atomic-write tests;
6. package embedded-manifest/resource/identity readback and tamper tests;
7. workflow fixture/sign/update/publish rejection and least-privilege audit;
8. actual packaged renderer-to-backend smoke and final revision traceability.
