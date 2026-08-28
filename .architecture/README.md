# Architecture Registry

This directory is the durable architecture intent registry for `gosling`.

It exists to keep refactors, merge campaigns, and hardening work anchored to
explicit repository rules instead of only narrative documentation.

Update this registry when a change does any of the following:

- introduces a new cross-component dependency
- changes component ownership
- changes a privilege boundary
- changes compatibility-adapter behavior
- changes a transport-versus-service responsibility split

Files:

- `components.yaml`: component owners, responsibilities, and dependency rules
- `invariants.yaml`: repo-wide architectural invariants and review gates

Review rule:

- changes that violate an invariant must update the invariant first or carry an
  explicit follow-up plan in the same change

## Entry status

`invariants.yaml` entries carry a `status`:

- `active`: the invariant is in force and reviews must enforce it.
- `retired`: the code the invariant governed no longer exists. The entry stays
  so its `ARC-NNN` id is never reused and the rule remains discoverable, and it
  records `retired_in` (the commit that removed the subject). A retired entry's
  `scope` paths intentionally name code that is gone; drift tooling must skip
  retired entries rather than report their paths as missing.

`components.yaml` is a current-state ownership map and carries no status. A
component whose code is removed is deleted from the map — the durable record of
that component lives in the retired invariants, its ADR, and version control.

## Removing a feature

Removing a feature is one of the changes that must update this registry. Before
the removal lands:

- delete the component entry for code that no longer exists;
- set every invariant whose scope was that code to `retired` with `retired_in`;
- retire the ADR that introduced it (status change, body unchanged).

An invariant left `active` over deleted code asserts a control that nothing
implements. That is worse than having no entry, because every review and audit
built on this registry inherits the false assurance.

