# Gate 6 security, concurrency, and LLM-boundary audit

Skills: `audit-security` v3.0, `audit-dataflow-concurrency` v3.1, and
`audit-security-llm` v0.3
Authority: read-only audit; the Gate 6 inventory was closed before the governed build workflow
applied repairs
Scope: the complete Workspaces feature, its Config/session/ACP/Electron/SDK seams, and the direct
workspace-to-agent context path
Date: 2026-07-18

## Result

Five findings were confirmed or source-evidenced as likely. All five are repaired and covered by
focused regressions:

| ID          | Domain      | Severity | Confidence | Disposition |
| ----------- | ----------- | -------- | ---------- | ----------- |
| SEC-GOS-001 | Security    | High     | Confirmed  | fixed       |
| SEC-GOS-002 | Security    | Medium   | Confirmed  | fixed       |
| CON-GOS-001 | Concurrency | High     | Confirmed  | fixed       |
| LLM-GOS-001 | LLM         | Medium   | Likely     | fixed       |
| LLM-GOS-002 | LLM         | Medium   | Confirmed  | fixed       |

No raw workspace credential is intentionally placed in workspace/session persistence, exports,
renderer profile responses, agent context, or logs. Credential authority remains in Config's OS
keyring/protected fallback and is resolved through the session-pinned profile reference.

## Classic security trust-boundary inventory

| Surface | Actor | Authority | Object/data reached | Enforcement location | Bypass review |
| ------- | ----- | --------- | ------------------- | -------------------- | ------------- |
| Desktop workspace ACP calls | local renderer user | authenticated Desktop ACP connection | workspace/profile metadata and session defaults | ACP token/origin middleware plus backend handlers/service validation | direct ACP calls encounter the same backend guards |
| Workspace import | local user or supplied metadata file | create one local workspace | non-secret workspace store | secret-shaped-field rejection, typed decode, schema/path/size validation | unknown secret keys and `..` paths are rejected before persistence |
| Distribution templates | distribution/config authority | first-launch materialization | non-secret profiles/workspaces | Config source, deny-unknown manifest, placeholder allowlist, provider/schema validation | templates cannot contain raw secret-shaped fields or unknown placeholders |
| Credential form | local renderer user | create/update/delete one secure profile | metadata store plus Config secure storage | declared provider-key validation, namespaced identifiers, dependency check, cross-store journal/locks | renderer never receives stored values; direct calls cannot edit global/distribution aliases |
| Workspace/session create | local renderer user | create a session in one selected workspace | session row, effective cwd, profile scope, non-secret context | workspace/profile validation and backend session preparation | active global state cannot replace the explicit/pinned workspace |
| Standalone ACP transport | local or explicitly configured remote client | token-authenticated ACP | all ACP methods, including workspace/profile methods | loopback defaults, token middleware, origin policy; TLS default in `gosling-server` | unauthenticated mode requires an explicit dangerous CLI flag |
| Folder chooser/reveal/create | local renderer user | selected local path | filesystem metadata or explicitly requested directory creation | Electron chooser plus backend absolute/native/traversal checks and `create_if_missing` | no workspace operation deletes, moves, or recursively rewrites selected content |
| Agent context | imported/template/user metadata | model-visible data only | workspace name and non-secret folders/outputs | bounded typed snapshot, data labeling, existing tool permission system | credential/profile metadata is excluded; prompt text is not treated as an authorization boundary |

## Concurrency inventory

