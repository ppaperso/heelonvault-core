# Releasing HeelonVault

Language: EN | [FR](RELEASING.md)

## Versioning rule

**One version number** for the whole set:

- the product (`heelonvault-app`, `vX.Y.Z` tags, GitHub releases, changelog);
- the `heelonvault-core` crate, published on [crates.io](https://crates.io/crates/heelonvault-core);
- `heelonvault-premium`, which depends on `heelonvault-core = "X.Y"`.

The number follows [semver](https://semver.org/), applied to the **public API of `heelonvault-core`**:

| Change in `heelonvault-core` | Version |
|---|---|
| Breaking: public item removed or renamed, signature changed, required method added to a public trait, variant added to an exhaustive enum | **major** (`X+1.0.0`) |
| Compatible addition: new function, new type, trait method with a default implementation, variant of a `#[non_exhaustive]` enum | **minor** (`X.Y+1.0`) |
| Fix without API change | **patch** (`X.Y.Z+1`) |

Why: a `heelonvault-core = "1.1"` dependency accepts any version from `1.1.0` up to, but excluding, `2.0.0`. Cargo therefore installs a new minor without asking; if it breaks the API, the user's code no longer compiles.

An API break therefore forces **the whole product** to a major version. To keep breaks rare:

- mark public enums that will grow `#[non_exhaustive]` (already the case for `AppError`, `AccessDeniedReason`, `RecoveryFailure`, `AuditAction`);
- give methods added to a public trait a default implementation, when possible;
- deprecate before removing: `#[deprecated]` in a minor, removal in the next major.

Pre-releases (`X.Y.Z-rc.N`) are tagged to build test packages; they are not published on crates.io.

## Automated guardrails

- **CI — `cargo semver-checks -p heelonvault-core`**: compares the API with the latest version published on crates.io. Fails when the declared version does not cover the breaks.
- **CI — `cargo publish --dry-run -p heelonvault-core`**: the crate must build on its own, with its crates.io dependencies, outside the workspace.
- **Test `crates/heelonvault-app/tests/release_consistency.rs`**: core, app and premium carry the same version; premium depends on that core version; the four changelogs (core and premium, FR and EN) have a section for it.

## Release checklist

1. **Versions** — in `crates/heelonvault-core/Cargo.toml`, `crates/heelonvault-app/Cargo.toml` and `heelonvault-premium/Cargo.toml` (`version` and `heelonvault-core = "X.Y"`).
2. **Lockfiles** — `git diff Cargo.lock` (in both repositories) must only show the version lines of our crates. If Cargo also changed other dependencies, restore the file, edit those lines by hand, then check with `cargo check --workspace --locked`.
3. **Documentation** — FR/EN changelogs of both repositories, README titles, `docs/UPDATE_GUIDE*.md`, `crates/heelonvault-core/README.md` (the `heelonvault-core = "X.Y"` line).
4. **SBOM** — `./scripts/generate-sbom.sh`, then commit `sbom.cyclonedx.json`.
5. **Local checks** (from `heelonvault-core`):
   ```bash
   cargo fmt -p heelonvault-core -p heelonvault-app -p sqlx -- --check
   cargo clippy --workspace --all-targets --all-features -- -D warnings
   cargo test --workspace --locked
   cargo check -p heelonvault-app --features licensing
   cargo semver-checks -p heelonvault-core
   cargo publish --dry-run -p heelonvault-core --locked
   ```
6. **Merge premium first.** Core's CI checks out premium's `main` branch: as long as it requires the previous core version, the core PR does not compile. Merge the premium PR, re-run the core PR's CI, then merge it.
7. **Publish the crate** from an up-to-date `main`: `cargo publish -p heelonvault-core`. Publishing is permanent (a version can only be withdrawn with `cargo yank`, never deleted).
8. **Tag** core's merge commit: `git tag vX.Y.Z && git push origin vX.Y.Z`. The tag triggers the AppImage, DMG, MSI and SBOM workflows, which create the GitHub release.
9. **Check** the GitHub release (artifacts and SHA-256 sums), the crates.io page and the documentation on [docs.rs](https://docs.rs/heelonvault-core).
