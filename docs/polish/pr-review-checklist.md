# PR review checklist

## Collaboration surface review

- [x] Pull request and bug/feature issue templates exist under .github.
- [x] .github/CODEOWNERS has a catch-all maintainer route.
- [x] Dependabot configuration exists.
- [x] Third-party workflow actions sampled by the static scan are SHA-pinned.
- [x] The only pull_request_target workflow is restricted to Dependabot and explicitly avoids checking out pull-request code.
- [ ] Default-branch protection is active. GitHub API reports no branch protection, and the repository ruleset is disabled.
- [ ] Current default-branch CI is green. The inspected CI run was still in progress; an earlier BuildNotify run failed.
- [x] BuildNotify treats an absent optional Discord webhook as a successful,
      visible no-op and retains delivery when configured.
- [ ] The inherited repository-identity gate on Dependabot auto-merge is reconciled with the current remote, where it is a no-op.

## Reviewer handoff

Do not publish from this checkout. Local format, build, test, lint, Desktop,
documentation, and Rust dependency gates pass; branch protection, remote CI,
fresh-clone/secret-history validation, installed artifacts, signatures, and
publication readback remain external gates.
