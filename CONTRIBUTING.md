# Contribution Guide

Language: EN | [FR](CONTRIBUTING.fr.md)

Thanks for contributing to HeelonVault.

## Scope of external contributions

HeelonVault follows an Open Core model: this repository is public, while the
`heelonvault-premium` repository is private and is compiled into the application. The workspace
does not load without a checkout of that private repository, so **external contributions are
limited to**:

- the `heelonvault-core` crate (`crates/heelonvault-core/`): models, repositories, services,
  translations and their tests;
- the documentation (`docs/` and the Markdown files at the repository root).

The GTK application (`crates/heelonvault-app/`), the other crates, the CI workflows, packaging and
release are changed by the maintainers. To propose a change there, open an issue that describes
the need: a maintainer implements it or agrees on another approach with you.

This applies to every contributor without access to `heelonvault-premium`, including contractors.
Write access to this repository is only granted to people who are also authorized on
`heelonvault-premium` (see [GOVERNANCE.md](GOVERNANCE.md)).

## Getting started

Build details and the internals are described in the
[development guide](docs/internal/DEVELOPMENT.md) and in
[ARCHITECTURE.en.md](docs/ARCHITECTURE.en.md).

Without `heelonvault-premium`, check `heelonvault-core` with:

```bash
scripts/check-core-standalone.sh            # fmt, clippy, clippy-secure, tests
scripts/check-core-standalone.sh --semver   # + cargo semver-checks (needs cargo-semver-checks)
```

The script copies the crate into a separate workspace (by default
`~/.cache/heelonvault/core-standalone`) and runs the same quality gates as CI there. It never
modifies your checkout.

## Workflow

1. **Issue first**: every non-trivial change starts from an issue, in which the need and the
   approach are agreed before coding. Typo and wording fixes in the documentation can skip it.
2. **Fork** the repository on GitHub and clone your fork.
3. **Topic branch** from an up-to-date `main`, one subject per branch, named
   `<type>/<short-subject>`, for example `fix/csv-import-bom` or `docs/update-guide-proxy`.
   Types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`.
4. **Commit** small, focused changes. Rebase on `main` rather than merging `main` into your branch.
5. **Open a pull request** from your branch to `main` and fill in the template. Open it as a draft
   while it is not ready for review.

Nobody pushes directly to `main`: it is protected and only accepts pull requests.

## Code standards

- Follow the existing style, naming and module layout.
- No `unwrap()` / `expect()` (banned by `clippy.toml`), no `unsafe`, no warning: CI builds with
  `-D warnings`. Use typed errors (`thiserror`).
- No `#[allow(...)]` to silence a lint without a written rationale.
- Never let a secret value reach the UI layer, a log or an error message.
- Add tests for repository and service behaviour changes
  (`crates/heelonvault-core/tests/`).
- Changes to the public API of `heelonvault-core` follow semver: prefer default trait methods,
  `#[non_exhaustive]` and deprecation over breaking changes
  ([RELEASING.en.md](docs/internal/RELEASING.en.md)).
- No new dependency without discussing it in the issue first. Do not commit secrets or personal
  data.

## Pull requests

The [pull request template](.github/pull_request_template.md) holds the checklist. In short:

- the PR links its issue (`Closes #123`) and covers a single subject;
- the quality gates pass, and the PR description lists the validation commands you ran;
- `sbom.cyclonedx.json` is regenerated if dependencies changed;
- user-visible changes are reflected in the changelog (FR and EN) and in the relevant guide
  (`docs/QUICKSTART*`, `docs/USER_GUIDE*`, `docs/UPDATE_GUIDE*`);
- security-sensitive changes are justified in the PR description.

The PR title becomes the commit message on `main` (squash merge): write it as
`<type>(<scope>): <summary>`, for example `fix(import): accept a UTF-8 BOM in CSV headers`.

### Continuous integration on a fork

GitHub does not give pull requests from forks access to the private repository. On such a PR:

- the **`heelonvault-core (standalone)`** job runs `scripts/check-core-standalone.sh --semver`:
  it must be green;
- the full `check, clippy, fmt, test` check fails at `Resolve Premium revision`. This is expected:
  before merging, a maintainer runs the full CI on a copy of your branch (see below).

## Review and merge

- **Who reviews and merges**: only the maintainer listed in [GOVERNANCE.md](GOVERNANCE.md).
  Contributors do not merge, including their own pull requests.
- **Merge conditions**, all required:
  1. the full CI (`check, clippy, fmt, test`) is green on the commit being merged;
  2. every review conversation is resolved;
  3. the maintainer has explicitly approved the pull request.
- **Merge method**: squash only. The branch is deleted after the merge.
- Any new push dismisses a previous approval: the maintainer reviews the latest changes again.
- **Fork pull requests**: once the review is done, the maintainer pushes your branch to an
  internal branch of this repository, where the full CI runs, then squash-merges it with you
  credited as author (`Co-authored-by`). Your original pull request is closed with a link to the
  merged commit.
- **Maintainer rule for that copy**: the internal CI checks out `heelonvault-premium` and runs the
  contributed code (tests, `build.rs`, new dependencies, workflow files) next to it. The branch is
  pushed internally only **after** reviewing its whole diff, with particular attention to
  `.github/`, `build.rs`, `Cargo.toml` / `Cargo.lock` and any code that reaches the network or the
  file system; never just to see whether CI passes. A pull request that changes `.github/` is not
  copied: that change is made by a maintainer.
- Expect a first answer within five working days. A pull request with no activity from its author
  for 90 days may be closed; it can be reopened at any time.

## Documentation

`docs/` is published on [doc.heelonvault.heelonys.fr](https://doc.heelonvault.heelonys.fr);
`docs/internal/` is for maintainers and is not published. Keep both languages in sync.

## Code of conduct

Participation in this project is governed by the [code of conduct](CODE_OF_CONDUCT.en.md).

## Security reports

Do not open a public issue for security vulnerabilities: see [SECURITY.md](SECURITY.md).
Contact: `security@heelonys.fr`
