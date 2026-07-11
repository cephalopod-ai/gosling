# Website

This website is built using [Docusaurus](https://docusaurus.io/), a modern static website generator.

Run all commands from the `documentation/` directory.

For the repository-facing map of the documentation surface, see [INDEX.md](./INDEX.md).

### Installation

```
$ npm i
```

### Local Development

```
$ npm run start
```

This command starts a local development server and opens up a browser window. Most changes are reflected live without having to restart the server.

### Build

```
$ npm run build
```

This command generates static content into the `build` directory and can be served using any static contents hosting service.

### Deployment

Using SSH:

```
$ USE_SSH=true npm run deploy
```

Not using SSH:

```
$ GIT_USER=<Your GitHub username> npm run deploy
```

If you are using GitHub pages for hosting, this command is a convenient way to build the website and push to the `gh-pages` branch.

The repository workflows deploy through GitHub Actions rather than the `gh-pages`
branch. Maintainers must enable Pages with GitHub Actions as the source and set the
repository Actions variable `ENABLE_GITHUB_PAGES` to `true`. Until both are set, the
workflows finish with a visible disabled-deployment notice instead of attempting a
deployment that GitHub Pages rejects.
<!-- trigger deployment -->
