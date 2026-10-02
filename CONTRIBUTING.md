# Contribution Guide

Language: EN | [FR](CONTRIBUTING.fr.md)

Thanks for contributing to HeelonVault.

## Getting started

Prerequisites, build, tests and quality gates are described in the
[development guide](docs/internal/DEVELOPMENT.md). The internals are covered in
[ARCHITECTURE.en.md](docs/ARCHITECTURE.en.md).

> Building the workspace currently requires the private `heelonvault-premium` repository checked
> out next to this one (see the development guide). Documentation fixes and changes confined to
> the `heelonvault-core` crate can still be checked with `cargo test -p heelonvault-core` from a
> standalone copy of that crate.

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
- Prefer small, focused commits. Do not commit secrets or personal data.

## Pull request checklist

- The quality gates of the development guide pass (`fmt`, `clippy`, `test`, `semver-checks`).
- `sbom.cyclonedx.json` is regenerated if dependencies changed.
- User-visible changes are reflected in the changelog (FR and EN) and in the relevant guide
  (`docs/QUICKSTART*`, `docs/USER_GUIDE*`, `docs/UPDATE_GUIDE*`).
- Security-sensitive changes are justified in the PR description.

## Documentation

`docs/` is published on [doc.heelonvault.heelonys.fr](https://doc.heelonvault.heelonys.fr);
`docs/internal/` is for maintainers and is not published. Keep both languages in sync.

## Security reports

Do not open a public issue for security vulnerabilities: see [SECURITY.md](SECURITY.md).
Contact: `security@heelonys.fr`
