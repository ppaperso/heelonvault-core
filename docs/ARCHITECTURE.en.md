# Project Architecture

Language: EN | [FR](ARCHITECTURE.md)

This document describes how HeelonVault works internally: code layout, startup, key model and
runtime security guarantees. It is written for security reviewers, administrators and
contributors. To install or use the application, see [QUICKSTART.md](QUICKSTART.md) and
[USER_GUIDE.en.md](USER_GUIDE.en.md).

## Overview

HeelonVault is a **local-first** desktop application written in Rust:

- UI: GTK4 + libadwaita;
- storage: local SQLite, schema versioned by SQLx migrations applied at startup;
- encryption: application-side AES-256-GCM, Argon2id key derivation;
- shipped platforms: Linux x86_64 (AppImage), macOS Apple Silicon (DMG), Windows x64 (MSI);
- toolchain: Rust 1.98 (pinned by `rust-toolchain.toml`), 2024 edition.

No server is required: secrets never leave the machine.

## Logical Layers

```text
UI (gtk4/libadwaita)
  -> Business services
    -> Repositories (SQLx)
      -> SQLite + migrations
```

## Code Layout (Open Core model)

```text
heelonvault-core/
├── crates/
│   ├── heelonvault-core/        # Public library (crates.io): models, repositories,
│   │                            # services, i18n, errors — no UI, no licensing
│   ├── heelonvault-app/         # GTK4 binary: assembles core + premium
│   │   ├── src/                 # main.rs (composition root), ui/ (windows, dialogs, widgets)
│   │   ├── migrations/          # SQL migrations applied at startup
│   │   ├── assets/              # Embedded CSS, icons, images (GResource)
│   │   └── wix/                 # Windows installer definition (MSI)
│   └── sqlx-shim/               # Local SQLx shim (publish = false)
├── linux/  macos/               # AppImage launcher and macOS bundle files
├── scripts/                     # Dev launcher, Linux system install, SBOM
├── docs/                        # Published documentation (docs/internal/: maintainers)
├── Cargo.toml                   # Workspace
├── clippy.toml                  # Security Clippy policy
└── rust-toolchain.toml          # Pinned toolchain
```

- `heelonvault-core` holds everything that depends neither on the UI nor on licensing. It never
  initializes a `tracing` subscriber.
- `heelonvault-app` is the **Open Core assembler**: the `premium` Cargo feature (on by default) is
  the only place that chooses between community implementations and `heelonvault-premium` ones.
- `heelonvault-premium` is a **proprietary** component in a separate private repository
  (multi-user administration, teams, audit report, license verification). Its code ships in the
  binary; it is activated at runtime by a verified signed license. Its source can be opened for
  audit under a non-disclosure agreement (see [SECURITY.md](../SECURITY.md)).

## Startup Flow

