---
title: External Skill Catalogs
sidebar_position: 4
---

Gosling can load a compiled skill catalog that remains outside the Gosling
repository. This supports private, organizational, or independently versioned
skill ecosystems without bundling their descriptors, instructions, names, or
routing policies into Gosling releases.

## Publication boundary

Gosling publishes only:

- the external catalog JSON contract;
- generic loading and routing code;
- configuration and validation requirements.

The catalog owner retains the catalog index, `SKILL.md` files, supporting files,
source descriptors, taxonomy, and build tooling. Gosling reads those files at
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

Do not commit a private catalog path or generated private index to the Gosling
repository. Configuration belongs in the user's normal Gosling configuration.

## Contract

Each catalog entry points to a relative directory beneath the index file. That
directory must contain a standard `SKILL.md`. Absolute directories, parent path
components, duplicate IDs, unknown route targets, and paths resolving outside
the catalog root are rejected.

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
scoring.

## Compatibility

External catalogs complement standard Agent Skills. Gosling continues to discover
ordinary `SKILL.md` files from its normal project, user, compatibility, and plugin
locations. Entries from all adapters are normalized into the same runtime skill
set; the first discovered ID retains precedence.

Catalog entries are read-only through Gosling's source-management API. Modify or
regenerate them in the owning catalog repository, then use `refresh_skills` or
start a new session.

## Privacy considerations

Keeping a catalog external prevents it from being published with Gosling. It does
not prevent selected skill instructions from being sent to the configured model
provider when the skill is loaded. Catalog owners should apply the same provider,
logging, and secret-handling policies they use for other prompt context. Skill
files should never contain credentials.
