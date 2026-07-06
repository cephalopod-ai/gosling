# Fresh comprehensive audit - 2026-07-06

Scope: full repository at `81a5513919e996b110f87f1b26f93e47bb85ae27` on branch `main`.

Ground rules: this audit was run fresh from current source, manifests, workflows, and repo instructions. Prior session logs and prior audit/repair reports were intentionally excluded as audit inputs.

## Applied skills

Applicable audit lenses used: `audit-architecture-nodejs`, `audit-architecture-seam`, `audit-compliance-posture`, `audit-contract-crossrepo`, `audit-contract-internalapi`, `audit-dataflow-cascade`, `audit-dataflow-concurrency`, `audit-dataflow-input-output`, `audit-dataflow-integrity`, `audit-dataflow-pipeline-graph`, `audit-dataflow-state-transition`, `audit-dataflow-temporal`, `audit-deadcode-cleanup`, `audit-dependency-criticality`, `audit-design-webapp`, `audit-equation-sourcebase`, `audit-failsafe-readiness`, `audit-invariant-sync`, `audit-memory-lifecycle`, `audit-multiagent-consensus`, `audit-negative-space`, `audit-operator-signal`, `audit-performance-profile`, `audit-pipeline-externalapi`, `audit-playtest-app`, `audit-recovery-idempotency`, `audit-reliability`, `audit-resource-lifecycle`, `audit-security`, `audit-security-code`, `audit-security-llm`, `audit-security-nodejs`, `audit-security-repo-posture`, `audit-security-repo-triage`, `audit-security-vuln-harness`, and `audit-workflow-gui`.

Non-applicable lenses: Supabase-specific and Flutter/iOS-specific lenses, because this repository does not expose those surfaces in the scanned scope.

## Skill escalation table

| Area | Primary lenses | Escalated to | Result |
| --- | --- | --- | --- |
| MCP app proxy CSP and iframe isolation | `audit-security`, `audit-dataflow-input-output`, `audit-contract-internalapi`, `audit-security-nodejs` | `audit-security-code`, `audit-security-vuln-harness` | Confirmed F-001 |
| Tool inspection and LLM command mediation | `audit-security-llm`, `audit-dataflow-pipeline-graph`, `audit-failsafe-readiness` | `audit-invariant-sync`, `audit-negative-space` | Confirmed F-002 |
| CI secrets and workflow supply chain | `audit-security-repo-posture`, `audit-compliance-posture`, `audit-dependency-criticality` | `audit-recovery-idempotency` | Confirmed F-003 and F-004 |
| Desktop renderer/main IPC and generated API boundary | `audit-design-webapp`, `audit-workflow-gui`, `audit-contract-crossrepo` | `audit-security-code` | No confirmed defect |
| Rust resource, memory, and concurrency lifecycle | `audit-resource-lifecycle`, `audit-memory-lifecycle`, `audit-dataflow-concurrency`, `audit-reliability` | `audit-recovery-idempotency` | Confirmed F-005 during repair verification |

## Findings

### F-001: Legacy MCP app proxy accepts raw CSP source tokens

Severity: High. Confidence: High. Repair group: G1.

Evidence:

- `crates/gosling-server/src/routes/mcp_app_proxy.rs:122` parses comma-separated domains by trimming and retaining every non-empty token.
- `crates/gosling-server/src/routes/mcp_app_proxy.rs:67` through `:119` directly joins those tokens into `script-src`, `style-src`, `connect-src`, `frame-src`, `img-src`, `font-src`, `media-src`, and `base-uri`.
- `crates/gosling-server/src/routes/mcp_app_proxy.rs:178` through `:181` injects the resulting policy into the HTML template.
- `crates/gosling-server/src/routes/templates/mcp_app_proxy.html:10` through `:12` places that value in a CSP meta tag attribute.
- `crates/gosling-server/src/routes/mcp_app_proxy.rs:214` through `:220` reuses the same raw parser for the guest CSP header path; the header parse at `:276` through `:279` happens after policy construction and does not protect the proxy meta path.

Impact: hostile MCP app metadata or crafted proxy query parameters can add CSP keywords such as wildcard sources or unsafe inline tokens, and metacharacters can break the proxy meta attribute. That weakens the sandbox ceiling around untrusted MCP app UI and can bypass the intended CSP restrictions.

### F-002: Egress inspector skips namespaced run-command tools

Severity: Medium. Confidence: High. Repair group: G2.

Evidence:

