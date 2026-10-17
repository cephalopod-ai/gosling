# 2026-10-17 — Together catalog repair and model search

## Scope and baseline

Requested: patch the investigated Together model-loading failure and add visible
model search across providers in the Switch models dialog. Clean `main` at
`9b72051bd`, macOS arm64, Rust 1.92, Desktop 1.5.0. No dependency changes,
publication, credential changes, or production configuration edits are needed.

Source findings: `REL-TOGETHER-001` (array catalogs become a static fallback and
valid models fail validation) and `REL-TOGETHER-002` (Muse is excluded from
recommendations), recorded in the preceding investigation at
`/var/folders/by/x9xn788x3fg1p54wlgwmp0500000gn/T/gosling-together-investigation-6ci6ol8b/report.md`.
Both are Medium reliability defects, localized implementation complexity.
The screenshot's model-search request is an explicitly authorized UI addition,
separate from the defect-only patch workflow.

## Stage plan and contracts

1. Repair shared catalog parsing and Muse family discovery together. Preserve
   OpenAI `data` envelopes, authentication/error propagation and static fallback
   behavior for missing endpoints. Accept Together arrays and use its `type`
   metadata to omit non-chat modalities. Add mixed Muse/Llama regression coverage.
2. Add a visible model-search field in the shared Switch models dialog. Derive
   filtered options from the complete provider cache, preserve manual model entry,
   and clear the query when changing providers. Verify selection and provider
   changes using the real dialog and select components.
3. Run focused Rust/UI checks, inspect transitive callers and the combined diff,
   then repeat the original catalog/configuration journey in isolated state.

Authoritative contracts: `Provider::fetch_supported_models` and
`fetch_recommended_models`; the declarative provider schema; ACP supported-model
and defaults-save DTOs; Desktop `acpListProviderModels`; and the existing model
transition boundary in `docs/architecture.md`. The existing parser violates the
documented Together array shape. UI/backend DTOs, session handoff and credential
resolution remain unchanged. The new search intentionally changes only local
presentation/filtering, with no provider-specific branch.

Pre-repair evidence: authenticated Together catalog HTTP 200, 269 records,
requested promo ID present. Installed-binary isolated reproduction rejects the
array catalog, accepts the equivalent `data` envelope, and drops Muse from a
mixed recommended inventory. The previous session's passing typecheck and
provider tests apply to the unchanged baseline; new regressions will capture the
specific failing paths before production edits.

Independent reads/checks can run concurrently. Rust commands share Cargo's
target directory and are sequential; UI checks can overlap them. No delegation
is requested or required. This log is the resumable checkpoint.

## Checkpoint

Both implementation stages complete. Before production edits, two of three new
Rust regressions failed (Together arrays fell back, including empty arrays), and
all four UI regressions failed because the search field was absent. Existing
OpenAI envelope compatibility passed. After the patch, all 498 provider unit
tests, all three new catalog regressions and all four new real-dialog UI tests
pass. TypeScript, targeted ESLint and provider Clippy also pass. Backend build,
native replay, localization checks and the final review are now complete; see
the evidence below.

## Validation and follow-ups

Full inference and promo availability after Together's displayed free-until date
are outside the patch verification.
The existing canonical README's OpenRouter/models.dev source discrepancy remains
an unrelated documentation follow-up.

### Results

- `cargo test -p gosling-providers --locked --offline --quiet` — 521 tests
  passed: 498 unit tests and 23 integration tests, including the three new
  discovery regressions. Existing authentication, error-payload, fallback,
  transport, streaming and model-filter behavior remains covered.
- `cargo clippy -p gosling-providers --all-targets --locked --offline -- -D warnings`
  — passed. `cargo fmt --all -- --check` and `git diff --check` — passed.
- `scripts/with-rusty-v8-cache.sh cargo build -p gosling-cli --bin gosling --locked --offline`
  — passed, using the existing Hermit environment, Command Line Tools and four
  Cargo jobs. No toolchain or dependencies installed; lockfiles unchanged.
- Fresh `target/debug/gosling acp` process, isolated config/data/state and disabled
  keyring: both Together array and OpenAI envelope fixtures list the promo ID,
  successfully save it as the default and retain it in refreshed recommendations.
  An embedding entry is excluded. No inference request or credential is involved.
  Results: `/var/folders/by/x9xn788x3fg1p54wlgwmp0500000gn/T/gosling-together-patched-bq84xgsf/probe-results.json`.
- `pnpm exec vitest run src/components/settings/models src/components/onboarding/ProviderConfigForm.test.tsx src/components/research/ResearchModelTeamSelector.test.tsx src/components/workspaces/WorkspaceEditorDialog.test.tsx`
  — 34 tests passed across five files. The new dialog tests use the real Select
  and Dialog components and cover case-insensitive search, selection stability,
  clearing, cached provider changes, custom providers, keyboard selection and
  unlisted IDs. Provider calls and the final changeModel effect are mocked.
- `pnpm run typecheck`, targeted ESLint, message extraction/compilation and
  `pnpm run i18n:check` — passed. The localization check also passed 21 sync/recovery
  tests and validated all 15 locale catalogs. Generated synchronization added the
  search label and reconciled an existing English security-scanner disclosure
  that had not yet reached the locale catalogs/source-hash manifest; no existing
  translation or source message was changed or removed.
- Prettier checks for both changed TSX files and provider-guide MDX compilation
  passed. The first MDX invocation ran at the repository root, where the package
  is not resolvable; running it from the existing `documentation` package passed
  without installing dependencies or changing the guide.

### Review, record closure and boundaries