| State/artifact | Writers | Readers | Transaction/idempotency | Lock/constraint | Race disposition |
| -------------- | ------- | ------- | ----------------------- | --------------- | ---------------- |
| `workspaces.json` | workspace/profile/template services | service list/get/prepare | one locked read-modify-validate-atomic-write | `.workspaces.lock`, unique IDs/names | held; two real concurrent writers preserve both effects |
| workspace temp file | every store mutation | startup recovery | write, fsync, rename, directory fsync | store lock and truncate-before-write | held after Gate 4 repair |
| Config secret aggregate | provider/profile secret setters | provider/profile resolution | locked fresh read-modify-atomic-write | per-process mutex plus `.secrets.lock` | CON-GOS-001 fixed stale-cache lost updates |
| profile metadata + secure values | create/update/delete/migration/bootstrap | profile listing and session preparation | metadata-first fail-closed protocol plus pending-deletion journal | local operation mutex plus cross-process credential transaction lock | CON-GOS-001 fixed interleaving/misalignment |
| active workspace | set-active/delete | Desktop contexts/new session | last completed mutation wins | workspace store lock; referenced ID must exist | held; affects future chats only |
| session workspace snapshot | new/fork/import update | resume/header/list | one SQLite update and schema v22 nullable columns | session manager transaction/row identity | held; active-workspace changes never rewrite it |
| Desktop multi-window cache | backend mutations and Electron broadcasts | each `WorkspaceContext` | refetch is idempotent | backend remains source of truth | held; events do not perform a second mutation |
| workspace export | explicit user save | filesystem/user | one non-secret document | Electron save result; no shared canonical export path | no reuse race established |

Lock order for credential-sensitive mutations is local `operation_lock` → cross-process credential
transaction lock → workspace store lock → Config secret lock. No inverse order was found.

## LLM trust-path and agency overlay

| Channel into model | Trust | Reaches context as | Reachable agency | Boundary outside model text |
| ------------------ | ----- | ------------------ | ---------------- | --------------------------- |
| workspace name/folder/output metadata | user, imported file, or distribution config | bounded `WorkspaceSessionContext` JSON explicitly labeled user-configured data | existing session tools/extensions | existing tool approval/policy; effective cwd and profile are resolved in backend code |
| pinned workspace context on resume | same session snapshot | same bounded JSON | existing session tools/extensions | persisted workspace/profile references; no adoption of active workspace |
| credential profile | secure local operator data | never placed in model context | provider authentication only | task-local Config resolution using namespaced secure-storage keys |
| model output | model-controlled | existing chat rendering/tool-call paths | unchanged by Workspaces | existing renderer/tool-permission boundaries; no new Workspaces sink |

Workspace metadata is not a security principal. The structured data marker is defense in depth, not
an authorization control. Tool permissions, backend path validation, session pinning, and secure
profile resolution remain the deterministic controls.

Framework results:

- Side-channel exfiltration: Workspaces adds no auto-fetched model URL, outbound message, or new
  render sink. No credential data enters context.
- RAG, vector storage, training, and fine tuning: no new surface.
- Memory/persistence: the non-secret workspace snapshot is pinned to the session; it is not written
  from model output and cannot cross a local user/tenant boundary introduced by this feature.
- Consumption: counts, individual field lengths, and total serialized workspace size are bounded;
  LLM-GOS-002 is fixed.
- Supply chain: template manifests come from distribution Config authority, materialize once, reject
  secret-shaped/unknown fields, and resolve only explicit path placeholders. Extension/model supply
  chains are unchanged.
- Capability/identity: workspace credentials are session-pinned and task-local. Tool capability
  ownership and approval remain in the existing extension permission system; the context cannot
  grant itself a tool or replace a profile.
- Containment/telemetry: ACP diagnostics now redact credential-shaped payloads. This gate did not
  claim a full incident-response/quarantine redesign for the pre-existing agent platform.

## Detailed findings

### SEC-GOS-001: ACP debug mode logged credential-profile request values

Severity: High  
Confidence: Confirmed  
Evidence basis: source-evidenced and regression-observed  
Domain: Security — Secret Exposure / Provider Secret Leakage

Pre-repair evidence: `ui/sdk/src/http-stream.ts` passed the complete ACP message to
`console.debug`. Credential-profile create/update messages contain `secretFields[].value`, so
enabling `ACP_DEBUG` placed a raw profile value in developer-console/log capture.

Expected boundary: diagnostic mode may name a method or safe metadata but must never reproduce
credential values.

Failure mechanism: a generic transport logger had no structured redaction boundary.

Break-it angle: enable `ACP_DEBUG`, create a profile with a sentinel value, and serialize the
console call.

Impact: a credential could enter screenshots, copied diagnostics, crash capture, or persistent
developer logs.

Repair: `redactAcpDebugPayload` recursively redacts declared `secretFields`, secret-bearing key/value
objects, sensitive property names, assignments, bearer tokens, and provider-token shapes before the
single debug sink.