- `crates/gosling/src/security/egress_inspector.rs:275` through `:282` recognizes bare `execute_command`/`run_command` names and a few namespaced shell suffixes, but not `__execute_command` or `__run_command`.
- `crates/gosling/src/security/egress_inspector.rs:333` through `:336` skips inspection when a tool name is not classified as shell or web.
- `crates/gosling/src/security/scanner.rs:394` through `:402` shows the adjacent prompt-injection scanner already treats those namespaced command suffixes as shell tools.
- `crates/gosling/src/agents/agent.rs:625` through `:629` registers `EgressInspector` in the main tool inspection pipeline.

Impact: a namespaced MCP/developer command tool such as `developer__run_command` can carry outbound upload commands without egress classification, even though equivalent bare names are inspected.

### F-003: CLI docs workflow logs an API key prefix

Severity: Medium. Confidence: High. Repair group: G3.

Evidence:

- `.github/workflows/docs-update-cli-ref.yml:171` through `:177` exposes `ANTHROPIC_API_KEY` to the AI synthesis step and logs `${ANTHROPIC_API_KEY:0:8}`.

Impact: GitHub secret masking does not reliably mask derived substrings. The workflow can leak a stable API key prefix into CI logs, which is enough to aid secret inventory correlation and should not be logged.

### F-004: Several GitHub Actions still use mutable tags

Severity: Medium. Confidence: High. Repair group: G4.

Evidence:

- `.github/workflows/scorecard.yml:76` uses `github/codeql-action/upload-sarif@v4`.
- `.github/workflows/bundle-desktop-windows.yml:87` uses `Jimver/cuda-toolkit@v0.2.35`.
- `.github/workflows/pr-smoke-test.yml:99`, `:133`, and `:219` use `actions/setup-node@v6` and `actions/setup-python@v6`.
- `.github/workflows/pr-comment-build-cli.yml:132` uses `peter-evans/create-or-update-comment@v5`.
- `.github/workflows/autoclose:13` uses `actions/stale@v9`.
- `.github/workflows/cargo-deny.yml:25` uses `actions/checkout@v7`.

Impact: mutable action tags leave CI behavior dependent on upstream tag integrity. This conflicts with the surrounding repo convention, where most actions are pinned to commit SHAs with version comments.

### F-005: Plugin discovery code and tests do not compile

Severity: High. Confidence: High. Repair group: G5.

Evidence:

- Repair verification with `cargo test -p gosling-server mcp_app_proxy` failed compiling `crates/gosling/src/plugins/discovery.rs:131` because an inner boolean `enabled` binding shadowed the outer `Vec` named `enabled`, then `.push(plugin)` was called on the boolean.
- Follow-up verification with `cargo test -p gosling egress_inspector` failed compiling stale discovery test fixtures at `crates/gosling/src/plugins/discovery.rs:442` and `:466` because `PluginConfigEntry` initializers omitted the newer `trusted` field.
- The same file documents the intended current rule at `crates/gosling/src/plugins/discovery.rs:90` through `:92`: user plugins stay enabled for compatibility, while project plugins require explicit trust.

Impact: the affected packages could not pass targeted test compilation, blocking repair verification and any CI job that compiles these targets.

## Non-findings

- Desktop MCP app proxy URL construction validates local ACP origins before converting WebSocket ACP URLs to HTTP proxy URLs (`ui/desktop/src/main.ts:271` through `:310`), and the ACP-side route already normalizes CSP sources.
- The legacy server guest iframe omits `allow-same-origin` (`crates/gosling-server/src/routes/templates/mcp_app_proxy.html:90` through `:102`), preserving the intended opaque-origin sandboxing for same-origin guest HTML.
- No forbidden generated OpenAPI imports were found under `ui/desktop/src`.
- No Goose compatibility links or normalization scripts were changed or required for the confirmed defects.
- The report artifacts from previous work were not used as evidence sources.

## Repair inventory

| Group | Findings | Files |
| --- | --- | --- |
| G1 | F-001 | `crates/gosling-server/src/routes/mcp_app_proxy.rs` |
| G2 | F-002 | `crates/gosling/src/security/egress_inspector.rs` |
| G3 | F-003 | `.github/workflows/docs-update-cli-ref.yml` |
| G4 | F-004 | `.github/workflows/scorecard.yml`, `.github/workflows/bundle-desktop-windows.yml`, `.github/workflows/pr-smoke-test.yml`, `.github/workflows/pr-comment-build-cli.yml`, `.github/workflows/autoclose`, `.github/workflows/cargo-deny.yml` |
| G5 | F-005 | `crates/gosling/src/plugins/discovery.rs` |

## Validation limits

This was a static source and workflow audit plus targeted code-path review. Repair verification added F-005 after the initial report draft. It did not run full desktop UI playtests before repair, and it did not consume prior session or audit logs.