The combined review followed catalog request → parser → raw supported-model list
and recommended inventory → ACP model validation → Desktop options and submission.
The second completeness pass matched both source finding IDs to the original
failing native probe and verified that the changed binary corrects both paths.
No new DTO, configuration key, storage format or session-handoff behavior was
introduced. Architecture/contract drift delta: **no new drift**; the documented
Together response-shape mismatch is corrected. No fixed-defect TODO markers
remain in the touched functions.

- `REL-TOGETHER-001`: open → repaired and locally verified on 2026-10-17, by the
  array parser change and successful native ACP defaults/save reproduction.
- `REL-TOGETHER-002`: open → repaired and locally verified on 2026-10-17, by Muse
  family recognition and a mixed Muse/Llama native inventory reproduction.
- The original scratch investigation is retained as historical evidence, with a
  dated disposition addendum linking this log. There is no external issue tracker
  record to update and no new governance ledger was invented.
- The shared Switch models search addition is verified in React DOM tests for
  built-in and custom-provider entries; the model cache is no longer rewritten
  by search. The query clears on provider changes, and manual-ID entry is retained.
- Updated the provider usage guide, docs index and selected-log ignore exception.

Final scoped status: `completed_verified` for the source patch and listed local
checks. The debug CLI was rebuilt for validation. The installed app/CLI were not
replaced in this patch-only continuation; packaged/native visual testing, full
workspace tests and authenticated generation were not performed. These results
do not claim a published release, deployed GUI, cross-platform qualification or
future promotional pricing.

## Authorized rebuild and reinstall

The operator subsequently requested “rebuild and reinstall”. Source baseline:
clean `main` at `55e829719`, including the preceding patch and its tests. The
existing Rust application workflow and local macOS arm64 package procedure apply.

Checkpoint: previous installed app and standalone CLI backed up under
`/Users/eric/.local/share/gosling/install-backups/reinstall-20261017-together-jl0y8zdb/`.
No Gosling GUI process was running at preflight. Formatting passed. Release
packaging is running through `just package-ui`, with Command Line Tools,
`RUSTUP_AUTO_INSTALL=0`, four Cargo jobs and Cargo offline mode. The receipt records
the source revision, prior binary hashes and lockfile fingerprints; the build log
is retained in the same directory. The package recipe uses existing Cargo/pnpm
inputs and ad-hoc signing. No version bump, publication or user-data migration is
part of this reinstall.

### Rebuild and installation results

- `just package-ui` passed: optimized Rust build, SDK/i18n generation, Vite,
  Electron arm64 packaging and ad-hoc signing. Cargo and pnpm lockfiles stayed
  unchanged. Runtime sources stayed at the preflight revision.
- Packaged, staged-for-install and installed apps passed
  `codesign --verify --deep --strict`; the rollback copy also passed.
- Release, staged, packaged/installed embedded backend and standalone CLI share
  SHA-256 `aab44a298b369370bb96aec4dc67c8fc4084b69d020f13b837b35aef8ee177fc`.
  This differs from the previous `a1980797...` build. Bundle, embedded backend
  and CLI all remain version 1.5.0.
- Replaced `/Applications/Gosling.app` and `~/.local/bin/gosling`. The previous
  installed app was also retained by rename under `previous-installed-original/`
  in the rollback directory. User configuration, credentials, sessions and
  Research Library were outside these replacement paths.
- Packaged ASAR inspection found the new search control in
  `.vite/renderer/main_window/assets/App-06TUX9Fu.js`. The initial verifier looked
  only in an `index-*.js` asset and failed; inspecting the actual packaged chunks
  and correcting that verifier passed. Production code/package needed no change.
- Installed embedded backend, fresh ACP process and temporary config/data/state:
  the authenticated Together catalog returned 191 supported chat/text models,
  including Muse Glimmer, Muse Spark Open and the exact promo ID. Refreshed
  recommendations contained 166 models and retained the promo ID. No completion,
  paid inference or user default-model change was performed. Sanitized results:
  `installed-provider-verification.json` in the rollback directory.

### GUI startup qualification

Launched the installed app through computer use. Its process runs from
`/Applications/Gosling.app/Contents/MacOS/Gosling`, but no main window/backend
appeared and native window inspection timed out. A one-second process sample
records its main thread waiting in `SecItemCopyMatching` /
`SecKeychainItemCopyContent` and Security IPC before the application startup log.
SecurityAgent is running. The computer-use tool explicitly refuses access to
`com.apple.SecurityAgent` for safety reasons; no security setting was bypassed.

The operator has been asked to review/unlock any expected macOS Keychain prompt
directly, without sharing a password. Current receipt status:
`installed_gui_keychain_pending`. Build, signatures, local replacement, packaged
search and live provider verification are complete. Native GUI rendering/search
inspection remains pending that Keychain interaction. Diagnostic sample,
verification artifacts, build log and rollback copies are retained in the same
backup directory.

### GUI verification after Keychain unlock

The operator resolved the macOS prompt. The installed app then opened normally;
native inspection identified its renderer at
`file:///Applications/Gosling.app/Contents/Resources/app.asar/.vite/renderer/main_window/index.html`.
The restored chat, navigation and Outputs pane rendered successfully.

Opened Switch models, selected Together AI locally in the dialog and entered
`mUsE` in the visible Search models field. The model dropdown contained exactly
three results: `meta-models/Muse-Glimmer-30B`,
`meta-models/Muse-Spark-Open-816B-A42B` and
`meta-models/Muse-Spark-Open-816B-A42B-promo`. This verifies case-insensitive
filtering and the live promo catalog in the installed GUI. Cancelled the dialog;
the original chat still shows `qwen3.8:27b`. No model change or inference was
submitted.

Reinstall status is now `installed_and_launched`, with native window and search
verification complete. The original Keychain diagnostic remains historical
evidence; the receipt records the operator resolution and successful GUI check.
