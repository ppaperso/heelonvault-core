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

```text
HeelonVault/
├── src/
│   ├── main.rs
│   ├── ui/
│   ├── services/
│   ├── repositories/
│   ├── models/
│   ├── config/
│   └── errors.rs
├── migrations/
├── tests/
├── Cargo.toml
├── scripts/run.sh
├── scripts/run-dev.sh
├── scripts/install.sh              # Unified installer (OS detection)
├── scripts/install-core.sh         # Shared Linux install library
├── scripts/install-ubuntu.sh               # Ubuntu / Debian installer
├── scripts/install-rhel.sh                 # Fedora / RHEL / Rocky Linux / AlmaLinux installer
├── scripts/remove.sh               # Unified uninstaller (OS detection)
├── scripts/remove-core.sh          # Shared Linux uninstall library
├── scripts/remove-ubuntu.sh                # Ubuntu / Debian uninstaller
├── scripts/remove-rhel.sh                  # Fedora / RHEL / Rocky Linux / AlmaLinux uninstaller
└── docs/
```

## Startup Flow

1. `main.rs` starts the tokio runtime.
2. Open SQLite with `HEELONVAULT_DB_PATH`.
3. Apply SQL migrations.
4. Build repositories and services.
5. Initialize UI and authentication.
6. Load secrets and session policy.

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
- `show_passwords_in_edit` preference is persisted per user.

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
cargo check
cargo test
```

## Migration Notes

- active runtime and operational scripts are Rust-only;
- legacy artifacts may remain without affecting current execution;
- docs and scripts must stay aligned with Rust-only flows.

## Architecture Decision - Zero-warning supply chain (P2)

Context:

- `cargo audit` reports unmaintained/yanked crates in the legacy PDF dependency chain.
- Project policy now targets strict `0 warning` (no permanent allowlist).

Decision:

1. Replace legacy `genpdf` dependency with a maintained PDF architecture.
2. Remove unnecessary transitive dependency features that introduce risky crates.
3. Enforce a CI-blocking policy for advisories, yanked crates, and unmaintained crates.

Options evaluated:

- legacy `genpdf`: rejected (obsolete chain, persistent audit warnings)
- maintained `genpdf` forks: possible fast transition, but still tied to legacy PDF transitive stack
- minimal internal PDF writer (no external PDF dependency): selected target to guarantee `cargo audit = 0 warning`

Implementation constraints:

- keep PDF audit report generation (no feature regression);
- preserve SHA-256 hash + Ed25519 signature in the generated document;
- validate Linux/Fedora/macOS/Windows before merge.

Definition of done (mandatory):

- `cargo audit` => 0 warnings;
- `cargo clippy --all-targets --all-features -- -D warnings` => pass;
- multi-platform CI => green;
- no permanent exception added to policy.
