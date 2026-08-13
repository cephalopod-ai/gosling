# 2026-08-13 project-shell readiness reassessment

- Task: reassess merged shared-shell implementation and replace the forward plan before DAWES,
  math, or another named project shell is created.
- Assessed source: clean `main` at `6fe6a3bfcab3a48846850fd321fb8a056223d355`; planning edits on
  `codex/project-shell-readiness-replan`.
- Muninn: available, but no authoritative shell-productization chat history was found. Broad recall
  returned only tentative low-trust project research artifacts and did not define shell integration.
  Source, ADRs, current CI, and runtime/package tests remained authority.
- Retained strengths: strict product identity, server-owned provisioning/policy/session authority,
  dedicated least-privilege Electron entries, main-owned authenticated ACP preflight, compatibility
  before session creation, explicit handoff, and host-target package integrity.
- Blocking findings: no copy-free renderer composition; no renderer session/prompt/update/permission
  API; focused ACP drops interactive behavior; production CLI injects no `DomainAdapter`; lifecycle
  states lack real producers; safe renderer snapshot is incomplete; package metadata/resources retain
  Gosling-specific/broad surfaces; packaged application/coexistence and reusable workflows are absent.
- CI correction: current main run `31695906352` failed before Linux Rust tests at
  `with-rusty-v8-cache.sh` line 44 (`File: unbound variable`). Local macOS helper tests still pass.
- Plan decision: preserve Gates 0–4 as historical evidence; supersede forward Gates 5–8 with R0–R8
  and distinguish host-substrate, project-shell-consumer, and fixture-distribution readiness.
- Immediate sequence: R0 portable helper repair plus two clean Linux runs; R1 ADRs for consumer
  topology, main-owned application runtime, and domain adapter transport; only then R2/R3 code.
- Validation: local V8 helper passed; shell profile/package suite 41/41; focused shell/host/serve/
  handoff Vitest 120/120 across 14 files; Desktop typecheck passed. Focused Prettier, Markdown-link
  resolution, docs-governance marker, documentation-only boundary, and `git diff --check` passed.
- Test ledger: no `docs/testing/test-ledger.yaml` or equivalent required ledger exists; no test code
  changed in this planning task.
- Scope: documentation/assessment only. No runtime, workflow, named shell, production identity,
  release destination, signing, notarization, publication, upload, or updater behavior was changed.
