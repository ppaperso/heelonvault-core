# Governance

Language: EN | [FR](GOVERNANCE.fr.md)

HeelonVault is developed by Heelonys and follows a maintainer-led model: decisions are made by the
maintainer, in the open whenever possible (issues and pull requests).

## Roles

| Role | Who | Responsibilities |
| --- | --- | --- |
| Maintainer | Patrick Paysan ([ppaperso](https://github.com/ppaperso)), Heelonys | Roadmap, review and merge, releases, security handling, licensing, repository administration and access |
| Contractor | Teams working under a contract with Heelonys | Implement the issues assigned to them, following [CONTRIBUTING.md](CONTRIBUTING.md) |
| Contributor | Anyone | Report issues, propose changes through pull requests within the contribution scope |

Contractors follow exactly the same process as other contributors: fork, pull request, review,
merge by the maintainer. Their contract may define additional deliverables, but not a different
path to `main`.

## Decisions

- Day-to-day decisions are made in the issue or the pull request concerned.
- The following decisions are written down in an issue **before** implementation, and approved by
  the maintainer: architecture changes, new dependencies, breaking changes to the public API of
  `heelonvault-core` (major release), changes to the security or key model, licensing and the
  boundary between the community and premium editions.
- When consensus is not reached, the maintainer decides and records the reason in the issue.

## Access

- `main` is protected: pull request required, full CI green, review conversations resolved,
  maintainer approval, squash merge only, no force-push, no deletion.
- **Write access** to this repository is granted only to people also authorized on the private
  `heelonvault-premium` repository. The reason is a mechanism, not a formality: the CI fetches
  `heelonvault-premium` with an SSH key stored as a repository secret (`PREMIUM_SSH_KEY`).
  GitHub hands that secret to every workflow run started from a branch of this repository, and
  such a run uses the workflow files of that branch. Anyone who can push a branch here can
  therefore change the workflow and read the premium sources or the key itself. Pull requests
  from forks do not receive the secret. Other contributors, contractors included, work from a
  fork; do not grant write access, even temporarily, to make their work easier.
- For the same reason, a fork branch is copied into this repository (to run the full CI) only
  after its whole diff has been reviewed ([CONTRIBUTING.md](CONTRIBUTING.md#review-and-merge)).
- Workflow actions are pinned by commit SHA and updated through Dependabot; release tags `v*`
  cannot be moved or deleted; secret scanning with push protection is enabled.
- Access granted for an engagement is reviewed at its end and removed when no longer needed.

## Releases and security

- Releases are prepared and published by the maintainer only
  ([CHANGELOG.en.md](docs/CHANGELOG.en.md)).
- Vulnerabilities are handled according to [SECURITY.md](SECURITY.md).

## Code of conduct

The [code of conduct](CODE_OF_CONDUCT.en.md) applies to every participant. It is enforced by the
maintainer (`contact@heelonys.fr`).

## Changes to this document

This document is changed by pull request, approved by the maintainer.
