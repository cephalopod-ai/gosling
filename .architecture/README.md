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
