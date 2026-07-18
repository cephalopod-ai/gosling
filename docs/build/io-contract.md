# I/O Contract — Gosling Desktop Workspaces

Format schema version: 1. Session database migration target: 22.

## Typed ACP inputs and outputs

| Operation family | Input | Output | Boundary validation | Error classes |
|---|---|---|---|---|
| list/get | empty or workspace UUID | canonical workspace(s), active/default IDs, validation reports | UUID + store version/load | not_found, storage |
| create/update/duplicate | canonical mutation payload or source UUID | canonical workspace + validation | names, IDs, enums, paths, bindings, default uniqueness | validation, conflict, storage |
| delete/set active | workspace UUID + explicit confirmation where required | updated state | existence, only/default invariant | not_found, conflict |
| validate | workspace UUID or mutation payload | per-folder/profile issues with stable codes/severity | backend path/profile checks | validation, unavailable |
| export/import | UUID or UTF-8 JSON document | safe JSON or canonical workspace | schema version, secret-shaped fields, paths, new IDs | validation, unsupported_version |
| profile list/create/update | metadata plus write-only field updates | metadata/status only | provider registry field census; secret/non-secret classification | validation, unsupported_auth, credential |
| profile delete/usage/test | UUID, confirmation | metadata status/reference list/test result | references, secure fields, provider support | conflict, credential, not_found |

Secret-field update values appear only in the incoming request object long enough to call
`Config::set_secret_values`; response serializers have no value field. Custom dispatch logging
records neither parameters nor request bodies.

## Workspace persistence

Location: `<Gosling data dir>/workspaces/workspaces.json`

```json
{
  "schema_version": 1,
  "active_workspace_id": "uuid",
  "default_workspace_id": "uuid",
  "migration_completed": true,
  "workspaces": [],
  "credential_profiles": []
}
```

Workspaces contain the canonical fields from `WorkspaceDto`. Credential profiles contain
metadata, configured logical field names, non-secret configuration fields, timestamps, and a
source kind (`workspace_secure_storage` or `global_configuration_alias`). They never contain
raw secret values or physical keyring identifiers.

| Data | Format/version | Overwrite | Recovery |
|---|---|---|---|
| workspace store | JSON envelope v1 | lock, private temp, fsync, atomic rename | if main exists it wins; if absent and temp parses at supported version, promote temp; otherwise report and preserve |
| profile secret field | existing Config secure store | batched atomic secure write | existing keyring/protected-fallback semantics |
| active selection | same JSON envelope | same atomic transaction as workspace mutation | invalid/missing active ID is repaired to Default and persisted |

On Unix, workspace directory is `0700` and JSON/lock/temp files are `0600`. Malformed main
data is never silently replaced; the error names the metadata path and safe parse category.

## Session persistence v22

Nullable columns added to `sessions`:

- `workspace_id TEXT`
- `workspace_name TEXT`
- `credential_profile_id TEXT`
- `credential_profile_name TEXT`
- `credential_binding_id TEXT`
- `workspace_context_json TEXT`

Fresh schema and v21→v22 migration must match. `workspace_context_json` serializes only the
non-secret session context DTO. Copy/fork copies all fields. Workspace/profile deletion does
not update or delete these columns.

## Path rules

- Interactive selections and imports resolve to absolute platform paths before persistence.
- `.` and `..` are collapsed lexically; a result that escapes its allowed template/base is rejected.
- Existing paths are canonicalized to resolve symlinks before directory/access/containment checks.
- Missing primary/non-directory paths are blocking; missing optional paths are warnings.
- `create_if_missing` does not authorize creation by itself; a separate confirmed create action does.
- Import/template placeholders are allowlisted (`{home}` and distribution-supplied absolute base),
  not arbitrary environment expansion or shell syntax.
- Validation/removal never deletes or moves the selected path.

## Malformed inputs

| Input | Malformation | Deterministic behavior |
|---|---|---|
| store | invalid JSON/current schema | safe storage error; original unchanged |
| store | future schema version | unsupported-version error; no write |
| workspace | blank/duplicate-insensitive name | validation issue; no mutation |
| binding | missing profile/target/default | validation/credential issue; no activation |
| folder | relative/traversal/non-directory primary | path issue; session blocked |
| output | empty output list, empty product types, or zero/multiple defaults | validation issue |
| import | `secret`, token/password/key value fields, physical key identifiers | reject the entire import |
| profile update | unknown/provider-mismatched field | reject before secure write |
| new session | unknown workspace ID | not-found; no session row remains |
| resume | pinned missing profile | relink-required; no global fallback |

## Export/import guarantees

Export is deterministic apart from the explicit export timestamp if present: arrays are sorted
by stable display/ID order and no secret-bearing or internal secure-key field exists in the
schema. Import always creates new workspace/folder/binding IDs unless an explicit future merge
contract is versioned. Credential bindings to unavailable profile names are imported as
unprovisioned metadata references, never configured status.
