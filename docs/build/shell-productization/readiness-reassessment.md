# Project-shell readiness reassessment

> R0 status update, 2026-08-13: the Linux baseline defects identified by this assessment were repaired and merged at `3feffca7c`. PR run `31731952749` and merged-main run `31732990062` both passed the helper, verified archive preparation, and full Rust tests. The project-shell readiness findings and R1 architecture requirement remain unchanged. See [`evidence/r0.md`](evidence/r0.md).

Date: 2026-08-13
Repository: `cephalopod-ai/gosling`
Assessed revision: `6fe6a3bfcab3a48846850fd321fb8a056223d355` (`main`)
Purpose: reassess the shared foundation before any DAWES, math, Project ABC, Physics/CST, or other project shell is implemented

## Executive decision

**The shared host substrate is materially real, but Gosling is not yet ready to accept an independently developed project shell.**

Gates 0–4 produced useful, mostly well-factored foundations: strict product identity, namespaced runtime state, a dedicated Electron main/preload/renderer package path, main-owned child and ACP lifecycle, generated contracts, bounded diagnostics, explicit handoff, and host-package integrity checks. Those assets should be retained.

The prior forward plan nevertheless assumed extension seams that do not exist in the running product. The current package always builds one hard-coded neutral renderer, the renderer cannot create or operate a session, and the generic Rust domain adapter has no production registration path. The remaining work therefore cannot safely begin as “Gate 5 UI.” It must first settle and prove the project-shell consumer topology, runtime API, and domain integration boundary.

The forward plan is superseded by [`project-shell-readiness-plan.md`](project-shell-readiness-plan.md). Historical Gate 0–4 evidence remains valid only for the exact claims and revisions it recorded.

## Evidence and method

The assessment used current source and live evidence rather than the previous completion summary:

- clean `main` at merge `6fe6a3bfc`;
- root instructions, README, docs index, shell architecture, ADR-0007–0009, plan package, and recent session logs;
- shell Electron, ACP, lifecycle, IPC, profile, Forge, package, verifier, Rust runtime, generated SDK, and CI source;
- current GitHub Actions state for `6fe6a3bfc`;
- local focused tests through the Hermit environment;
- Muninn recall. Muninn was available, but no authoritative shell-productization conversation was found. A broad search returned only tentative low-trust research artifacts that establish DAWES as programmatic work and Math_MCP as mathematical-method selection; those artifacts did not define a shell integration architecture and were not used as authority.

Observed local validation:

```text
scripts/test-with-rusty-v8-cache.sh                         passed on macOS
ui/desktop shell profile/package Node suite                 41/41 passed
focused shell/host/serve/handoff Vitest                     120/120 passed in 14 files
ui/desktop typecheck                                        passed
```

Observed remote validation for current `main`:

```text
Desktop lint/test job                                       passed
Rust format, Clippy, MSRV, schema/SDK, Windows build         passed
Canary and live-provider workflows                           passed
Linux “Build and Test Rust Project”                          failed before Rust tests
failure                                                      scripts/with-rusty-v8-cache.sh: line 44: File: unbound variable
```

The Linux failure is reproducible from the source shape: `archive_size` tries BSD `stat -f` first. GNU `stat -f '%z'` exits successfully with filesystem text, so the numeric `[[ ... -ge ... ]]` expression interprets `File` as a shell variable under `set -u`.

## What is strong and should be preserved

### 1. Build identity has one reviewed authority

ADR-0007 and `ui/desktop/scripts/shell-profile.js` establish a strict, deterministic, secret-free product profile. Unknown keys, unsafe paths, identity collisions, secret-shaped content, publish contradictions, and fixture promotion fail closed. Forge consumes a projection rather than a second identity configuration.

### 2. Runtime policy and persistence authority remain in Rust

`ShellProvisioning`, server-fixed identity, server-side custom-method denial, directory-aware validation, runtime namespaces, and generated ACP DTOs keep settings, credentials, session persistence, and policy out of renderer code.

### 3. Electron has a distinct least-privilege process boundary

Focused shells have dedicated main, preload, and renderer entries. The preload exposes a frozen eight-channel bridge rather than full Desktop authority. Main owns profile resolution, app identity, child generation, authenticated ACP, diagnostics, handoff, cleanup, and navigation restrictions.

### 4. Compatibility and failed-preflight ordering are explicit

The ACP adapter verifies identity, core version, required methods, provisioning schema, and validation before session create/resume. Live integration evidence showed incompatible preflight leaving no durable session and an empty process registry.

### 5. Handoff is explicit and non-executing

The sender consumes a server-prepared envelope once. Full Gosling validates canonical bounded encoding and opens a non-auto-submitted review draft without granting capability, performing mutation, fetching references, or navigating to embedded destinations.

### 6. Host-target package integrity has real readback

The local wrapper builds one exact CLI binary, packages it, and verifies source profile, manifest, provisioning, binary hash, dedicated entries, updater absence, protocol, executable, and core macOS identity. This is useful foundation evidence, though not complete distribution proof.

