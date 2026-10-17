# 2026-10-17 — Model context window discovery and overrides

## Scope and source findings

Requested: correct the context window for Spark Open and let users edit a model's
context window when discovery is unavailable, with an appropriate warning and
validation error. The initial working tree was clean. No credentials, user
configuration, installed application or dependency versions are changed.

The OpenAI-compatible model parser retained IDs but discarded Together's
`context_length` metadata. Generic model-info construction replaced unknown
limits with 128,000, obscuring the distinction between a known limit and a runtime
fallback. Desktop's Switch models dialog also discarded available context metadata
when constructing the selected model, even though the session transition already
accepted `targetContextLimit`.

Source contracts: `Provider` model metadata and effective context APIs,
`ModelConfig`, provider configuration entries, ACP defaults and supported-model
DTOs, the checkpoint-backed session transition, and Desktop's shared model context.
The architecture keeps provider execution and compaction in core; Desktop only
edits and presents values. The catalog's defect-only skill explicitly excludes
feature/GUI additions, so this requested addition uses the repository workflow.

Together's documented catalog shape includes `context_length`:
<https://docs.together.ai/reference/models>. No context size is hard-coded for Spark.

## Implementation checkpoint

- Discover positive integer `context_length` metadata in array and `data` catalog
  shapes, preserve local `meta.n_ctx` support, and retain explicit override priority.
- Return known context limits alongside supported model IDs through an additive,
  optional ACP map. Unknown metadata remains unknown; runtime retains its fallback.
- Expose a context-window field with unknown-limit and capacity warnings. Reject
  zero, negative, fractional, nonnumeric and unsafe integer values. Clear the
  override when changing targets, restore saved values, and invalidate a checkpoint
  preview after changing its context budget.
- Save Settings selections with the provider's default model; session selections
  use the existing transition and persistence path. Replace stale renderer model
  metadata after activation. Providers managing their own context hide the control.
- Refresh the context-management guide and regenerate schema/SDK/localization
  artifacts. Relevant validation is recorded below as it finishes.

## Validation and limits

Provider tests passed (498 unit tests and 26 integration tests), including metadata
discovery, unknown/invalid limits, runtime probes and explicit override priority.
The first focused Desktop run found two new test-fixture/expectation mistakes:
missing redaction-report fields and an unformatted number expectation. The fixture
was completed and the user-facing number now uses locale formatting. The next run
passed 22 tests. Additional persistence and ACP merge coverage is in progress.

Final scoped status: source implementation and focused automated validation
complete. No packaged GUI or authenticated inference is claimed. Rust commands
ran sequentially because they share Cargo's build cache; independent frontend
checks ran concurrently.

### Final results

- `cargo test -p gosling-providers --locked --offline --quiet` — 524 tests passed
  (498 unit tests and 26 integration tests).
- `cargo test -p gosling --lib --features
  code-mode,aws-providers,telemetry,otel,rustls-tls,system-keyring
  config::providers::tests --locked --offline --quiet` — 10 tests passed, including
  scoped storage, model changes and zero-limit rejection without changing defaults.
- `cargo test -p gosling --test context_window_overrides --features
  code-mode,aws-providers,telemetry,otel,rustls-tls,system-keyring
  --locked --offline --quiet` — two integration tests passed. Task-scoped unsaved
  configuration proves provider/model isolation, global configuration precedence,
  explicit session override priority and zero-limit rejection without writing
  operator settings.
- ACP schema generation compiled and executed successfully with the same features;
  `pnpm run build-gosling-sdk` regenerated and built the SDK. Only the three intended
  ACP definitions changed, all through optional additive fields. These Rust
  commands used Hermit, Command Line Tools, four jobs, offline locked dependencies,
  disabled automatic toolchain installation and the existing V8 cache wrapper.
- Focused Desktop regression run — 75 tests passed across eight files: the shared
  model dialog, model/provider context, ACP provider adapter, context-limit resolver,
  Hub, research selector, and workspace editor. The final run includes the loading
  guard so a selection cannot submit before context metadata finishes loading.
- `pnpm run typecheck` and targeted ESLint with zero allowed warnings — passed.
  Type checking initially exposed a new inferred required `contextLimit` property;
  preserving optional metadata in the adapter corrected it without modifying
  unrelated test expectations.
- Localization extraction, synchronization, compilation and `pnpm run i18n:check`
  — passed, including 21 synchronization/recovery tests and validation of 15 locale
  catalogs. Exactly five new messages were added; no existing message hashes changed
  and no messages were removed. New entries use the normal English fallback.
- Cargo formatting, targeted Prettier, `git diff --check`, and compilation of the
  edited context-management guide with the documentation package's MDX compiler
  — passed. Cargo and pnpm lockfiles are unchanged.

### Final review and limits

Reviewed catalog parsing → model-info construction → optional ACP metadata →
provider/model selection → checkpoint preview → transition → session metadata and
compaction, plus Settings save/read → matching default-model materialization.
Explicit custom-provider model limits retain priority over catalog-derived values.
Unknown limits do not masquerade as discovered 128,000-token metadata, while the
runtime fallback remains available. Clearing an override returns to automatic
selection; switching targets clears the prior field value; changing a previewed
budget invalidates that preview. Failed transitions retain the existing provider.

The requested defaults persistence and metadata additions are reflected in the
Rust contracts and regenerated consumers. The execution/compaction ownership and
checkpoint transition boundaries remain unchanged. No new architecture drift was
identified in this scoped comparison; this is not a repository-wide audit.

The complete workspace suite, Clippy, native GUI replay, authenticated inference,
release packaging and reinstall were not run. `/Applications/Gosling.app` and the
standalone installed CLI remain unchanged. The source patch is locally verified;
installed behavior will require rebuilding and reinstalling the application.
