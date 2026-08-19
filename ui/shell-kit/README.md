# Gosling Shell Kit

`@repo-makeover/gosling-shell-kit` is the versioned build and conformance boundary for a shell
consumer owned outside the Gosling repository. Consumers pin its exact version in `dependencies`;
the resolver derives the trusted consumer root from that package registration and never accepts a
root from `shell-consumer.json`.

```sh
npm pkg set 'dependencies.@repo-makeover/gosling-shell-kit=0.1.0'
npm install --ignore-scripts --no-save /reviewed/path/repo-makeover-gosling-shell-kit-0.1.0.tgz
npx gosling-shell init --id example-shell --display-name "Example Shell"
npx gosling-shell check shell-consumer.json
npx gosling-shell resolve --manifest shell-consumer.json --target linux-x64
```

Registry publication is not implied by this source package. After a separately authorized release,
the archive install can be replaced by an exact registry install of the same version.

`init` refuses to overwrite any existing consumer/product files. Generated products are explicitly
non-publishable, unsigned, updater-disabled templates. `check` and `resolve` reject traversal,
symlinks, secret-shaped data, unpinned shell-kit dependencies, and manifest/package version drift.
