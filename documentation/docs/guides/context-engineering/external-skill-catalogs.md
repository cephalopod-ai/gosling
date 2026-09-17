---
title: External Skill Catalogs
sidebar_position: 4
---

gosling can load a compiled skill catalog that remains outside the gosling
repository. This supports private, organizational, or independently versioned
skill ecosystems without bundling their descriptors, instructions, names, or
routing policies into gosling releases.

## Publication boundary

gosling publishes only:

- the external catalog JSON contract;
- generic loading and routing code;
- configuration and validation requirements.

The catalog owner retains the catalog index, `SKILL.md` files, supporting files,
source descriptors, taxonomy, and build tooling. gosling reads those files at
runtime from an explicitly configured local path and does not copy them into its
source tree.

The schema is available at
[`/schemas/skill-catalog-v1.schema.json`](/schemas/skill-catalog-v1.schema.json).

## Configuration

Add one or more compiled catalog index files to user-local `config.yaml`:

```yaml
GOSLING_SKILL_CATALOGS:
  - "/path/to/private-catalog/gosling-skill-catalog.json"
```

Or set a JSON array through the environment:

```bash
export GOSLING_SKILL_CATALOGS='["/path/to/private-catalog/gosling-skill-catalog.json"]'
```

Do not commit a private catalog path or generated private index to the gosling
repository. Configuration belongs in the user's normal gosling configuration.

## Contract

Each catalog entry points to a relative directory beneath the index file. That
directory must contain a `SKILL.md`. Catalog discovery indexes descriptor
metadata without reading skill instructions or walking supporting files. The
selected `SKILL.md` is parsed on demand, and its name must match the catalog ID.
Absolute directories, parent path components, duplicate IDs, unknown route
targets, and paths resolving outside the catalog root are rejected.

```json
{
  "schemaVersion": 1,
  "catalogId": "example-private-catalog",
  "skills": [
    {
      "id": "plan-example-workflow",
      "version": "1.0",
      "summary": "Plan a synthetic workflow.",
      "directory": "skills/plan-example-workflow",
      "routing": {
        "actions": ["plan"],
        "roles": ["planner"],
        "surface": "example",
        "targets": ["workflow"],
        "keywords": ["synthetic", "planning"],
        "aliases": [],
        "excludes": []
      },
      "execution": {
        "authority": "plan-only",
        "requiresHumanApprovalFor": ["target-changes"],
        "criticRequired": false,
        "overlaysAllowed": true
      },
      "deprecated": false
    }
  ],
  "routes": []
}
```

Routing fields are required so selection can happen locally and deterministically
without placing every skill description in the model prompt. Routes are optional
and support catalog-authored disambiguation that should outrank generic keyword
scoring. Every route must include a positive action, role, surface, target, or
keyword condition; `notKeywords` can narrow a route but cannot define one alone.

## Admission and authority

Finding a skill in a catalog does not give it any authority. Gosling records an
**admission** only when a skill is loaded for use through `load_skill` or a skill
slash command. The admission names the configured catalog, the declared version,
and the SHA-256 of the exact `SKILL.md` bytes that were loaded. A short
**Host Admission** section at the top of the loaded skill shows this record.

- `contentHash` is optional. When it has the form `sha256:<64 lowercase hex>`,
  Gosling compares it with the loaded `SKILL.md` and refuses a skill that no
  longer matches. Other forms are recorded as unverifiable and not interpreted.
- If a project, user, or plugin skill uses the same id as a configured catalog
  entry, Gosling refuses to load either one under that id. Rename one of them.
- `execution.authority` can restrict the current turn but never grants a
  permission:

| Declared authority | Effect in Gosling |
| --- | --- |
| absent | No change; existing permissions apply |
| `read_only`, `plan_only` | Tool calls other than Gosling's verified read-only tools need your approval for each call, even in Autonomous mode |
| `destructive_admin` | Same per-call approval requirement |
| `test_only`, `low_risk_repair`, `governed_repair` | No added restriction; Gosling cannot verify these scopes for shell or editor tools |
| any other value | Treated like `read_only` |

The verified read-only tools are skill search and loading, Session History
search and read, and the developer `tree` and `read_image` tools. A restriction
lasts until the turn ends, applies to subagents the turn delegates to, and cannot
be saved as "Always allow". Code Mode cannot run other tools while it applies.
`requiresHumanApprovalFor` terms are shown in the admission section but not
enforced, because Gosling has no reliable mapping from those terms to tool calls.

If the current provider runs its own tools outside Gosling, a restricted skill
cannot be started with a slash command, because Gosling could not enforce the
restriction.

## Compatibility

External catalogs complement standard Agent Skills. gosling continues to discover
ordinary `SKILL.md` files from its normal project, user, compatibility, and plugin
locations. Entries from all adapters are normalized into the same runtime skill
set; the first discovered ID retains precedence for listing, but a local skill that
shadows a configured catalog ID is not admitted.

Catalog entries are read-only through gosling's source-management API. Modify or
regenerate them in the owning catalog repository, then use `refresh_skills` or
start a new session.

## Privacy considerations

Keeping a catalog external prevents it from being published with gosling. It does
not prevent selected skill instructions from being sent to the configured model
provider when the skill is loaded. Catalog owners should apply the same provider,
logging, and secret-handling policies they use for other prompt context. Skill
files should never contain credentials.
