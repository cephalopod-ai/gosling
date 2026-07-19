# PC-002 workspace-launch workflow and contract audit

Authority: scoped Gate 6/8 audit under `plan-prototype-build` v3.0.1

Scope: canonical workspace defaults, provider-inventory capability projection, new-session model
materialization, Electron home-directory runtime configuration, workspace editor async workflows,
additional working-folder selection, and Hub workspace selection/submission.

## Boundary inventory

| Boundary | Source | Validation / ownership | Effect |
| --- | --- | --- | --- |
| Provider/model/effort | backend provider inventory | generated SDK contract; model-compatible effort list | non-secret workspace metadata |
| Workspace session defaults | backend workspace store | existing versioned/atomic store and `prepare_session` validation | pinned session `ModelConfig` |
| Home directory | Electron main `os.homedir()` | runtime-only app config; platform-owned absolute path | new draft defaults to `<home>/Work` |
| Additional working folders | existing directory chooser | recent-directory grant plus backend workspace validation | metadata reference only; no directory mutation |
| New-chat workspace | backend workspace list | unavailable rows disabled; backend revalidates on session creation | explicit workspace ID and primary folder passed to ACP |

## Finding and repair

### WFG-GOS-009: A successful model request could erase a provider-list failure

Severity: Low
Confidence: Confirmed
Disposition: fixed

The initial implementation shared one error slot between two independent asynchronous inventory
requests. The model request cleared that slot on start, so a provider-list failure could disappear
even though the provider dropdown could not be populated. Provider and model failures now have
separate state and remain independently actionable. A regression forces provider-list rejection
while model loading succeeds and asserts both the error and model option remain visible.

## Audit disposition

| Check | Disposition |
| --- | --- |
| Free-text provider/model injection | closed: both fields are inventory-backed selects; saved legacy IDs remain visible as fallback options |
| Model-incompatible effort | closed: the backend advertises accepted values and the editor normalizes to medium or the first supported value |
| Global provider mutation | closed: workspace effort is applied to the new session `ModelConfig`; no global defaults or secrets are written |
| Active-workspace race | closed: Hub submits its selected workspace ID; backend resolves and pins that ID under the existing session flow |
| Stale async provider/model result | closed: every request owns a cancellation flag and cannot update state after provider/dialog change |
| Missing/unavailable workspace | closed: the option is visibly marked/disabled and backend `prepare_session` remains authoritative |
| Secret exposure | closed: new fields contain provider/model identifiers and an enum only; credential storage and renderer responses are unchanged |
| Existing workspace compatibility | closed: `defaultThinkingEffort` is optional with serde/default-compatible generated types; saved paths/defaults are not migrated or overwritten |
| Physical folder mutation | closed: adding/removing working folders changes references only; output creation retains its separate confirmation path |

## Residual limitations

- The quick-launch prompt has no pre-submit workspace screen and therefore retains the persisted
  active-workspace fallback. Normal New Chat navigation uses the Hub selector.
- Provider inventories can only advertise effort options for models with capability metadata. An
  unrecognized live model remains selectable but uses the provider/app effort default until its
  inventory metadata is enriched.
- Electron folder chooser behavior is covered through renderer tests; no packaged GUI was rebuilt
  or reinstalled by request.

No open PC-002 audit finding remains.
