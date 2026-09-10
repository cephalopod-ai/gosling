# gosling Release Checklist

This is a maintainer-owned publish gate. Documentation preparation does not check
any item automatically. The current candidate is `v1.2.5`; preserve published
historical tags rather than rewriting them.

## Version and source identity

- [ ] The release commit is reviewed, immutable for the tag, and based on the intended `main` revision.
- [ ] `Cargo.toml`, workspace entries in `Cargo.lock`, `ui/desktop/package.json`, packaged app metadata, About, and `gosling --version` all report `1.2.5`.
- [ ] The ACP schema and generated TypeScript SDK match the release source. They do not carry the app version; the deleted Desktop OpenAPI schema is not recreated or staged.
- [ ] `README.md`, `RELEASE.md`, and the candidate release notes match the final release scope.
- [ ] Contributor and goose upstream attribution remain intact.
- [ ] No release note claims a test, platform, signature, updater state, or artifact that was not observed.

## Source validation

- [ ] `cargo fmt --all -- --check` passes on the final `v1.2.5` tree.
- [ ] `cargo clippy --all-targets -- -D warnings` passes on the final `v1.2.5` tree.
- [ ] The complete Rust workspace test suite passes on the final `v1.2.5` tree.
- [ ] `cd ui/desktop && pnpm run typecheck` passes on the final `v1.2.5` tree.
- [ ] `cd ui/desktop && pnpm test` passes on the final `v1.2.5` tree.
- [ ] The documentation build, test, and typecheck commands pass from `documentation/` on the final candidate.
- [ ] Provider-managed sessions do not receive a gosling-owned compaction countdown; the known per-turn context-injection regression is repaired or explicitly blocks publication.
- [ ] Release workflow integrity, lockfile integrity, and archive/checksum verification pass.
- [ ] The full scenario-card suite is replayed, or every non-replayed card is explicitly dispositioned with focused replacement evidence.

## Installed Desktop: macOS 26 Apple Silicon

- [ ] Install the signed Apple Silicon release artifact on a clean or isolated account; do not test only `just run-ui`.
- [ ] Launch, onboarding, local ACP connection, app restart, and `Cmd+Q` cleanup work without orphaned Electron or `gosling serve --platform desktop` processes.
- [ ] `File > New Chat Window` opens an independent usable window; closing one window does not terminate another.
- [ ] The chat composer visibly includes the credential-profile selector and **Manage credential profiles** action.
- [ ] Creating, selecting, replacing, and deleting a credential profile preserves secret redaction and produces an actionable missing/relink state.
- [ ] Clicking a workspace filters the sidebar chat list and does not change the default workspace for global New Chat.
- [ ] Starting a chat from the `+` action beside a workspace preselects that workspace.
- [ ] Global New Chat allows explicit workspace selection, and an existing chat remains pinned to its original workspace and credential profile.
- [ ] **Crash recovery** offers Manual, Safe, and Always policies; Safe is the default, normal shutdown does not resume work, and unsafe/incomplete tool recovery waits for review.
- [ ] **Session Handoff** previews and inspects a bounded redacted checkpoint, preserves the prior provider on transition failure, and requires explicit confirmation for new-context-only targets.
- [ ] `Enter` inserts a newline; `Cmd+Enter` submits on macOS; while a task is running the shortcut queues and clicking **Send** interrupts.
- [ ] Qualified workspace outputs and valid local embedded files open directly without a redundant picker; ambiguous or blocked paths remain guarded; **Close all** clears every artifact tab.
- [ ] Packaged Content Security Policy permits only the required loopback ACP HTTP/WebSocket connection.
- [ ] Signed/notarized status, Gatekeeper launch, updater metadata, and architecture identity are correct.

## CLI and interoperability

- [ ] Clean installs and upgrades work on each published OS/architecture.
- [ ] `gosling --version`, `gosling doctor`, session creation/resume, interruption, provider failure, and machine-readable output behave as documented.
- [ ] Existing configurations using the removed direct `codex` provider receive migration guidance for `codex-acp` or `chatgpt_codex`; no documentation advertises removed Amp ACP, Avian, or Gemini OAuth implementations.
- [ ] Custom source builds that need `code-mode` enable it explicitly; default CLI builds are tested without it.
- [ ] gosling and goose coexist without sharing config, data, database, keyring service, deep-link scheme, or single-instance lock.
- [ ] Provider API-key and subscription-backed ACP flows are tested without committing credentials or logs containing secrets.
- [ ] MCP extension discovery and deterministic goose compatibility adapters preserve source attribution.

## Artifact and publication checks

- [ ] Release-branch artifacts are downloaded and tested before tagging.
- [ ] Every expected CLI and Desktop artifact is present, named correctly, and associated with the correct OS/architecture.
- [ ] Checksums verify against downloaded artifacts.
- [ ] macOS and Windows signatures are verified where applicable.
- [ ] The install scripts resolve the candidate version correctly in an isolated environment.
- [ ] The GitHub release body uses the final candidate notes and contains no inherited goose release boilerplate.

## Publish and post-release

- [ ] Create and push the selected candidate tag only after all blocking items above are complete.
- [ ] Confirm the tag-triggered `release.yml` run succeeds.
- [ ] Read back the GitHub release, assets, checksums, and install commands from an unauthenticated client.
- [ ] Confirm `releases/latest` and the stable install path resolve to the candidate version only after publication is complete.
- [ ] Keep native macOS auto-update disabled until a compatible shipped version and updater metadata make promotion safe.
- [ ] Record any failed or deferred gate in the release notes or a follow-up issue; do not silently waive it.