1. `main.rs` sets the GTK rendering variables (including `GSK_RENDERER`) **before** starting Tokio.
2. Tokio runtime and logging are initialized.
3. The SQLite database is opened (path resolved as described in [Data Paths](#data-paths)).
4. SQL migrations are applied. The directory is looked up in this order:
   `HEELONVAULT_MIGRATIONS_DIR`, next to the executable, then the current directory. The
   AppImage, DMG and MSI packages point this variable at their bundled migrations.
5. Repositories and services are built (premium implementations when the feature is on).
6. First run: setup wizard. Otherwise: sign-in screen, then main window.

### End of the setup wizard

A successful first-run setup does **not** open a session. `bootstrap_flow.rs` wipes the account
key (drop of `BootstrapResult`), sets the flag shared with the `close_request` handler (otherwise
closing would be treated as a cancellation and quit the app), closes the wizard, then calls
`on_bootstrap_completed(username)`. `main.rs` switches back to sign-in mode and shows a regular
`LoginDialog`, on which `show_account_created()` pre-fills the username.

## Key Model

- **Account key** (`services/account_key.rs`): a random 32-byte key that wraps every vault key of
  an account. It is only stored encrypted:
  - under the master password (password envelope, `auth_service`);
  - under the 24-word recovery phrase (domain-separated derivation).
- **Vault keys**: one per vault, wrapped by the owner's account key and, for a shared vault, by
  each member's.
- **Secrets**: AES-256-GCM encrypted under their vault key.

Consequences:

- **Master password change** (`UserService::change_master_password`): the account key does not
  change, only the password envelope is rewritten. Vaults and secrets are not re-encrypted. An
  older account (pre-account-key format) is migrated during this change: vault keys, TOTP secret
  and recovery material are rewrapped in a single transaction (`rekey_service.rs`).
- **Recovery**: the recovery phrase reopens the account key and lets the user set a new password
  without losing any vault (`recovery_service.rs`, `login_dialog/restore_flow.rs`). A stored
  verifier checks the phrase without keeping it.

## Main UI View

The main window uses a root `GtkStack` to avoid modal dialogs for the most frequent flows:

- `entries_view`: main secret list;
- `secret_editor_view`: inline create / edit;
- `profile_view`: `Profile & Security` page;
- `users_view` / `teams_view`: administration (premium).

The sidebar stays visible during profile and edit operations. The profile badge opens a
read-only popover with recent sign-in history.

### Secret display: cards or list

`entries_view` relies on a single `GtkFlowBox` (`secret_flow`) whatever the display mode (`SecretViewMode::Grid` / `List`, defined in `ui/view_preferences.rs`):

- `center::apply_view_mode_to_flow` configures the container (multi-column grid, or one full-width column + `main-secret-list` CSS class);
- `SecretCard::new(data, mode)` builds a card or a compact row from the same building blocks (title, badges, actions), keeping the same action buttons;
- the current mode lives in `FilterRuntime::view_mode`; the toggle (`events::setup_view_mode_handlers`) persists the choice then rebuilds the widgets from the last load (`SecretListCallbacks::rerender`), with no query and no decryption. A full reload only happens when nothing has been loaded yet.

Filtering, sorting, search, counters and keyboard shortcuts are therefore mode-independent. The choice is persisted per installation in `ui_view_preferences.json` (same directory as `ui_main_window_state.json`).

### Secret list: no plaintext value in the UI

`window/refresh.rs` builds a single `SecretFlowContext` shared by two callbacks (`SecretListCallbacks`): `reload` (database) and `rerender` (widgets only). `secret_flow.rs` works in three stages:

1. **Load** (`refresh_secret_flow`, worker thread): for each secret, `get_secret()` decrypts the value, from which only `has_secret`, `is_weak` and a SHA-256 fingerprint are derived; the plaintext stays in the service's `SecretBox` (wiped on drop). `finalize_rows` flags duplicates then drops the fingerprints **inside the loader thread**. The master key copy handed over is a `Zeroizing<Vec<u8>>`.
2. **Render** (`render_secret_rows`): builds cards or rows from `LoadedSecrets` (metadata + live usage counters, `Rc<Cell<u32>>`), kept in the context for `rerender`.
3. **On-demand copy** (`PasswordCopier::copy`): snapshot of the session key (refused when locked), `open_vault_for_user` (re-checks access — a revoked share can no longer be copied), `get_secret`, copy through `sensitive_clipboard`, then wipe. The button is disabled and an `in_flight` flag blocks re-entry (`Ctrl+C` emits `clicked` even on an insensitive button).

Invariant: `SecretRowView` / `SecretRowData` **never** hold a secret value. Login and URL are not encrypted (`metadata_json`) and can therefore be copied directly. The per-copy overhead (a few SQLite queries + one AES decryption) is imperceptible.

Known limitation: `get_secret()` re-reads each secret from the database although `list_by_vault()` just loaded it (N+1 queries). Fixing it needs a new method on the public `SecretService` trait of `heelonvault-core`: deferred to a dedicated change.

### Clipboard exposure indicator

`ui/sensitive_clipboard.rs` publishes an `Exposure` (`Idle`, `Decrypting`, `InClipboard { kind, copied_at, expires_at }`) on every change: copy (`copy_sensitive(text, SensitiveKind, delay)`), expiry, `clear_now()`, clipboard replaced by another application (`changed` signal with `is_local() == false`: the value is wiped at once, leaving the new content alone), and decryption in flight (`begin_decrypting()` returns an RAII guard held by `PasswordCopier`).

- `subscribe()` calls the listener at once, then on every change, until it returns `ControlFlow::Break`. Notifications are emitted **after** the internal state is released (a listener may call back `clear_now()`), and subscriptions made during a notification are kept.
- `ui/widgets/clipboard_indicator.rs`: header bar button (icon + ring drawn with cairo in a `DrawingArea`). It only holds weak references, so the subscription ends with the main window rebuilt at each sign-in. The ring is animated (tick callback) only while something is exposed; idle, it costs nothing. Clicking calls `clear_now()`.
- Labels only claim what the application controls: never "no secret in memory".

## Runtime Session and Security

### PIN Quick-Unlock

- `pin_cache_service`: in-memory cache of the master key, protected by Argon2id (8 MiB, t=3) +
  AES-256-GCM, **never persisted to disk**.
- `pin_setup_dialog`: enable / disable from the profile (4 to 8 digits).
- `pin_unlock_dialog`: PIN entry when unlocking after auto-lock.
- Safeguards: 3 attempts per cache, hard 12 h expiry, bound to `user_id`, random AES-GCM nonce per
  activation, `zeroize` wipe on `Drop`.
- PIN badge in the title bar with a session timer (nominal, warning, critical).

### Brute-force protection

- Per username: progressive delay between failed sign-ins.
- Per address (`ip_rate_limit_service`, `login_attempts_ip` table): default `IpRateLimitPolicy`
  of 20 attempts per one-hour window, then a one-hour lock. Both policies are combined by
  `CombinedRateLimitService`; `cleanup_expired()` purges expired locks.
- TOTP: a valid code cannot be replayed immediately.

### End of session

- Closing the main window and auto-lock both trigger a clean logout back to the sign-in screen
  (or to PIN unlock when enabled).
- Sign-in history is stored in `login_history`.

### Memory hardening

- Keys travel in types that wipe themselves on drop (`SecretBox`, `Zeroizing`). `try_pin_unlock`
  returns a `Zeroizing<Vec<u8>>`; the unlock callback receives `Option<Zeroizing<Vec<u8>>>`.
- `clippy.toml` bans `unwrap()` / `expect()` on `Result` and `Option`: a panic must not be able
  to expose sensitive data in an error message.
- `.cargo/config.toml` enforces `-D warnings` and `-D unsafe_code` across the workspace.

## CSV Import

- 3-phase UI: preview, progress (`import_progress_dialog`), final summary.
- Row-by-row, error-tolerant processing with an aggregated report (imported, failed, per-row
  details).
- Rejected rows are written to `csv_import_rejects_*.txt` in the log directory.

## Search

Indexed fields: title, login, email, URL, notes, category, tags, secret type.

- case / accent normalization (Unicode);
- field-scoped syntax (`email:`, `tag:`, `type:`…), `field: value` equals `field:value`;
- light typo tolerance for long enough terms;
- **MultiVault** toggle left of the search bar: search all vaults or only the active one.

## Data Paths

The database path can always be forced with `HEELONVAULT_DB_PATH`, the log directory with
`HEELONVAULT_LOG_DIR`. Without them:

| Installation | Database | Logs |
| ------------ | -------- | ---- |
| Windows (MSI) | `%LOCALAPPDATA%\Heelonys\HeelonVault\data\heelonvault\data\heelonvault-rust.db` | `…\heelonvault\logs\` |
| macOS (DMG) | `~/Library/Application Support/fr.Heelonys.HeelonVault/heelonvault/data/heelonvault-rust.db` | `…/heelonvault/logs/` |
| Linux (AppImage, "Personal" system install) | `~/.local/share/heelonvault/heelonvault-rust.db` | `~/.local/state/heelonvault/logs/` |
| Linux ("Enterprise" system install) | `/var/lib/heelonvault/heelonvault-rust.db` | `/var/log/heelonvault/` |
| Development (`scripts/run-dev.sh`) | `data/heelonvault-rust-dev.db` | `./logs/` |

On Linux, the launchers (the AppImage's AppRun, the system install's `run.sh`) set these
variables; on Windows and macOS the application applies these defaults itself.

## Logs

- Daily rotation via `tracing-appender`; JSON files `heelonvault_YYYYMMDD.log`.
- Level: `RUST_LOG` (takes priority), else `HEELONVAULT_LOG_LEVEL`. Default: `info` for a release
  build, `debug` for a development build. Sensitive modules (crypto, secrets, authentication,
  vaults) stay capped at `warn` unless explicitly named in the filter.
- Logs contain no secret value (enforced by the `privacy_no_secret_in_logs` test suite).

## Supply Chain

- **Zero-warning** policy: `cargo audit` and `cargo deny check` must be clean before any merge,
  with no permanent exception.
- The former PDF chain (`genpdf`) was removed: the premium audit report uses a minimal internal
  PDF writer that keeps the SHA-256 digest and the Ed25519 signature.
- CycloneDX SBOM of the shipped binary, published and attested with every release: see
  [SECURITY.md](../SECURITY.md#13-supply-chain-security-and-sbom).
- The `heelonvault-core` crate follows semver: `cargo semver-checks` in CI rejects an API break
  without a major version.

## Tests

Integration tests live in `crates/heelonvault-core/tests/` and run against a real SQLite
database. They cover cryptography, authentication and brute force, access control, SQL
injection, absence of secrets in logs, GDPR (erasure, portability), backups and account-key
migration (`account_rekey_integration`). To run them, see
[DEVELOPMENT.md](internal/DEVELOPMENT.md).