Validation: `http-stream-redaction.test.ts` asserts both direct transformation and the actual
`console.debug` call never contain the sentinel.

### SEC-GOS-002: Future-compatible workspace fields could preserve secret-shaped data

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Security — Secret Exposure / Trust Boundary Confusion

Pre-repair evidence: `WorkspaceStoreDocument` deliberately flattened unknown top-level fields and
round-tripped them, but `validate()` did not apply the import/export secret-field rejection to that
map.

Expected boundary: forward-compatible metadata may survive, but raw secret-shaped fields must never
be emitted by a normal workspace-store mutation.

Failure mechanism: compatibility preservation bypassed the otherwise canonical secret-shaped-field
guard.

Break-it angle: add top-level `api_key` with a sentinel, perform any workspace mutation, and inspect
the rewritten store.

Impact: raw secret material from a future/malformed writer could be normalized into durable
workspace persistence.

Repair: store validation now applies `reject_secret_shaped_value` to all flattened unknown fields
before any write while benign future fields continue to round-trip.

Validation: `secret_shaped_unknown_fields_are_never_persisted` and
`unknown_top_level_fields_survive_mutation` cover both sides of the boundary.

### CON-GOS-001: Concurrent profile writes could lose or misalign secure state

Severity: High  
Confidence: Confirmed  
Evidence basis: source-evidenced and concurrent-test-observed  
Domain: Concurrency — Lost Update / Stale Read / Missing Transaction Boundary

Pre-repair evidence: Config secret mutation used a per-instance mutex and a cached aggregate
read-modify-write. Two Config instances/processes could each read the same old aggregate and the
later write would discard the earlier key. Workspace profile metadata and secure writes also had
only a per-`WorkspaceService` mutex, permitting another backend process to interleave the two stores.

Expected boundary: concurrent provider/profile changes preserve disjoint keys, and one profile
operation's metadata and secure values cannot be reordered around another profile operation.

Failure mechanism: stale cached reads plus process-local locks around shared keyring/file and
workspace artifacts.

Break-it angle: preload two Config caches, release two writers together, and assert both final keys;
or interleave two updates to one profile between its metadata and secure writes.

Impact: unrelated credentials can disappear, or configured-field metadata can disagree with the
actual secure value, causing incorrect authentication or relink state.

Repair: Config now obtains an owner-only cross-process secret transaction lock, invalidates its
cache under that lock, reads fresh state, and atomically writes. Credential-sensitive workspace
operations hold a separate cross-process transaction lock over metadata plus secure-store steps.
The existing pending-deletion journal remains the crash-recovery protocol.

Validation: `secret_mutations_across_config_instances_do_not_drop_updates` uses a real barrier and
two Config instances; `concurrent_store_mutations_preserve_both_updates` uses two concurrent store
writers and asserts both final effects.

### LLM-GOS-001: Workspace metadata entered the system context as instruction-like prose

Severity: Medium  
Confidence: Likely  
Evidence basis: source-evidenced source-to-context-to-model path; no unauthorized tool execution was
run  
Domain: LLM — Indirect Prompt Injection / Trust Boundary Confusion

Pre-repair evidence: imported/template-controlled workspace names, labels, and paths were directly
formatted as Markdown lines and attached through `extend_system_prompt`.

Expected boundary: required workspace metadata must be clearly represented as untrusted data; it
must not look like adjacent developer instructions, and it must not grant authority.

Failure mechanism: data and operational guidance shared an undifferentiated prose block.

Break-it angle: import a workspace whose label resembles a tool-use instruction, start a session,
and inspect the resulting context/tool decision. The audit did not call a live model or third-party
tool.

Impact: supplied metadata can steer model behavior even though it cannot independently grant a tool,
change the backend cwd, or replace the session credential.

Repair: render the typed context as JSON inside an explicit user-configured-data boundary, with
operational guidance outside the data. Existing code-enforced tool permissions and backend session
resolution remain the authority boundary.

Validation: `rendered_context_contains_no_credential_metadata` asserts the structured marker and
absence of credential identifiers. Residual model steerability is documented; prompt labels are not
claimed as an authorization control.

### LLM-GOS-002: Workspace metadata could exhaust the session context

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: LLM — Unbounded Consumption

