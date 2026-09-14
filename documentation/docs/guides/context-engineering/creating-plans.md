---
sidebar_position: 2
title: Creating Plans Before Working
sidebar_label: Creating Plans
---

# Creating plans before working

Host-enforced planning gives a session a durable plan lifecycle before implementation begins.
While a plan is open, gosling limits the model to bounded, read-only evidence gathering and plan
updates. It cannot run shell commands, edit files, browse the network, delegate work, or gain
ordinary session tools through an extension.

:::caution Source-candidate feature
Durable host-enforced planning is present in the current source candidate. The CLI and first-party
Desktop paths exist in source, but final packaged and cross-platform Desktop acceptance is still
pending. This page does not claim that the feature has shipped in a published release.
:::

## Before you start: provider compatibility

Planning uses the active session provider, model, thinking setting, and context limit. It does not
start a second planner connection. A provider that executes tools outside gosling cannot enforce
the planning boundary, so gosling rejects planning on that route before it creates or resumes a
plan generation.

Provider and model changes are also blocked while a plan is `drafting` or `awaiting_review`.
Approve or abandon the open generation before changing the session route.

The CLI still reads the legacy `GOSLING_PLANNER_*` variables as compatibility checks:

| Variable | Current behavior |
| --- | --- |
| `GOSLING_PLANNER_PROVIDER` | If set, it must be non-empty and exactly match the active session provider. |
| `GOSLING_PLANNER_MODEL` | If set, it must be non-empty and resolve to the active model and thinking setting. |
| `GOSLING_PLANNER_CONTEXT_LIMIT` | If set, it must be an integer of at least 4,096 and exactly match the active model's context limit. |

Unset these variables unless a compatibility check is required. A different value fails
explicitly; it does not select a separate planner. Desktop and other ACP clients use the active
session route directly.

## What planning may read

The planning policy provides seven fixed host capabilities: list, read, and search bounded
workspace text; update a plan and request review; and search or read bounded persisted session
text. This internal capability set is not a user-configurable extension and does not widen the
session's ordinary tool permissions.

:::warning Read-data disclosure
Workspace or session text returned by a planning tool becomes model input and is sent to the
active session provider. gosling applies path controls, ignore rules, size and time bounds, and
secret redaction. It also rejects hidden, protected, symbolic-link, binary, and oversized file
reads. These controls reduce exposure but cannot guarantee that every sensitive value is removed.
Review the session's primary and additional working directories, keep secrets out of readable
project text, and use ignore rules before starting a plan.
:::

Planning treats retrieved text as untrusted evidence, not instructions or approval. Only an
explicit lifecycle action can approve a revision.

## CLI workflow

Start an interactive session, then use `/plan` with an optional first planning prompt:

```bash
gosling session
```

```text
( O)> /plan Design a reversible database migration with rollback checks
```

The durable planning commands are:

| Command | Effect |
| --- | --- |
| `/plan [prompt]` | Start or resume the current generation and optionally submit one planning prompt. With no prompt, show the current snapshot. |
| `/plan-status` | Show the selected generation, status, revision identity, content hash, and plan content. |
| `/plan-feedback <text>` | Record feedback against the exact revision awaiting review, return it to drafting, and request a revised plan. |
| `/plan-comment <start>-<end> <text>` | Attach line-scoped feedback to the exact revision, return it to drafting, and request a revised plan. |
| `/plan-approve` | Approve the exact current revision without starting implementation. |
| `/plan-approve-and-run` | Approve first, then submit a separate implementation turn using the current authorization mode. |
| `/plan-abandon` | Abandon the current open generation. |
| `/endplan` | Leave planning. If a reviewable revision exists, confirm before abandoning it; non-interactive clients must use explicit `/plan-abandon`. |
| `/plan-export` | Print the exact current revision as Markdown with plan provenance. |

For example, review and revise a plan without starting work:

```text
( O)> /plan-status
( O)> /plan-feedback Keep the rollout reversible and add a restore drill
( O)> /plan-status
( O)> /plan-approve
```

When a revision is ready, the CLI displays the review commands. While the status is
`awaiting_review`, use a lifecycle command rather than another `/plan <prompt>`; feedback is the
explicit route back to drafting.

## Durable states

| State | Meaning |
| --- | --- |
| `drafting` | The generation is open. Planning prompts may gather bounded evidence and update the plan. |
| `awaiting_review` | The generation is open and has an exact revision ready for feedback, approval, export, or abandonment. |
| `approved` | The selected revision was approved. Approval alone does not implement it. |
| `abandoned` | The open generation was deliberately closed without approval. |
| `stale` | The generation is retained as history but cannot be approved as current authority. Imported or copied plan history is made stale. |

Plans, revisions, feedback, events, and decisions are stored with the session. After restarting
gosling and resuming the same session, `/plan-status` recovers the latest snapshot and `/plan`
resumes an open generation. Terminal generations remain reviewable history rather than being
reopened implicitly.

Feedback, approval, and implementation references are checked against the selected generation,
revision, content hash, source history, and workspace scope. If any of those changed, refresh the
snapshot and review again. This prevents a stale screen or command from approving different
content. Markdown export identifies the selected generation, revision, content hash, and status;
it is a read-only export, not a mutating approval check.

## Approval is separate from implementation

`/plan-approve` records approval and stops there. It does not clear or rewrite conversation
history, change `GOSLING_MODE`, or grant new permissions.

`/plan-approve-and-run` first records the same exact approval, then submits a separate ordinary
implementation turn. That turn uses the session's unchanged authorization mode and normal tool
approval behavior. If submission fails after approval, the plan stays approved and gosling reports
that implementation did not start; you can retry implementation without approving different
content.

## Desktop review

The current source-candidate Desktop UI includes a Plan control and review dialog. It can start a
plan, show the current revision and provenance, collect optional line-scoped feedback, refresh a
stale view, approve, approve and implement, abandon, and save a Markdown export. The normal
composer is unavailable while a revision awaits review so that ordinary text cannot bypass the
lifecycle controls.

These controls use the same typed ACP lifecycle as the CLI. Unsupported providers disable the
entry point. Final presentation details and packaged, cross-platform acceptance remain release
work; consult the candidate release notes before relying on Desktop availability.

The lightweight app shell intentionally receives only the minimum open-plan projection: status,
generation, revision ID, and revision SHA-256. It does not become plan authority or reproduce the
full review surface. Review an open plan in the first-party Desktop UI or CLI; the host continues
to reject provider or model transitions while the plan is open.

## Export and session transfer

`/plan-export` produces user-readable Markdown for the selected current revision. The document
contains provenance, but exporting it neither approves the revision nor grants implementation
authority.

Native session JSON export uses a separate optional `plan_history_v1` section. On import or copy,
gosling remaps plan identities and preserves that history as `stale`, so authority is not silently
transferred to another session. Session deletion removes its plan history; archiving preserves it.

## Customize the plan format

You can change the planning instructions by editing the `plan.md`
[prompt template](/docs/guides/context-engineering/prompt-templates). The host boundary and
lifecycle checks remain fixed even when the prompt wording changes.

## Additional resource

- [Planning Complex Tasks tutorial](/docs/tutorials/plan-feature-devcontainer-setup)
