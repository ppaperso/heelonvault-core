# Development Guide

Language: EN | [FR](DEVELOPMENT.fr.md)

Building, running and testing HeelonVault from source. For contribution rules, see
[CONTRIBUTING.md](../../CONTRIBUTING.md); for the internals, [ARCHITECTURE.en.md](../ARCHITECTURE.en.md).

## Prerequisites

- **Rust**: install [rustup](https://rustup.rs/); the `1.98.0` toolchain pinned by
  `rust-toolchain.toml` is picked up automatically.
- **GTK4 and libadwaita development files**:
  - Ubuntu / Debian: `sudo apt install build-essential pkg-config libgtk-4-dev libadwaita-1-dev`
  - Fedora: `sudo dnf install gcc pkgconf-pkg-config gtk4-devel libadwaita-devel`
- **`heelonvault-premium`** checked out as a sibling directory (`../heelonvault-premium`).
  `heelonvault-app` declares it as an optional path dependency, and Cargo loads the manifest of
  every path dependency, even optional ones: **without this checkout, the workspace does not
  load**, `--no-default-features` included. The `[patch]` entries of the root `Cargo.toml` point
  at the same sibling directory.

```bash
rustc --version   # 1.98.0
```

## Run in development

```bash
./scripts/run-dev.sh
```

- database: `data/heelonvault-rust-dev.db` (created on first run);
- logs: `./logs`, level `debug`.

`run-dev.sh` always sets `HEELONVAULT_DB_PATH`, `HEELONVAULT_LOG_DIR` and
`HEELONVAULT_LOG_LEVEL`, so these cannot be overridden through it. `RUST_LOG` still wins over the
level; for other paths, run the binary directly:

```bash
RUST_LOG=info,heelonvault::ui=debug ./scripts/run-dev.sh
HEELONVAULT_DB_PATH=/tmp/hv-dev.db HEELONVAULT_LOG_DIR=/tmp/hv-logs cargo run -p heelonvault-app
```

## Tests

Integration tests live in `crates/heelonvault-core/tests/` and use a real SQLite database
(helpers in `tests/common/mod.rs`). `crates/heelonvault-app/tests/` holds repository consistency
checks: shared version number, and every message key used by the UI present in both catalogs.

```bash
cargo test --workspace
cargo test -p heelonvault-core --test security_crypto       # one test file
cargo test -p heelonvault-core some_test_fn_name            # one test
```

## Quality gates (what CI enforces)

`.cargo/config.toml` sets `-D warnings` and `-D unsafe_code`; `clippy.toml` bans `unwrap()` /
`expect()`. Before pushing:

```bash
cargo fmt -p heelonvault-core -p heelonvault-app -p sqlx -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo clippy-secure
cargo test --workspace --locked
cargo semver-checks -p heelonvault-core
cargo publish --dry-run -p heelonvault-core --locked
cargo audit
cargo deny check advisories
```

`cargo fmt --all` would also reformat the sibling premium repository: format per package.

When dependencies change, regenerate the SBOM and commit it (the `check-sbom` CI job fails
otherwise):

```bash
./scripts/generate-sbom.sh
```

## Release build and Linux system install

```bash
cargo build --release -p heelonvault-app
cp target/release/heelonvault .
sudo ./scripts/install.sh            # see docs/UPDATE_GUIDE.en.md
```

Official packages (AppImage, DMG, MSI, SBOM) are built by CI on every `vX.Y.Z` tag: see
[RELEASING.en.md](RELEASING.en.md). Windows-specific details:
[RUNBOOK_WINDOWS_PACKAGING.md](RUNBOOK_WINDOWS_PACKAGING.md).

Premium license during development: on Linux, a binary run from `target/debug` or
`target/release` reads `~/.config/heelonvault/license.hvl` instead of `/etc/heelonvault/license.hvl`.

## Troubleshooting

- **Rendering glitches or blank window**: force the GL renderer, `GSK_RENDERER=gl ./scripts/run-dev.sh`.
- **Dev database with an outdated schema**: move it aside, a fresh one is created on next run:
  `mv data/heelonvault-rust-dev.db data/heelonvault-rust-dev.db.bak`.
- **`pkg-config` cannot find `gtk4` or `libadwaita-1`**: the development packages above are
  missing.
- **No logs**: check `HEELONVAULT_LOG_DIR`, then run with `RUST_LOG=debug`.

## Documentation conventions

- `docs/` is published on [doc.heelonvault.heelonys.fr](https://doc.heelonvault.heelonys.fr);
  `docs/internal/` is not (see [README.md](README.md)).
- Bilingual documents start with `Langue : FR | [EN](…)` or `Language: EN | [FR](…)`: the site
  uses that line to detect the language.
- Do not write version numbers or "new in vX" into the documents: the changelog holds the history.
- A new static UI label must also be reapplied in the relevant `refresh_i18n`.