Pre-repair evidence: workspace mutations had no aggregate limits on additional folders, outputs,
bindings, labels, paths, or serialized size, and every folder/output was copied into the model
context on session creation and resume.

Expected boundary: local/imported/template metadata has deterministic resource bounds before it
enters durable storage or a context window.

Failure mechanism: unbounded arrays and strings crossed persistence into system context.

Break-it angle: import tens of thousands of folders or a smaller collection of multi-kilobyte paths,
then start/resume a session.

Impact: context-window exhaustion, elevated token cost, session startup failure, or degraded model
behavior.

Repair: enforce per-field length limits, collection count limits, and a 64 KiB total serialized
workspace boundary for create, update, import, templates, and store reload validation.

Validation: `workspace_boundary_limits_model_context_size` covers both over-count and over-size
payloads.

## Required inventory dispositions

### SEC-001 through SEC-015

| Check | Disposition | Evidence |
| ----- | ----------- | -------- |
| SEC-001 Missing Authentication | Held | Desktop/standalone workspace methods share ACP token middleware; unauthenticated standalone mode is explicit and dangerous. |
| SEC-002 Missing Authorization | Held | local single-user metadata authority; profile sources and destructive dependencies are backend-enforced. |
| SEC-003 Object Scope / IDOR | Not applicable | Workspaces introduces no tenant/user ownership model; IDs address the same local user's store. |
| SEC-004 Privilege Escalation | Held | workspace context cannot grant tools or replace the pinned profile; profile sources restrict edits. |
| SEC-005 Trust Boundary Confusion | Finding | SEC-GOS-002 and LLM-GOS-001. |
| SEC-006 Injection | Held after repair | typed JSON/path parsing; no workspace value reaches SQL, shell, eval, or template execution. Model-specific path is LLM-GOS-001. |
| SEC-007 Secret Exposure | Findings | SEC-GOS-001 and SEC-GOS-002. |
| SEC-008 Unsafe Path/File Access | Held | absolute/native/traversal checks; explicit directory creation only; no delete/move operation. |
| SEC-009 Unsafe Deployment Default | Held | Desktop binds loopback with token; standalone server defaults are loopback and TLS. |
| SEC-010 Sensitive Route Exposure | Held | credential methods are on the authenticated ACP router and return metadata only. |
| SEC-011 Overbroad Permission | Held | per-profile secure keys and per-session scope; no global secret swap. |
| SEC-012 Unsafe External Tool Invocation | Not applicable to delta | no workspace code spawns a shell or provider CLI. |
| SEC-013 Boundary Only In UI | Held | only/default deletion, credential source/dependency, folder, path, and secret-field guards are backend-side. |
| SEC-014 Reverse Proxy Assumption | Held | token enforcement is runtime behavior; no proxy is required for Desktop's loopback backend. |
| SEC-015 Provider/Environment Leakage | Finding then held | SEC-GOS-001 fixed; profile secrets remain keyring/file-backed and task-local rather than copied to global env/settings. |

### CON-001 through CON-018

| Check | Disposition | Evidence |
| ----- | ----------- | -------- |
| CON-001 Race Condition | Finding | CON-GOS-001. |
| CON-002 Lost Update | Finding | stale aggregate secure-store mutation fixed by cross-process fresh-read transaction. |
| CON-003 Double Processing | Held | UUID/name invariants reject duplicate canonical workspace/profile creation; intentional Duplicate actions create distinct copies. |
| CON-004 Replay Hazard | Held | replayed import/create name conflicts rather than overwriting; set-active is idempotent. |
| CON-005 Retry Collision | Held after repair | store and secure writes serialize; pending deletion cleanup is idempotent. |
| CON-006 Stale Read | Finding | Config cache refresh now occurs under the secure transaction lock. |
| CON-007 Stale Write | Finding | CON-GOS-001. |
| CON-008 Ordering Dependency | Finding | cross-store profile order is now serialized and crash-recovered. |
| CON-009 Partial Commit | Held after repair | metadata-first fail-closed protocol and deletion journal from Gate 4; Gate 6 prevents concurrent interleaving. |
| CON-010 Missing Transaction Boundary | Finding | CON-GOS-001. |
| CON-011 Lock Inversion | Held | one documented acquisition order; no inverse path found. |
| CON-012 Shared Mutable State | Held after repair | local mutexes plus file locks protect shared stores; backend remains source of truth. |
| CON-013 Non-Atomic File Output | Held | temp write, fsync, atomic rename, directory fsync, restrictive permissions. |
| CON-014 Duplicate Canonical Creation | Held | read/write uniqueness invariants and generated UUID identities. |
| CON-015 Check-Then-Act | Held after repair | profile dependency check and mutation share the credential transaction lock. |
| CON-016 Concurrent Bulk Scope Drift | Not applicable | no workspace bulk operation exists. |
| CON-017 Artifact Reuse Race | Held | workspace temp protected and truncated; Config secure temp protected by transaction lock. |
| CON-018 Watcher/Event Reentrancy | Held | Electron broadcasts trigger idempotent refetches, not new mutations. |