## Blocking findings

### PSR-001 — No copy-free project renderer composition seam

**Severity:** critical
**Evidence:** `ui/desktop/shell.html` always imports `src/shell/renderer.ts`; `vite.shell.renderer.config.mts` always builds that one HTML entry; `forge.config.ts` has one fixed `shell_window` renderer; the product profile contains no renderer/package composition contract.

The current renderer is 13 lines and displays only lifecycle text. A DAWES or math UI cannot be supplied from a separate project without editing Gosling Vite/Forge/source files or replacing the hard-coded entry. That contradicts the mission that a domain team can consume the host without copying or modifying Gosling orchestration.

**Required disposition:** define and prove a versioned project-shell consumer package/workspace contract with an independently supplied renderer entry and no Gosling core edits.

### PSR-002 — The renderer cannot perform the common agent workflow

**Severity:** critical
**Evidence:** `connectShellAcp` lives in Electron main. `GoslingShellAPI` exposes lifecycle, diagnostics, handoff, and external-open only. It exposes no session create/resume/read, prompt, cancel, streaming update, permission, elicitation, or domain operation.

A session is created only by tests or main-owned integration code. A real project renderer cannot ask Gosling a question or observe a response. The prior requirement that “the packaged renderer establishes authenticated ACP” is also inaccurate: main establishes ACP and intentionally withholds the token from renderer.

**Required disposition:** retain main-owned ACP and add a narrow, typed, bounded, generation-fenced application runtime API for session and domain workflows. Do not expose the ACP URL/token.

### PSR-003 — Focused ACP deliberately discards required interactive behavior

**Severity:** critical
**Evidence:** `clientCallbacks()` in `ui/desktop/src/shell/acpRuntime.ts` automatically cancels every tool permission request, declines every elicitation, and drops every session update. No prompt/cancel method is exposed on `ShellAcpConnection`.

This is safe for preflight but not a usable agent application. A DAWES-like coding workflow would silently deny tool execution; a math workflow could not stream or present model output.

**Required disposition:** extract or build focused pure reducers/controllers for session notifications, permission and elicitation requests, prompt cancellation, and reconnect. Renderer presentation remains narrow; server remains the execution authority.

### PSR-004 — The advertised domain adapter is unreachable in production

**Severity:** critical
**Evidence:** the Rust `DomainAdapter` trait and generated domain snapshot/action methods exist, but `crates/gosling-cli/src/cli.rs::build_shell_runtime` always calls `ShellRuntime::new(provisioning, None)`. There is no concrete `DomainAdapter` implementation or registry outside tests. Provisioning may still contain a descriptor, so validation can describe an adapter whose methods always return method-not-found.

The product profile’s exact required-method allowlist also excludes domain snapshot/action, and the Electron ACP adapter does not call those generated methods.

**Required disposition:** choose one truthful domain integration topology. The preferred direction is a versioned, out-of-process adapter/extension contract owned by the project shell, not Rust dynamic-library loading and not domain implementations compiled into Gosling. Until an ADR and conformance fixture prove this path, `DomainAdapter` is an unfulfilled internal seam, not a supported consumer capability.

### PSR-005 — Lifecycle vocabulary is broader than runtime behavior

**Severity:** high
**Evidence:** the reducer defines `busy`, `relink_required`, and `fatal`, but production controller code never transitions to those states. `degraded` is used only for `PROVISIONING_INVALID`; credential issue codes are not classified into `relink_required`; prompt work cannot drive `busy`; illegal transitions are returned but not recorded as the promised internal-bug diagnostic.

Gate 5 cannot merely render every enum member and call the workflow complete.

**Required disposition:** define event-to-state mapping from real startup, session, permission, domain, transport, and cleanup events; prove every user-visible state has a production producer and recovery action or remove it.

### PSR-006 — Verified runtime identity is not available to the renderer

**Severity:** high
**Evidence:** `runtime.read` returns only `ShellLifecycleState`. It does not include the server-verified identity, namespace, provisioning summary, current session, adapter descriptor, or compatibility result. The old Gate 5 plan asks UI to display these values without defining a bridge source.

**Required disposition:** introduce a bounded safe runtime snapshot contract populated only after ACP verification. Keep profile paths, credentials, ACP URL/token, and raw provisioning values out of it.

### PSR-007 — Focused packaging still carries Gosling-specific or broad metadata

**Severity:** high
**Evidence:** shell Forge projection still inherits Gosling calendar/reminder permission descriptions, directory document types, homepage, development category, full-Gosling Linux desktop templates, broad Flatpak permissions, and signing entitlements. `extraResource` includes the entire `src/bin` directory, which currently contains `uvx`, `jbang`, `node`, `npx`, and setup scripts in addition to `gosling`.

The verifier checks primary identity and forbidden renderer authority but does not read back these metadata/permission/resource surfaces.

**Required disposition:** make every included capability and platform metadata item either profile/host-contract derived or absent; stage only the exact required runtime binary/resources; add target readback and negative-space tests.

### PSR-008 — Current Linux CI is red

