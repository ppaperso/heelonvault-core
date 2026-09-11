# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

HeelonVault is a local-first desktop secrets manager written in Rust, built on GTK4/libadwaita
with a SQLite backend (via SQLx). It follows an **Open Core** model: this repository
(`heelonvault-core`) is the public/community codebase; a private sibling repo,
`heelonvault-premium` (checked out as a sibling directory, e.g. `../heelonvault-premium`),
provides premium implementations (multi-user admin, teams, audit reporting, license
verification) that get compiled in via the `premium`/`licensing` Cargo feature. Premium
activation itself happens at **runtime** via a signed license — the premium code is always
present in the shipped binary, but gated behind license checks.

## Commands

```bash
# Dev run (sets HEELONVAULT_DB_PATH to data/heelonvault-rust-dev.db, debug logs to ./logs)
./scripts/run-dev.sh

# Build / lint / test — community edition
cargo check --workspace
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo test --workspace

# Build / check — premium edition (needs heelonvault-premium checked out as a sibling dir,
# or access to its private git repo; see [patch] section in root Cargo.toml)
cargo check -p heelonvault-app --features licensing

# Run a single test
cargo test -p heelonvault-core --test security_crypto
cargo test -p heelonvault-core some_test_fn_name

# Extra security-focused clippy pass (defined as a cargo alias in .cargo/config.toml)
cargo clippy-secure

# Supply-chain checks (required clean before merge per CI policy)
cargo audit
cargo deny check advisories
```

Toolchain is pinned via `rust-toolchain.toml` to Rust `1.98.0`. `.cargo/config.toml` sets
`-Dwarnings` and `-Dunsafe_code` globally, so any warning or `unsafe` block fails the build.

Integration tests live under `crates/heelonvault-core/tests/` (the top-level `tests/` directory
is a stale skeleton, not wired into the workspace). Most integration tests spin up a real SQLite
DB via helpers in `crates/heelonvault-core/tests/common/mod.rs`.

## Zero-technical-debt policy

This repo enforces a strict zero-warning / zero-debt policy (see
`.github/copilot-instructions.md`). Practical implications when writing code here:

- **Never use `.unwrap()` / `.expect()`** on `Result`/`Option` — `clippy.toml` bans them via
  `disallowed-methods` (reason: avoid panics that could leak secrets on sensitive paths). Use
  `thiserror`-based typed errors instead.
- No `#[allow(...)]` to silence lints/security warnings without explicit rationale, owner, due
  date, and a tracked issue.
- No new dependencies with unmaintained/yanked advisories; don't add permanent `cargo audit`/
  `cargo deny` ignore entries.
- All 4 CI platforms (Linux, Fedora, macOS, Windows) must stay green.

## Architecture

Layered flow: `UI (gtk4/libadwaita) -> Services -> Repositories (SQLx) -> SQLite`.

### Workspace layout

- **`crates/heelonvault-core`** — publishable library crate (`heelonvault_core`, on crates.io).
  Contains everything UI-independent and license-independent:
  - `models/` — domain types (`user`, `vault`, `secret_item`, `team`, `audit_log`, `license`).
  - `repositories/` — SQLx-backed data access per aggregate (users, vaults, secrets, teams,
    audit log, IP rate limiting).
  - `services/` — business logic (auth, crypto, password, TOTP, backup, import, access control,
    admin, team, audit, license provider, pin cache, IP rate limiting, federated auth, login
    history). Community-only default implementations live here; premium overrides them via
    `heelonvault-premium`.
  - `config/` — settings and constants.
  - `i18n.rs` — fluent-templates based translations (`locales/`).
  - `errors.rs` — the shared `AppError`/`AccessDeniedReason` types used across services so the
    UI can render localized messages instead of parsing English strings.
  - **Never** initializes a `tracing` subscriber and contains no `unwrap`/`expect` on the
    sensitive paths — those are binary-only concerns.