### LLM-001 through LLM-014

| Check | Disposition | Evidence |
| ----- | ----------- | -------- |
| LLM-001 Direct Prompt Injection | No new finding | direct user prompts are an existing agent input; Workspaces adds no authority based on prompt text. |
| LLM-002 Indirect Prompt Injection | Finding | LLM-GOS-001. |
| LLM-003 Improper Output Handling | No new finding | no model output is used by new workspace persistence, path, or renderer sinks. |
| LLM-004 Excessive Agency | Held for delta | tools are unchanged and retain code-enforced permission handling; context cannot grant a capability. |
| LLM-005 Tool/MCP Confused Deputy | Held | credential/profile identity is resolved in backend code and omitted from prompt context. |
| LLM-006 Side-channel Exfiltration | No new finding | no workspace-specific URL rendering/fetch or outbound sink; credentials never enter context. |
| LLM-007 Context-window Data Leak | Held | only non-secret workspace/folder/output data is included and tested. |
| LLM-008 RAG/Vector-store Weakness | Not applicable | no RAG/vector ingestion is added. |
| LLM-009 Training/Fine-tune Poisoning | Not applicable | no training pipeline is added. |
| LLM-010 Memory/Persistence Poisoning | Held for delta | workspace snapshot is user/config data, not model write-back, and is session-pinned. |
| LLM-011 AI Worm | No new finding | no automated workspace-context propagation to another user/agent. |
| LLM-012 Unbounded Consumption | Finding | LLM-GOS-002. |
| LLM-013 Model/Tool/Plugin Supply Chain | Held for delta | distribution templates are deployment-authority Config, validated and materialized once; tool/model supply chains are unchanged. |
| LLM-014 Manipulated Output Integrity | No new finding | Workspaces adds no automated downstream consumer of model output. |

## Break-it review

- Direct API with UI controls removed: backend refuses default/only workspace deletion, invalid
  output defaults, undeclared secret fields, global/distribution profile edits, and non-native or
  traversal paths.
- Secret sentinel: rejected from imports/unknown store fields, absent from export/session context,
  redacted from renderer errors and ACP debug output, and never returned after secure storage.
- Two concurrent store writers: both final effects survive.
- Two Config instances with preloaded stale caches: both secure keys survive after simultaneous
  mutation.
- Interrupted profile deletion: pending namespaced deletion identifiers remain for startup cleanup;
  raw values are not journaled.
- Workspace switch while active generation/session is visible: only future defaults change; no
  cancellation or session rewrite path exists.
- Hostile/unbounded metadata: structured as data and rejected above collection/field/64 KiB limits.

## Validation limits

- No live provider, real OS keyring, or third-party model/tool was invoked. File-backed secure-store
  and source-level keyring paths were tested.
- Concurrency regressions use real simultaneous threads and independent store/Config instances;
  a destructive multi-process kill/fault-injection drill was not run.
- The LLM audit is scoped to the new Workspaces ingress and direct agency seams, not a claim that the
  entire pre-existing agent/MCP/rendering platform was independently re-certified.
- Prompt data labeling reduces data/instruction ambiguity but is not treated as a security boundary;
  deterministic tool and backend controls remain required.

Stop condition: all SEC-001–015, CON-001–018, and LLM-001–014 items have a finding, held
non-finding, or explicit not-applicable disposition; all five Gate 6 findings are repaired and have
regression evidence.
