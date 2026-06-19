# Changelog — HeelonVault

Language: EN | [FR](CHANGELOG.md)

All notable changes are documented here, in descending version order.
Format inspired by [Keep a Changelog](https://keepachangelog.com/).

---

## [Unreleased] — Target v1.2.0

> Release note: `v1.1.0` (and `v1.1.0-rc.1`) is frozen. Changes below are scoped for `v1.2.0`.

### Infrastructure — Edition 2024 and Rust 1.96

- Migrated workspace crates to `edition = "2024"`.
- Aligned `rust-version` to `1.96` for `heelonvault-core`, `heelonvault-app`, and `sqlx-shim`.
- Added `rust-toolchain.toml` (`1.96.0`) to keep local and CI build/lint environments consistent.
- Applied Rust 2024 compatibility hardening: replaced premium test-time `std::env::*_var` mutation patterns with explicit configuration wiring (no global environment mutation).
- Reached zero clippy warnings with 1.96 under `-D warnings` across core/premium scope.

### Dependencies

- Updated `Cargo.lock` dependencies to latest Rust 1.96-compatible versions.
- Updated the git reference of `heelonvault-premium` consumed by the core app.
- Supply-chain validation preserved: `cargo audit` (exit 0) and `cargo deny check advisories` (ok).

### Infrastructure — Open Core polyrepo split (Phase 5f)

- `heelonvault-core v1.1.0` published to [crates.io](https://crates.io/crates/heelonvault-core): the core library is now a public, reusable artifact.
- `heelonvault-premium` extracted into a separate private repository (`ppaperso/HeelonVault-Premium`): premium code is no longer exposed in the public repository.
- `heelonvault-app` references premium as an optional git dependency; the community build (`cargo check --workspace`) never fetches the private repo.
- `.cargo/config.toml`: local patches configured for joint development (no network required locally).
- VSCode workspace updated to multi-root mode (HeelonVault + HeelonVault-Premium).

### Infrastructure — SQLx 0.8 → 0.9 upgrade and RSA elimination (Phase 5e)

- `sqlx` dependency updated from `0.8` to `0.9` in `heelonvault-core` and `sqlx-shim` (API-compatible, no application code changes).
- **RUSTSEC-2023-0071 eliminated**: the `rsa` crate (timing side-channel on PKCS#1 v1.5 decryption) was a transitive dependency of sqlx 0.8; it is no longer present in the dependency tree.
- `cargo audit`: 0 active vulnerabilities (431 dependencies scanned).

### Infrastructure — MSRV 1.94 → 1.95

- `rust-version = "1.95"` aligned across all 4 workspace members (`heelonvault-core`, `heelonvault-app`, `heelonvault-premium`, `sqlx-shim`).
- Stable toolchain updated to `1.95.0` (rustc 59807616e, 2026-04-14).
- No new Clippy lints introduced by 1.95.0.

### Dashboard UX — cards and keyboard productivity

- Secret cards now follow a clearer mouse/keyboard flow:
  - single click = select,
  - double click = open editor.
- Removed edit/delete controls from cards to reduce visual noise; maintenance actions now go through dedicated screens.
- Card ordering is now usage-driven (`usage_count` descending) to prioritize frequently used secrets.
- Added/refined card badges: strength, incomplete, duplicate, usage.
- Global shortcuts on the active card: `Ctrl+C` (copy secret), `Ctrl+L` (copy login), `Ctrl+U` (open URL).

### Health-related secrets — manual marker and local detection

- Added persistent "Health data access" marker in the create/edit form.
- Added high-confidence local auto-detection to classify relevant health secrets without user configuration.
- Added quick search shortcut `#sante` to target marked/detected health secrets.
- Added a "Sante" badge on matching cards for faster visual identification.

### PIN status badge and session countdown timer

- Added a clickable **"PIN active"** badge in the title bar, kept in real-time sync with the PIN cache state.
- Three progressive visual states based on remaining session time:
  - **Nominal** (> 2 h): standard semi-transparent white badge.
  - **Warning** (≤ 2 h, > 15 min): amber border and text — visual cue that session renewal is advisable.
  - **Critical** (≤ 15 min): amber-filled badge, text changes to "PIN · Xm", 2-second `pulse` animation.
- Permanent tooltip on the badge: _"PIN-secured session — Expires in Xh Ym"_.
- 60-second GLib timer managed by an inner closure (`glib::timeout_add_local`): cleanly cancelled via `SourceId::remove()` on every exit path (logout, cache exhaustion, quit).
- Hermetic lifecycle: badge and timer are reset immediately on every exit path; double-remove is prevented by setting `SourceId` to `None` before returning `Break`.
- Added `PinCache::remaining(hard_timeout) -> Duration` method (returns `ZERO` if already expired).
- Fix: badge text was unreadable against the dark title-bar background (CSS rules for `headerbar button.header-pin-badge label`).
- Fix: badge remained "PIN active" after automatic cache expiry (synced via `on_pin_state_cb`).
- Fix: missing "Quit application" button on the PIN unlock dialog.
- FR/EN localization: new key `pin-tooltip-secure`.

### PIN Quick-Unlock

- New `pin_cache_service`: in-memory master-key cache protected by Argon2id (8 MiB, t=3) + AES-256-GCM, never persisted to disk.
- `pin_setup_dialog`: PIN activation and deactivation from the user profile view (4–8 digits).
- `pin_unlock_dialog`: PIN entry dialog displayed when the auto-lock fires.
- Auto-lock integration: a logout now triggers a PIN lock (when a PIN is set) instead of a full disconnect, preserving the session in memory.
- Security: 3 attempts maximum per cache, 12-hour hard timeout, `user_id` binding (prevents cross-session replay), random AES-GCM nonce per activation, `zeroize` wipe on `Drop`.
- Full FR/EN localization for all PIN messages (entry, error, limit, timeout, enable/disable).

### Memory hardening — master key lifecycle (memory PR #1)

- `try_pin_unlock` now returns `Zeroizing<Vec<u8>>`: the zeroize guarantee is enforced by the type system.
- `on_unlocked` callback redesigned as `Option<Zeroizing<Vec<u8>>>`: `Some(key)` on success, `None` on cache exhaustion — eliminates the `Vec::new()` sentinel-value idiom.
- Removed the `key.to_vec()` in `try_pin_unlock` that silently stripped the zeroize guarantee.

### Master key rotation (hardening)

- `user_service`: hardened `rotate_master_key_hardened` flow enabled with pre/post rotation validation.
- Owner/shared vault key-envelope rewrap now applied through an atomic SQL mutation.
- Sample-secret validation is wired into `VaultAndSampleSecret` mode.
- Manual verification confirmed: master key change succeeds in real application runtime.
- Automated verification confirmed: broad `cargo test` run is green.

### CSV import UX (premium)

- `profile_view`: redesigned CSV import into a 3-step user flow:
  - file preview (detected/importable/manual-review rows),
  - visible progress during import,
  - final detailed summary including rows that require manual follow-up.
- `import_service`: switched to row-level fault tolerance with a structured summary report (`imported`, `failed`, per-row details) instead of one opaque global failure.
- `ui/dialogs/import_progress_dialog`: new dedicated import progress dialog.
- Full FR/EN localization for new import messages (preview, progress, summary, error details).

### Legacy migration v0.4 -> v1.1

- Added `scripts/export-legacy-v0.4-to-csv.py` to export legacy databases to a CSV format compatible with v1.1 import.
- Supports legacy layouts via `--profile`, `--workspace-uuid`, or explicit `--db-path` + `--salt-path`.

### v1.1.0 technical debt (issues #5, #6, #7, #8, #9)

- `main`: moved `env::set_var("GSK_RENDERER", "gl")` before Tokio runtime initialization.
- `import_service`: removed the global `clippy::disallowed_methods` bypass and hardened CSV parsing (explicit required-field extraction with typed validation errors).
- `main`: split orchestration into focused units (startup flags, runtime/UI execution, service builders) to reduce complexity and duplication.
- `team_service`: decomposed `share_vault_with_team` and `rotate_vault_key` into internal helpers (member-key resolution, share persistence, audit recording).

### API documentation (issue #10)

- Added/completed `///` docs on public service traits (`vault_service`, `secret_service`) to clarify preconditions, error paths, and security contracts.

### Validation

- Automated validation run after refactor: `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`.

### Search UX — multi-vault toggle and premium help popover

- `shell.rs`: added **MultiVault** toggle button to the left of the search bar; replaces the previous auto-detection mode-switch.
- Fix: removed `selectable(true)` from the help popover label — it blocked focus return to the main window after a right-click (apparent freeze).
- Fix: `parse_search_terms` — `field: value` syntax (space after colon) now behaves identically to `field:value`.
- Redesigned help (`?`) popover: three structured sections (`caption-heading` + `dim-label` + `monospace`), `help-browser-symbolic` icon, fixed 348 px width, `autohide(true)`.
- Refactor: `ContentShell` struct with a centralized `refresh_i18n` closure; `new_body.inc` delegates all shell i18n updates through a single call.
- Structured FR/EN i18n keys: `main-search-help-no-prefix-title`, `main-search-help-no-prefix-body`, `main-search-help-prefix-title`, `main-search-help-fields`, `main-search-help-examples`, `main-search-help-fuzzy`.

### Dependency security (issue #13)

- Updated the transitive TLS chain in `Cargo.lock`: `rustls-webpki` `0.103.10` -> `0.103.13`, `rustls` `0.23.37` -> `0.23.40`.
- Lockfile verification: vulnerable `rustls-webpki 0.103.10` is no longer present.
- Post-fix validation executed: `cargo check`, `cargo test --locked --all-targets --no-run`, `cargo clippy --all-targets --all-features -- -D warnings`.
- Expected security impact: fixes the high Dependabot alert and the two related low alerts for `rustls-webpki` once pushed to `main`.

## [1.1.0] — 2026-05-13

### Authentication and 2FA security

- Hardened password change flow: old/new password comparison now uses constant-time equality in `auth_service`.
- Hardened login failure accounting: invalid passwords now correctly increment `auth_policy.failed_attempts` in the login path.
- Added server-side TOTP replay guard: a code that was already accepted cannot be reused immediately in the same time window.

### Brute-force mitigation and lock UX

- Added progressive retry backoff in `auth_policy_service` (bounded exponential growth), complementing the existing lock window behavior.

### CSV import hardening

- Added security limits for CSV import: max file size, max row count, and max field length.
- Added strict URL validation for import: only `http://` and `https://` schemes are accepted.

### Sensitive file permissions

- Backup exports (`.bak` and `.hvb`) now apply owner-only permissions (`0600` on Unix).
- Restored SQLite files now apply owner-only permissions (`0600` on Unix).

### Tests

- Added unit tests for:
  - identical password change rejection,
  - CSV URL validation,
  - auth policy backoff calculations.
- Full validation run completed: `cargo test` passes after changes.

## [1.0.4] — 2026-04-14

### Dependency security

- Fixed the `rand` Dependabot advisory path: removed vulnerable `rand 0.8.5` from the resolved graph and pinned `rand 0.9.3`.
- Migrated BIP39 recovery-key generation to explicit `getrandom` entropy, without relying on `bip39/rand` feature wiring.
- Moved off the `sqlx` aggregator crate to a SQLite-only shim (`crates/sqlx-shim`) to remove unnecessary `sqlx-mysql` / `sqlx-postgres` transitive dependencies from lockfile and SBOM.

### SBOM, attestation, and release

- Productionized CycloneDX 1.4 SBOM flow: local generator `scripts/generate-sbom.sh`, CI freshness gate (`check-sbom`), and release publication job (`generate-sbom-artifact`).
- Release now publishes `sbom.cyclonedx.json` + `sbom.cyclonedx.json.sha256` with build provenance attestation (`actions/attest-build-provenance@v4`).
- Harmonized Linux/Windows/macOS release jobs with checksums and GitHub Release uploads via `gh` CLI.

### CI/CD and macOS reliability

- Fixed macOS `.app/.dmg` packaging path issues (GDK-Pixbuf staging, Homebrew symlink resolution, DMG checksum via `shasum -a 256`).
- Added explicit `gdk-pixbuf` Homebrew dependency in macOS CI/release workflows.
- Removed remaining Node 20 deprecation warnings by dropping Homebrew caching step based on `actions/cache@v4` in macOS jobs.

### Documentation and compliance

- Updated `README.md` / `README.en.md` to 1.0.4 and documented signed SBOM publication.
- Realigned `THIRD_PARTY_LICENSES.md` and `sbom.cyclonedx.json` with the final dependency graph.

## [1.0.3] — 2026-04-10

### UI refactoring

- Finalized the split of large Rust screens into dedicated modules for `login_dialog`, `main_window`, `profile_view`, and related flows.
- Kept the maintainability constraint satisfied: no active UI Rust file above 800 lines.
- Extracted sizing helpers and UI subcomponents to reduce local coupling and clarify responsibilities.

### Technical cleanup

- Removed unreferenced intermediate split files left behind during the refactor.
- Verified that `assets/images/user-guide` images remain documentation-only assets and are not unintentionally bundled at runtime.

### Validation-

- Validated the release state with `cargo check`, `cargo clippy`, `cargo test`, and `cargo fmt --all -- --check`.

### Version

- Bumped application and documentation version to **1.0.3**.

## [1.0.1] — 2026-04-06

### Product documentation

- Added a **bilingual user guide** (`docs/USER_GUIDE.md` and `docs/USER_GUIDE.en.md`) with an end-user manual tone.
- Integrated real UI screenshots across screen-by-screen sections (bootstrap, login, dashboard, secret creation forms, profile/security, import/export, user/team administration, trash).
- Structured the guide with a table of contents and formal screen/capture numbering.

### CI/CD and Linux packaging

- Added a shared smoke test (`scripts/smoke-test.sh`) with `--install/--remove`, permissions checks, and desktop-entry validation.
- Hardened CI/Release workflows: Rust cache, Fedora container job, external `.sha256` checksum asset, build provenance attestation, and core script inclusion in `dist/`.

### Version1

- Bumped application and documentation version to **1.0.1**.

## [1.0.0] — 2026-04-02

### Stable release

- Official move to **1.0.0** (stable release), removing the beta suffix from the application version and reference documentation.

### PDF audit report

- Simplified premium visual header: removed the gold framed panel.
- New black primary title: **REGISTRE DE TRAÇABILITÉ DES ACCÈS**.
- Signed audit log exported as an actionable table (date, action, actor, target, detail).

### Traceability and readability

- Actor identity resolution now prefers display name / username in exports.
- Audit targets enriched with vault names and secret titles when available.
- `secret.created` event now includes secret title in audit detail payload.

## [0.9.4-beta] — 2026-04-01

### License

- Switched from proprietary Source-Available license to **Apache 2.0**: free to use, modify, and redistribute; HEELONYS copyright and brand retained.

### Application license system (LicenseService)

- Ed25519 signature verification for signed license files (`~/.config/heelonvault/license.hvl` in dev, `/etc/heelonvault/license.hvl` in prod).
- JSON format with `payload` field (JSON object or serialized string) and `signature` (128-char hex or base64).
- Automatic fallback to **Community** license when no file is present or verification fails.
- Automatic tolerance of whitespace and `0x` prefix in hex values (`sanitize_hex_input`).
- Audit log entries `LicenseCheckSuccess` / `LicenseCheckFailure` at application startup.

### License badges in the UI

- **"Licence free"** / **"Licence pro — CLIENT"** badge in the login hero section, visible before authentication.
- License badge in the main-window header bar (next to the BETA badge).
- High-visibility CSS style `.login-license-badge` (teal gradient).
- i18n keys `license-status-community`, `license-status-professional`, `license-status-invalid` added in FR and EN.

---

## [0.9.3-beta] — 2026-03-31

### Security dashboard

- Security dashboard window rendered via WebKitGTK (WebView-first, no GTK fallback blocks).
- Global vault score computed in real time with `zxcvbn` evaluation.
- Dedicated FR and EN translations for all dashboard labels and states.

### Login history

- Each successful login is recorded in the `login_history` table (migration 0007).
- History displayed in the `Profile & Security` view.

### TOTP 2FA activation

- Guided TOTP activation via QR code in `Profile & Security`.
- Mandatory first-code verification before activation is confirmed.
- TOTP secret stored encrypted in the database (migration 0009).

### Fixes and robustness

- Secret restore from trash: atomic transaction with automatic parent vault restore when needed (avoids "invisible secret" state).
- Vault resolution fixed in the multi-vault secret edit dialog.
- Password envelope persistence corrected on reload.

---

## [0.9.2-beta] — 2026-03-27

### Internationalization and UX

- Login language selector replaced with FR/EN flags.
- Fixed a UI freeze during language switching on the login screen.
- Harmonized live i18n refresh across main-window global areas (sidebar, tooltips, placeholders, stack titles).
- User language preference now persists and applies live from `Profile & Security`.

### Installer, CI/CD, and release reliability

- Installer hardened with explicit validation of critical artifacts (`run.sh`, desktop entries).
- Dual desktop-entry installation (`com.heelonvault.rust.desktop` and `heelonvault.desktop`) for environment compatibility.
- Installer smoke test added to the release workflow.
- Dedicated CI pipeline (`.github/workflows/ci.yml`): formatting, lint, build, test compilation, desktop validation, smoke test.

### Bootstrap wizard, recovery key, and secure backup

- 3-step first-admin setup wizard in the login dialog: identity → oath (24-word phrase) → pending.
- 24-word BIP39-style mnemonic phrase generated at bootstrap via `BackupService::generate_recovery_key()`.
- Mandatory spot-check of 2 randomly drawn words before confirmation.
- Clipboard copy with automatic wipe after 60 seconds.
- Recovery key re-export available from `Profile & Security` for any admin.
- `BackupApplicationService`: RBAC access control on `.hvb` export and import operations.
- Audit log introduced (table `audit_log`, migration 0013).

### Team sharing, RBAC, and admin UX

- Fixed team vault sharing: member key now derived from `password_envelope` when no explicit key is provided.
- Fail-fast protection: explicit failure when no member receives a vault key.
- Explicit vault picker added to the team sharing dialog.
- ADMIN badge in the header next to the connected identity.
- Owner-side shared-state visibility for owned vaults.
- FR badge labels normalized to uppercase.
- i18n cleanup: removed obsolete `main-vault-shared-badge` key.

### Bilingual documentation

- FR/EN coverage across all operational Markdown documentation.
- Central bilingual documentation index in `docs/README.md`.

---

## [0.9.1-beta] — 2026-03-01

### Initial Rust architecture

- Full migration from Python to Rust (GTK4 + libadwaita).
- Service/repository/model layer in Rust with `sqlx` and 9 initial migrations.
- Argon2id authentication, AES-256-GCM encryption, TOTP RFC 6238.
- Multi-user with isolated vaults per user.
- Multi-field search with Unicode normalization.
- Rotating structured JSON logs via `tracing`.
- Security Clippy policy (`clippy.toml`) forbidding `unwrap()`/`expect()` on sensitive paths.