- **`crates/heelonvault-app`** — the actual GTK4/libadwaita binary (`heelonvault`). This is the
  **Open Core assembler**: `Cargo.toml`'s `[features]` (`premium`, default-on) is the *only*
  place that decides which implementations (community vs. `heelonvault-premium`) get wired up.
  `src/main.rs` (~2000 lines) is the composition root: sets GTK render env vars, boots the
  Tokio runtime, opens SQLite (`HEELONVAULT_DB_PATH`), runs SQLx migrations (path resolved by
  `resolve_migrations_path()`, overridable via `HEELONVAULT_MIGRATIONS_DIR`, packaged installs
  set this to `/opt/heelonvault/migrations`), constructs repositories/services (swapping in
  `heelonvault_premium::services::*Impl` when `feature = "premium"` is enabled, else community
  no-op/default impls), then builds the login flow and main window.
  - `src/ui/windows/main_window/` — the main window. `window/core.rs` is the orchestrator: it
    owns construction order only, delegating UI to `window/views.rs` and the panel builders
    (`center.rs`, `sidebar.rs`, `shell.rs`, `header.rs`) and behavior to `window/events.rs`
    plus one module per feature — `window/editor.rs` (inline secret editor page),
    `window/refresh.rs` (secret list reload), `window/vault_list.rs` (vault sidebar),
    `window/navigation.rs` (profile/users/teams stack pages), `window/pin_badge.rs`,
    `window/i18n_refresh.rs`. `impl_post.rs` holds only the session API that `main.rs` drives
    (auto-lock, PIN cache, logout callbacks).
    The main window is a root `GtkStack`, not modal dialogs: pages are `entries_view`,
    `secret_editor_view`, `profile_view`, and — premium only — `users_view` / `teams_view`.
    Adding a page means registering it on `center_panel.main_stack` and wiring the matching
    sidebar button.
  - `src/ui/windows/main_window/profile_view/` — the inline "Profil & Sécurité" page:
    `sections/*.rs` each build one `adw::PreferencesGroup` and own its `refresh_i18n()`;
    `handlers/*.rs` wire that section's callbacks with dependencies passed as an explicit
    deps struct; `core.rs` assembles them.
  - Widget-holder structs (`SidebarWidgets`, `CenterPanelWidgets`, the section structs) derive
    `Clone` — GTK widgets are refcounted handles, so cloning them into closures is the norm.
  - Language changes re-translate the live UI rather than requiring a restart: any new static
    label must also be re-applied in the relevant `refresh_i18n`.
  - `src/ui/dialogs/` — modal/dialog flows, several structured the same way as `main_window`:
    a directory per dialog (`login_dialog/`, `pin_setup_dialog/`, `pin_unlock_dialog/`,
    `add_edit_dialog/`) split into `core.rs` (state/build), `events.rs` (signal wiring),
    `views.rs`, `types.rs`, `feedback.rs` (user-facing messages), and dedicated flow files
    (e.g. `login_dialog/bootstrap_flow.rs`, `login_flow.rs`, `restore_flow.rs`).
  - `src/ui/widgets/` — reusable widgets (`secret_card.rs`, `password_strength_bar.rs`).
  - `migrations/` — the 18 SQLx SQL migrations actually applied at startup (this is the real
    migrations directory; `docs/ARCHITECTURE.md`'s top-level `migrations/` reference is legacy).
- **`crates/sqlx-shim`** — a local crate published under the name `sqlx` that re-exports
  `sqlx-core`/`sqlx-sqlite` with a pinned feature set. `heelonvault-app` depends on this shim
  (not upstream `sqlx`) directly for its own SQLx usage, while `heelonvault-core` depends on
  upstream `sqlx` normally — see the shim's `Cargo.toml` for the exact feature set if versions
  need bumping.
- **`heelonvault-premium`** (sibling repo, private) — implements premium versions of core
  traits (`admin_service_impl`, `audit_log_service_impl`, `team_service_impl`,
  `psc_auth_service_impl`), plus `license_service` (Ed25519 license verification) and
  `psc_config`. Root `Cargo.toml` has `[patch]` entries so the workspace can resolve
  `heelonvault-premium` from the local sibling checkout instead of its private git URL —
  required to build with `--features licensing` without git credentials for that repo.

### Session/runtime notes worth knowing before touching auth or window flows

- Closing the main window and auto-lock both trigger a clean logout back to the login screen.
- Master password rotation (`rotate_master_key_hardened`) rewraps owner/shared vault key
  envelopes and applies critical mutations atomically, with pre/post validation.
- The main window uses a root `GtkStack` (not modal dialogs) to switch between
  `entries_view`, `profile_view`, and `secret_editor_view`, keeping the sidebar visible during
  profile/edit operations.
- CSV import runs a 3-phase UI (preview → progress via `import_progress_dialog` → summary) with
  per-row tolerant processing; rejected rows are written to
  `HEELONVAULT_LOG_DIR/csv_import_rejects_*.txt`.
- Search indexes title, login, email, URL, notes, category, tags, and secret type, with
  Unicode-normalized matching, field-scoped syntax (`email:`, `tag:`, `type:`), and light fuzzy
  tolerance for long tokens.
- Logs: `tracing-appender` daily rotation, directory via `HEELONVAULT_LOG_DIR`, level via
  `RUST_LOG` (takes priority) then `HEELONVAULT_LOG_LEVEL`. Dev defaults to `debug`/`./logs`;
  packaged prod defaults to `info`/`~/.local/state/heelonvault/logs`.
- Data paths: dev DB at `data/heelonvault-rust-dev.db`; packaged personal install at
  `~/.local/share/heelonvault/heelonvault-rust.db`; enterprise packaged install at
  `/var/lib/heelonvault/heelonvault-rust.db`. A legacy Python deployment's shared data lives at
  `/var/lib/heelonvault-shared` — never touch it from this Rust runtime.

For a deeper narrative walkthrough (startup sequence, decision log for the PDF/audit-report
dependency replacement, etc.), see `docs/ARCHITECTURE.md` / `docs/ARCHITECTURE.en.md`.
