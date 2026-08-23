# Making a Release

gosling releases are built and published by GitHub Actions from version tags. Preparing documentation or pushing a release branch does not publish a release.

## Current release target

The next release candidate is **v1.1.0**. This explicit minor release begins a
new `1.1` line. Thereafter, release versions increment the single-digit patch
component through `v1.1.1` to `v1.1.9`, then carry to `v1.2.0`.

The 2026-08-23 readback found that this checkout's source manifests declare
`0.1.0`, while the previous stable GitHub release is titled `v1.0.1` but is
tagged `v1.0.1-optimization-and-workspaces`. That historical tag does not match
the normal `[v]major.minor.patch` grammar. Preserve it as published history: do
not retag it or globally replace historical version strings. The historical
[v1.0.0 release notes](documentation/docs/release-notes/v1.0.0.md) remain a
point-in-time record, not the current release target.

## Required version alignment

Before tagging `v1.1.0`, update and review every version-bearing surface for
`1.1.0`, including:

- `Cargo.toml` workspace package version;
- workspace package entries in `Cargo.lock`;
- `ui/desktop/package.json` and the applicable pnpm lockfile entries;
- `ui/desktop/openapi.json` `info.version` and generated SDK metadata;
- packaged Desktop metadata and About/version output;
- README and candidate-specific documentation release notes.

## Automated release path

1. Run the [minor release workflow](https://github.com/cephalopod-ai/gosling/actions/workflows/minor-release.yaml) manually, or use its scheduled version-bump PR, if it matches the intended target.
2. Review and merge the version-bump PR into `main`.
3. Use the generated `release/<version>` branch and release PR for QA and release-only corrections.
4. Complete every required item in `RELEASE_CHECKLIST.md`, including installed artifacts on supported platforms.
5. Create and push the final `v1.1.0` tag only from the reviewed release commit.
6. Confirm `release.yml` completes and the GitHub release contains the expected signed artifacts, checksums, install scripts, and notes.
7. Perform the post-release checks before promoting updater behavior or announcing availability.

`release.yml` is currently tag-limited to `v1.*` releases. The previously inherited automatic patch-branch creation and tag-triggered release-PR cleanup workflows were intentionally retired. Patch releases therefore require an explicit reviewed branch/PR and tag; do not rely on an automatic next-patch branch.

## Tagging

Use the exact reviewed release commit. Replace `<release-commit>` only after the
checklist is complete:

```bash
git tag -a v1.1.0 <release-commit> -m "gosling v1.1.0"
git push origin v1.1.0
```

Do not move or recreate a published tag to repair an artifact. Fix forward with a new patch version.

## Release boundary

- Documentation may be merged before the tag, but install links continue to resolve to the latest published artifact.
- Historical audit and release notes remain point-in-time evidence and are not rewritten to make a release look green.
- A successful source test suite is not a substitute for installed Desktop, signing, updater, and clean-machine checks.
- The release owner, not documentation automation, approves signing, tagging, publication, and announcement.