**Severity:** high
**Evidence:** GitHub Actions run `31695906352`, job `94433546982`, failed in the helper self-test before archive preparation or Rust tests. The prior plan text says remote evidence is blocked because push/PR was unauthorized, which is now stale because Gate 4 is merged and push CI ran.

**Required disposition:** repair the portable size probe, add Linux-specific regression coverage, and require two clean Linux CI runs on the same relevant revision lineage before the prerequisite returns to GO.

### PSR-009 — Gate 4 GO was narrower than its declared exit criteria

**Severity:** high
**Evidence:** Gate 4 declared failure paths including renderer crash, ACP disconnect/reconnect, graceful-timeout escalation, and every failure reaching a typed actionable state. Gate 4 evidence explicitly leaves the full failure matrix and packaged behavior to Gate 6. The implementation is useful, but the label overstates conformance to the original gate text.

**Required disposition:** preserve the historical “Gate 4 process-boundary GO” wording as point-in-time evidence, but reopen the omitted acceptance in the new runtime and packaged gates. Do not rewrite history into a broader pass.

### PSR-010 — Profiles prove identity, not external consumption

**Severity:** high
**Evidence:** approved profile roots are inside a Gosling checkout, `repositoryRoot()` requires Gosling `Cargo.toml` plus `ui/desktop`, and package scripts execute from the Gosling Desktop workspace. `docs/SHELL_PRODUCTS.md` says to add a profile under this repository. No packed SDK, template, external fixture, or compatibility policy proves a separate shell repository can consume the foundation.

**Required disposition:** add a consumer manifest/template and run a fixture from outside Gosling source or from an isolated workspace package that may touch only its own consumer directory and lockfile registration. The final oracle is “new shell without Gosling source edits,” not “third profile validates.”

### PSR-011 — Reusable shell build/release workflows do not exist

**Severity:** high
**Evidence:** no `bundle-shell.yml`, `shell-package-smoke.yml`, or `release-shell.yml` exists. Local package/readback is only macOS arm64 fixture A. Existing workflows are Gosling-specific.

**Required disposition:** add profile/consumer-driven fixture workflows only after the consumer seam and packaged application workflow pass. Release activation remains a later human gate.

### PSR-012 — Shared UI primitives are scaffolding, not an application shell kit

**Severity:** medium
**Evidence:** only `ShellFrame` and `ShellStatus` exist. There is no provider, runtime store, recovery UI, diagnostics UI, handoff confirmation, session view, permission surface, accessibility harness, or domain-content mounting contract.

**Required disposition:** build these only after the runtime/consumer contracts are frozen, so UI does not force an accidental IPC or domain architecture.

## Architecture conclusions

### Preserve

- product profile and provisioning remain separate authorities;
- Rust owns settings, credentials, policy, sessions, and domain-operation authorization;
- Electron main owns child lifecycle and authenticated ACP;
- renderer receives a typed capability-oriented API, never raw ACP or process/filesystem authority;
- full Gosling and every project shell have isolated application/runtime identity;
- handoff remains explicit and non-executing;
- neutral fixtures prove shared machinery and remain non-publishable.

### Change before implementation continues

1. Replace the implicit “edit Gosling to add a renderer” model with an explicit consumer topology.
2. Replace lifecycle-only preload with a reviewed application-runtime contract for safe session and domain operations.
3. Resolve the dead in-process `DomainAdapter` seam through an ADR and executable out-of-process conformance path.
4. Make platform metadata/resources least-privilege and profile/contract derived.
5. Treat current CI and omitted Gate 4 failure paths as open prerequisites.

### Preferred consumer topology to test in the architecture gate

The preferred design is a copy-free composition model:

- **Gosling owns:** versioned Rust/ACP contracts, Electron host/main/preload, shared React shell kit, build verifier, and conformance harness.
- **A project-shell repository owns:** product profile, provisioning references, renderer/domain UI, out-of-process domain adapter or MCP extension, assets, and domain tests.
- **Composition owns:** an immutable Gosling core/host version plus a versioned consumer manifest. It does not permit arbitrary main/preload replacement.
- **Distribution owns:** profile-derived package metadata and a guarded workflow. It never compiles domain code into Gosling merely to satisfy the old Rust trait.

This is a recommendation, not an accepted implementation contract. Gate R1 must compare it against an in-repository workspace-package model and document why one is selected. Rust dynamic-library plugins and copying `ui/desktop/src/main.ts` are rejected defaults.

## Readiness statement

At the assessed revision:

- **Host substrate:** partially ready and locally well tested.
- **Project-shell consumer API:** not ready.
- **Domain integration:** not wired.
- **Shared application UI:** not implemented.
- **Packaged end-to-end project workflow:** not demonstrated.
- **Cross-platform/release infrastructure:** not implemented.
- **Current main CI:** red on Linux helper self-test.

No DAWES, math, or other project shell should begin implementation against this foundation until the new plan reaches its project-shell consumer-ready milestone. Requirements discovery for those products may proceed separately, but it must not drive domain code or special cases into the shared host.
