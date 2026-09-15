# Project Architecture (Rust)

Language: EN | [FR](ARCHITECTURE.md)

Documented target version: `1.2.0-rc.1`

## Overview

HeelonVault runs as a Rust-only desktop runtime. Version **1.2.0-rc.1** introduces major improvements in security, user flows, and infrastructure.

- Runtime: repository root
- Desktop UI: GTK4 + libadwaita
- Database: SQLite
- SQL migrations: `sqlx::migrate!` at startup
- Launchers: `scripts/run.sh` (prod), `scripts/run-dev.sh` (dev)
- **MSRV**: Rust 1.98
- **Edition**: Rust 2024

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
│   ├── heelonvault-core/          # Public library (crates.io v1.2.0-rc.1)
│   ├── heelonvault-app/           # GTK4 binary (Open Core assembler)
│   └── sqlx-shim/                 # Local SQLx shim (publish = false)
├── migrations/                    # SQL migrations applied at startup (19 migrations)
├── assets/                        # Embedded GTK assets (CSS, icons)
├── resources/                     # Non-localized resources (fonts)
├── tests/                         # Integration tests
├── docs/                          # Technical documentation
├── Cargo.toml                     # Workspace root (resolver = "2")
├── clippy.toml                    # Clippy security policy
├── rust-toolchain.toml            # Pinned toolchain at Rust 1.98.0
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

1. `main.rs` applies GTK rendering runtime variables (including `GSK_RENDERER`) **before** starting Tokio.
2. `main.rs` starts the tokio runtime.
3. Open SQLite with `HEELONVAULT_DB_PATH`.
4. Apply SQL migrations (19 migrations in v1.2.0-rc.1).
5. Build repositories and services.
6. Initialize UI and authentication (with refactored bootstrap flow).
7. Load secrets and session policy.

In packaged Linux installs, generated `run.sh` explicitly exports `HEELONVAULT_MIGRATIONS_DIR=/opt/heelonvault/migrations`.
The installer validates copied migrations (filename parity + file content parity) and fails fast if the directory is missing or invalid.

### Bootstrap Flow Refactor (v1.2.0-rc.1)

The initialization flow has been completely refactored with the following components:

- **login_dialog/bootstrap_flow.rs**: 3-step initialization flow management
- **login_dialog/restore_flow.rs**: Restoration flow with recovery key management
- **account_key.rs**: Dedicated service for account key management
- **recovery_service.rs**: Account recovery service with secure validation
- **Migration 0019**: `0019_user_recovery_key_envelope.sql` for persistence of recovery key envelopes

The new system enables:
- Generation and secure storage of recovery key envelopes
- Account restoration via recovery key with two-step validation
- User key management with secure rotation

### Account Key Recovery System

Commit `7cc2556` introduces a complete recovery system:

- **Services**: `AccountKeyService`, `RecoveryService`, `RekeyService`
- **Repositories**: Extensions to `UserRepository` and `VaultRepository` for envelope management
- **Tests**: Complete suite in `tests/account_rekey_integration.rs` (881 lines)
- **Flow**:
  1. Generation of recovery key envelope during bootstrap
  2. Secure storage with AES-256-GCM encryption
  3. Validation and restoration via dedicated dialog

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

### PIN Quick-Unlock (New in v1.2.0-rc.1)

- New `pin_cache_service`: in-memory master-key cache protected by Argon2id (8 MiB, t=3) + AES-256-GCM, never persisted to disk.
- `pin_setup_dialog`: PIN activation and deactivation from the user profile view (4-8 digits).
- `pin_unlock_dialog`: PIN entry dialog displayed when the auto-lock fires.
- Auto-lock integration: a logout now triggers a PIN lock (when a PIN is set) instead of a full disconnect, preserving the session in memory.
- **Security**: 3 attempts maximum per cache, 12-hour hard timeout, `user_id` binding (prevents cross-session replay), random AES-GCM nonce per activation, `zeroize` wipe on `Drop`.
- PIN badge in the title bar with session countdown timer (3 visual states: nominal, warning, critical).

### Master Key Rotation (Hardened)

- `user_service`: hardened `rotate_master_key_hardened` flow enabled with pre/post rotation validation.
- Owner/shared vault key-envelope rewrap now applied through an atomic SQL mutation.
- Sample-secret validation is wired into `VaultAndSampleSecret` mode.
- Manual verification confirmed: master key change succeeds in real application runtime.

### IP-based Brute-Force Protection (New in v1.2.0-rc.1)

- **IP-based Rate Limiting**: New `login_attempts_ip` table to track login attempts by IP address.
- **Configurable policy**: `IpRateLimitPolicy` with `max_attempts` (20 default), `lock_duration_secs` (3600s), and `window_duration_secs` (3600s).
- **Combined service**: `CombinedRateLimitService` integrates username-based (existing) and IP-based (new) rate limiting to block systematic attacks.
- **Automatic cleanup**: `cleanup_expired()` removes expired lock entries.

