# Project Architecture (Rust)

Language: EN | [FR](ARCHITECTURE.md)

Documented target version: `1.1.0`

## Overview

HeelonVault now runs as a Rust-only desktop runtime.

- Runtime: repository root
- Desktop UI: GTK4 + libadwaita
- Database: SQLite
- SQL migrations: `sqlx::migrate!` at startup
- Launchers: `scripts/run.sh` (prod), `scripts/run-dev.sh` (dev)

## Logical Layers

```text
UI (gtk4/libadwaita)
  -> Business services
    -> Repositories (SQLx)
      -> SQLite + migrations
```

## Active Structure

HeelonVault is organized as a **Cargo workspace** (Open Core model):

```text
HeelonVault/
├── crates/
│   ├── heelonvault-core/          # Public library (crates.io v1.1.0)
│   ├── heelonvault-app/           # GTK4 binary (Open Core assembler)
│   └── sqlx-shim/                 # Local SQLx shim (publish = false)
├── migrations/                    # SQL migrations applied at startup
├── assets/                        # Embedded GTK assets (CSS, icons)
├── resources/                     # Non-localized resources (fonts)
├── tests/                         # Integration tests
├── docs/                          # Technical documentation
├── Cargo.toml                     # Workspace root (resolver = "2")
├── .cargo/config.toml             # Compiler flags
├── scripts/run.sh                 # Production launcher
├── scripts/run-dev.sh             # Development launcher
├── scripts/install.sh             # Unified installer (OS detection)
├── scripts/install-core.sh        # Shared Linux install library
├── scripts/install-ubuntu.sh      # Ubuntu / Debian installer
├── scripts/install-rhel.sh        # Fedora / RHEL / Rocky Linux installer
├── scripts/remove.sh              # Unified uninstaller (OS detection)
├── scripts/remove-core.sh         # Shared Linux uninstall library
├── scripts/remove-ubuntu.sh       # Ubuntu / Debian uninstaller
├── scripts/remove-rhel.sh         # Fedora / RHEL / Rocky Linux uninstaller
└── docs/
```

> **Premium**: `heelonvault-premium` lives in a separate private Git repository
> (`ppaperso/heelonvault-premium`). It is referenced in `heelonvault-app`
> as an optional git dependency (`features = ["licensing"]`). Community builds
> never access the private repo.

## Startup Flow

1. `main.rs` applies GTK rendering runtime variables (including `GSK_RENDERER`) before starting Tokio.
2. `main.rs` starts the tokio runtime.
3. Open SQLite with `HEELONVAULT_DB_PATH`.
4. Apply SQL migrations.
5. Build repositories and services.
6. Initialize UI and authentication.
7. Load secrets and session policy.

In packaged Linux installs, generated `run.sh` explicitly exports `HEELONVAULT_MIGRATIONS_DIR=/opt/heelonvault/migrations`.
The installer validates copied migrations (filename parity + file content parity) and fails fast if the directory is missing or invalid.

## Main UI View

The main window uses a root `GtkStack` for frequent flows:

- `entries_view`: main secrets list
- `profile_view`: inline profile and security page
- `secret_editor_view`: inline create/edit secret view

Effects:

- sidebar remains visible during profile operations;
- secret creation/editing stays in the center pane;
- profile badge opens a read-only popover with recent login history.

## Runtime Session and Security

- closing main window performs secure logout and returns to login;
- auto-lock uses the same secure logout path;
- login history is persisted in `login_history`;
- master password change via `rotate_master_key_hardened`:
  - owner/shared vault key-envelope rewrap,
  - atomic SQL apply for critical mutations,
  - pre/post rotation validation in `VaultAndSampleSecret` mode;
- `show_passwords_in_edit` preference is persisted per user.

## CSV Import (Pipeline)

The CSV import flow combines guided UX with fault-tolerant processing:

- 3-phase UI: preview, progress, final summary;
- dedicated `import_progress_dialog` for live progress;
- row-by-row service processing with aggregated report (`imported`, `failed`, per-row details);
- reject-report file `csv_import_rejects_*.txt` written in `HEELONVAULT_LOG_DIR` (or `./logs` fallback) when rows are rejected.

## Search

Indexed fields include:

- title, login, email, URL, notes, category, tags, secret type.

Engine behavior:

- case/accent normalization;
- fielded syntax (`email:`, `tag:`, `type:`...);
- light typo tolerance on long tokens.

## Data Paths

- Dev: `data/heelonvault-rust-dev.db`
- Packaged user DB: `~/.local/share/heelonvault/heelonvault-rust.db`
- Legacy Python path (do not modify): `/var/lib/heelonvault-shared`

## Logs

- daily rotation via `tracing-appender`;
- log directory configurable with `HEELONVAULT_LOG_DIR`;
- level configurable with `RUST_LOG`, then `HEELONVAULT_LOG_LEVEL`.

Examples:

```bash
RUST_LOG=info,heelonvault_rust::ui=debug ./scripts/run-dev.sh
HEELONVAULT_LOG_LEVEL=warn ./scripts/run.sh
```

## Validation

```bash
# Community build
cargo check --workspace
cargo test --workspace

# Premium build (requires access to the private repo, or the local patch declared in Cargo.toml)
cargo check -p heelonvault-app --features licensing
```

## Migration Notes

- active runtime and operational scripts are Rust-only;
- legacy artifacts may remain without affecting current execution;
- docs and scripts must stay aligned with Rust-only flows.

## Architecture Decision - Zero-warning supply chain (P2)

Context:

- `cargo audit` reported unmaintained/yanked crates in the legacy PDF dependency chain.
- Project policy targets strict `0 warning` (no permanent allowlist).

Current status:

- ✅ **RUSTSEC-2023-0071 eliminated**: the `rsa` crate (PKCS#1 v1.5 timing side-channel) was removed from the dependency tree when sqlx was upgraded from 0.8 to 0.9 (Phase 5e). `cargo audit` now reports **0 vulnerabilities** across 431 dependencies.
- ⏳ **PDF**: the legacy `genpdf` dependency is still pending replacement (no active advisory today, but poorly maintained chain). The decision to replace it with a minimal internal PDF writer remains in effect.

Decision for PDF:

1. Replace `genpdf` with a maintained PDF architecture or a minimal internal writer.
2. Remove unnecessary transitive dependency features that introduce risky crates.
3. Enforce a CI-blocking policy for advisories, yanked crates, and unmaintained crates.

Implementation constraints:

- keep PDF audit report generation (no feature regression);
- preserve SHA-256 hash + Ed25519 signature in the generated document;
- validate Linux/Fedora/macOS/Windows before merge.

Definition of done (mandatory):

- `cargo audit` => 0 warnings;
- `cargo clippy --all-targets --all-features -- -D warnings` => pass;
- multi-platform CI => green;
- no permanent exception added to policy.