### Other Security Features

- closing main window performs secure logout and returns to login;
- auto-lock uses the same secure logout path;
- login history is persisted in `login_history`;
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

### MultiVault Mode (New in v1.2.0-rc.1)

- Added **MultiVault** toggle button to the left of the search bar.
- Allows searching across all vaults or only the active vault.
- Replaces the previous auto-detection mode-switch.

Engine behavior:

- case/accent normalization;
- fielded syntax (`email:`, `tag:`, `type:`...);
- light typo tolerance on long tokens;
- unified syntax: `field: value` == `field:value`.

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

Current status (v1.2.0-rc.1):

- ✅ **RUSTSEC-2023-0071 eliminated**: the `rsa` crate (PKCS#1 v1.5 timing side-channel) was removed from the dependency tree when sqlx was upgraded from 0.8 to 0.9 (Phase 5e).
- ✅ **MSRV 1.98 Security Fixes**: `crossbeam-epoch` 0.9.18 → 0.9.20 (RUSTSEC-2026-0204), `webbrowser` 1.2.1 → 1.2.4 (RUSTSEC-2026-0257), `event-listener` 5.4.1 → 5.4.2 (RUSTSEC-2026-0221), replaced yanked versions `chacha20` and `spin`.
- ✅ **`cargo audit` : 0 vulnerabilities, 0 warnings** across all dependencies.
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

## Memory Hardening (New in v1.2.0-rc.1)

### Master Key Lifecycle (Memory PR #1)

- `try_pin_unlock` now returns `Zeroizing<Vec<u8>>`: the zeroize guarantee is enforced by the type system.
- `on_unlocked` callback redesigned as `Option<Zeroizing<Vec<u8>>>`: `Some(key)` on success, `None` on cache exhaustion — eliminates the `Vec::new()` sentinel-value idiom.
- Removed the `key.to_vec()` in `try_pin_unlock` that silently stripped the zeroize guarantee.

### Clippy Security Policy

The [`clippy.toml`](clippy.toml) file globally forbids `unwrap()` / `expect()` calls on all `Result` and `Option` values:

```toml
# excerpt from clippy.toml
disallowed-methods = [
  { path = "std::result::Result::unwrap",  reason = "Use typed errors (thiserror) on sensitive paths" },
  { path = "std::result::Result::expect",  reason = "Avoid panics and secret-leaking failure messages" },
  { path = "std::option::Option::unwrap",  reason = "Handle missing values explicitly" },
  { path = "std::option::Option::expect",  reason = "Handle missing values explicitly" }
]
```

This ensures that no unexpected panic can expose sensitive data in production.

## Open Core Infrastructure

### crates.io Publication

- `heelonvault-core v1.2.0-rc.1` published to [crates.io](https://crates.io/crates/heelonvault-core)
- `heelonvault-premium` extracted into a separate private repository
- `heelonvault-app` references premium via optional git dependency
- Community build (`cargo check --workspace`) never fetches the private repo

### Local Patches

```toml
# Cargo.toml (workspace root)
[patch.crates-io]
heelonvault-core = { path = "crates/heelonvault-core" }

[patch.'ssh://git@github.com/ppaperso/heelonvault-premium.git']
heelonvault-premium = { path = "../heelonvault-premium" }
```

## Operational Scripts

All scripts are maintained in EN/FR:

- `scripts/README.md` and `scripts/README.fr.md`: Script documentation
- `scripts/install.sh`: Unified installer with OS detection
- `scripts/run.sh` / `scripts/run-dev.sh`: Production/development launchers
- `scripts/smoke-test.sh`: Smoke tests for post-installation validation
- `scripts/generate-sbom.sh`: CycloneDX SBOM generation
- `scripts/generate-license.sh`: License file generation
- `scripts/export-legacy-v0.4-to-csv.py`: Migration from v0.4

## Summary of Major Changes in v1.2.0-rc.1

| Category | Change | Impact |
|----------|--------|--------|
| Infrastructure | MSRV Rust 1.96 → 1.98 | Consistent build/lint |
| Infrastructure | Edition 2021 → 2024 | Future compatibility |
| Security | PIN system + auto-lock | Enhanced UX |
| Security | IP rate limiting | Brute-force protection |
| Security | Hardened master key rotation | Compliance |
| Security | cargo-deny integration | Supply-chain hardening |
| Security | Zeroizing for keys | Memory protection |
| UX | Bootstrap flow refactor | User experience |
| UX | Account key recovery | Account recovery |
| UX | MultiVault toggle | Global search |
| UX | 3-step CSV import | Error tolerance |
| UX | PIN badge + timer | Session visibility |
| Architecture | 19 SQL migrations | Updated schema |
| Tests | account_rekey suite | Complete validation |
