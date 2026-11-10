# AGENTS.md — HeelonVault working guide (canonical)

This file is the **single source of truth** for coding agents working on HeelonVault.
Keep the standard filename `AGENTS.md` (plural, uppercase); do not create an `AGENT.md` duplicate.
Per-agent entry points are thin pointers to this file:

- **Mistral Vibe**: `.vibe/skills/heelonvault-dev/SKILL.md` (this `AGENTS.md` at the repo root is also auto-loaded by Vibe)
- **Claude Code**: `CLAUDE.md` at the root (auto-loaded) and `.claude/skills/heelonvault-dev/SKILL.md`
- **GitHub Copilot**: `.github/copilot-instructions.md`

When the project changes, edit **this file**; keep the per-agent files as pointers.
Read this guide before editing; consult the linked references only as needed. If a command or
policy differs from the checked-out implementation or CI, report the discrepancy rather than
silently bypassing a gate.

## Safe working procedure

- Inspect `git status --short --branch` in this repo and in Premium before changes. Preserve
  pre-existing edits, untracked files, local databases, logs and recovery material.
- Do not reset, clean, prune, rewrite history, delete branches, commit, push or merge unless
  requested. Never open or migrate a real user database to validate a change; use synthetic
  fixtures and temporary test databases.
- Make scoped changes, run the relevant checks, and report exact commands/results, skipped
  checks and remaining blockers. A passing build is not proof of graphical or package certification.

## Project snapshot

- HeelonVault is a **local-first** desktop secrets manager: passwords, API keys, SSH keys, encrypted documents (AES-256-GCM, Argon2id, TOTP RFC 6238, 24-word recovery phrase). Rust + GTK4/libadwaita + SQLite via SQLx.
- **Open Core**: this repo (`heelonvault-core`, public) holds the publishable library crate and the GTK binary; the private sibling repo `heelonvault-premium` (checked out as `../heelonvault-premium`) is compiled in via the `premium` Cargo feature (default-on; `licensing` implies it), resolved to the local checkout by the `[patch]` section of the root `Cargo.toml`. Premium activation happens **at runtime** via a signed Ed25519 license — premium code is always present in the shipped binary, gated by license checks.
- Workspace: `crates/heelonvault-core` (crate published on crates.io), `crates/heelonvault-app` (the `heelonvault` binary, composition root `src/main.rs`), `crates/sqlx-shim` (published under the name `sqlx`), `crates/heelonvault-url-check`.
- Layered flow: `UI (gtk4/libadwaita) → Services → Repositories (SQLx) → SQLite`.
- **Platform scope (current decision)**: Linux first (Wayland session), Windows 11 second target; **macOS development/distribution paused** (DMG workflow disabled, no new DMGs published, old DMGs not recommended for production).

## Commands (CI-aligned — always `--locked`)

```bash
./scripts/run-dev.sh                                   # dev run (DB data/heelonvault-rust-dev.db, logs ./logs)
cargo fmt --all -- --check
cargo check --workspace --all-features --locked
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
# clippy-secure, spelled as in CI so --locked reaches cargo:
cargo clippy --locked --workspace --all-targets --all-features -- \
  -D clippy::unwrap_used -D clippy::expect_used -D clippy::disallowed_methods \
  -D clippy::panic -D clippy::todo -D clippy::unimplemented
cargo test --workspace --all-features --locked
cargo test -p heelonvault-core --test release_2_1_1_compatibility --locked  # 2.1.1 baseline: schema digest, cipher vectors, backup/restore
# Linux native GTK certification (Wayland only, blocking in CI):
dbus-run-session -- bash scripts/run-wayland-tests.sh \
  cargo test --locked --workspace --all-features native_ -- --ignored --nocapture
cargo check -p heelonvault-app --features licensing --locked  # premium edition (needs ../heelonvault-premium)
cargo check -p heelonvault-app --no-default-features --locked # community assembly
cargo semver-checks -p heelonvault-core                   # public API vs crates.io (also in CI)
cargo publish --dry-run -p heelonvault-core --locked      # crate must build standalone (also in CI)
cargo deny --locked --all-features check --deny warnings  # supply-chain (also in CI)
cargo audit --deny warnings                               # + cargo audit --file ../heelonvault-premium/Cargo.lock --deny warnings
scripts/check-core-standalone.sh --semver                 # same gates without the premium repo (fork PRs)
./scripts/generate-sbom.sh                                # rerun when dependencies change → commit sbom.cyclonedx.json
```

Run standalone Premium checks **from `../heelonvault-premium`**, not by passing
`--manifest-path` from the core workspace: Cargo configuration and workspace patches depend
on the working directory. Run `cargo fmt --all -- --check`,
`cargo clippy --locked --all-targets --all-features -- -D warnings`,
`cargo check --locked --all-features`, `cargo test --locked --all-targets --all-features` and
`cargo deny --locked --all-features check advisories --deny warnings` there.

Toolchain and MSRV are defined in `rust-toolchain.toml` and the crate manifests; CI checks
MSRV separately. `.cargo/config.toml` sets `-Dwarnings` and `-Dunsafe_code` globally: any
warning or `unsafe` block fails the build. Do not remove `--locked` to make validation pass;
regenerate a lockfile only for an intentional dependency change.

Integration tests live under `crates/heelonvault-core/tests/` (plus repo-consistency tests in `crates/heelonvault-app/tests/`: `release_consistency.rs`, `i18n_usage.rs`, …). Most integration tests spin up a real SQLite DB via `crates/heelonvault-core/tests/common/mod.rs`.

## Non-negotiable rules

1. **Never `.unwrap()` / `.expect()`** on `Result`/`Option` (banned by `clippy.toml` `disallowed-methods`; panics can leak secrets on sensitive paths). Use typed `thiserror` errors (`AppError` in `crates/heelonvault-core/src/errors.rs`).
2. No `#[allow(...)]` to silence a lint without written rationale + owner + due date + tracked issue.
3. **Never put secret values in list/display models, logs or error messages.** Plaintext may
   reach an explicit reveal/edit/copy flow only after access checks; keep its lifetime and
   scope minimal. Never include real secrets in tests, screenshots or reports.
4. **No hardcoded user-facing text**: every label/tooltip/toast/dialog string goes through `heelonvault_core::tr!` / `tr_args` with a key in **both** `crates/heelonvault-core/locales/fr/main.ftl` and `locales/en/main.ftl` (enforced by `crates/heelonvault-core/tests/i18n_catalog.rs` and `crates/heelonvault-app/tests/i18n_usage.rs`). Any new static label must also be re-applied in the relevant `refresh_i18n` (live retranslation without restart; refreshers are collected in `I18nTargets::extra_refreshers`). Don't pass pre-rendered translated text around as data (the license badge is a `LicenseDisplay`, rendered at display time).
5. No new dependency without prior agreement in an issue; never add permanent `cargo audit` / `cargo deny` ignore entries.
6. **One version number** for the product, the `heelonvault-core` crate and `heelonvault-premium` (semver of the crate's public API): any API break = major release of the whole product. Prefer default trait methods, `#[non_exhaustive]`, deprecate-then-remove. Enforced by `crates/heelonvault-app/tests/release_consistency.rs` (which also requires the current version's section in **all four changelogs**: core `docs/CHANGELOG.md` + `docs/CHANGELOG.en.md`, premium `CHANGELOG.md` + `CHANGELOG.en.md`) and CI `cargo semver-checks`.
7. **User-visible change → FR + EN changelog entry** plus the relevant guides (`docs/QUICKSTART*`, `USER_GUIDE*`, `UPDATE_GUIDE*`); follow the PR template checklist (`.github/pull_request_template.md`).
8. Commits, PR titles, reviews and code comments in **English**. Commit/PR titles use
   `<type>(<scope>): <summary>` (the PR title becomes the squash commit on `main`).
   Issues may be FR or EN. User-facing docs are bilingual; see Documentation layout below.
9. Workflow: issue first → branch `<type>/<short-subject>` → PR to `main` (protected, **squash only**). **Merge the premium PR before the core PR**: core's CI checks out premium's `main` (`docs/internal/RELEASING.md`, step 6). External contributions are limited to `crates/heelonvault-core` and docs.
10. Dependency change → regenerate `sbom.cyclonedx.json` (`./scripts/generate-sbom.sh`) and commit it.
11. **Zero technical debt / zero security warnings**: no warning bypasses or permanent
    advisory ignores. An approved temporary exception needs rationale, owner, due date and
    a tracked blocking issue. Report output must remain deterministic and preserve its
    cryptographic integrity markers (hash/signature).

## Security patch in progress (do not "clean up")

- `[patch.crates-io]` pins `launchbadge/sqlx` at rev `10462fa4414c6c7f1ed1d39bb31678e3b726878c` (RUSTSEC-2026-0459, patched `libsqlite3-sys`) in **both repos**; `deny.toml` allows only that Git source, pinned by `rev`. Replace it with a published SQLx release accepting fixed SQLite **by 2026-11-16 or before crates.io publication, whichever comes first** (#75). Never simply delete the patch or restore vulnerable SQLite. If no fixed release exists by the deadline, keep publication blocked and report it; a passing publish dry-run does not clear this blocker. `crates/heelonvault-core/tests/release_2_1_1_compatibility.rs` asserts the bundled SQLite is ≥ 3.54.0.

## Security invariants (never break)

- `SecretRowView` / `SecretRowData` never hold a secret value (only `has_secret` / `is_weak` / `is_duplicate`; fingerprints stay in the loader thread). Copying a password decrypts on demand (`PasswordCopier` in `secret_flow.rs`) and re-opens the vault so access is re-checked. Don't reintroduce plaintext into display data or closures.
- Every clipboard copy of sensitive data goes through `sensitive_clipboard::copy_sensitive(text, SensitiveKind, delay)`. New on-demand decryption for a copy holds a `begin_decrypting()` guard. The exposure indicator must only claim what the app controls (never "no secret in memory").
- Key model: a random account key wraps every vault key, stored encrypted under the master password **and** the recovery phrase (`services/account_key.rs`). `change_master_password` rewrites only the password envelope; legacy accounts are migrated in one transaction (`rekey_service.rs`).
- `heelonvault-core` never initializes a `tracing` subscriber and never unwraps on sensitive paths — binary-only concerns.

## Desktop qualification (current decision)

- **Wayland is the only blocking Linux graphical certification**: the `native_` test suite runs with `G_DEBUG=fatal-warnings`, `GDK_BACKEND=wayland`, `GTK_IM_MODULE=simple`, through `scripts/run-wayland-tests.sh` (headless labwc, 1920×1600 output, `wl_seat` required). No Xvfb/X11 path remains in CI; **X11 is not certified** (#76). The Windows 11 package recipe and the Rust/security gates remain mandatory. No warning filtering, no weakened assertions. Audit and rationale: `docs/internal/GTK_BACKEND_QUALIFICATION.md`.
- macOS is paused: do not re-enable `.github/workflows/macos-dmg-rc.yml`; no DMGs in new releases.

## Architecture map

- **`crates/heelonvault-core`** — publishable library crate: `models/` (domain types), `repositories/` (SQLx data access per aggregate), `services/` (business logic; community default impls, premium overrides via traits), `config/`, `i18n.rs` + `locales/`, `errors.rs` (`AppError`/`AccessDeniedReason`).
- **`crates/heelonvault-app`** — the GTK4/libadwaita binary and **Open Core assembler**: `src/main.rs` is the composition root (GTK env vars, Tokio runtime, SQLite open via `HEELONVAULT_DB_PATH`, SQLx migrations, repository/service wiring swapping in `heelonvault_premium::services::*Impl` when `feature = "premium"`).
  - Main window: `src/ui/windows/main_window/` — `window/core.rs` orchestrates construction; behavior in `window/events.rs` + one module per feature (`editor.rs`, `refresh.rs`, `vault_list.rs`, `navigation.rs`, `pin_badge.rs`, `i18n_refresh.rs`, …). The main window is a root `GtkStack` (not modal dialogs): `entries_view`, `secret_editor_view`, `profile_view`, plus premium-only `users_view` / `teams_view`. Adding a page = register it on `center_panel.main_stack` + wire the sidebar button.
  - `profile_view/`: `sections/*.rs` each build one `adw::PreferencesGroup` and own their `refresh_i18n()`; `handlers/*.rs` wire callbacks with an explicit deps struct; `core.rs` assembles.
  - Widget-holder structs derive `Clone` — GTK widgets are refcounted handles, cloning into closures is the norm.
  - Dialogs: `src/ui/dialogs/<name>/` split into `core.rs` / `events.rs` / `views.rs` / `types.rs` / `feedback.rs` (+ flow files like `login_dialog/bootstrap_flow.rs`).
  - `src/ui/widgets/`: reusable widgets (`secret_card.rs`, `password_strength_bar.rs`).
  - `migrations/`: SQLx migrations applied at startup (no top-level `migrations/`); path via `resolve_migrations_path()`, overridable with `HEELONVAULT_MIGRATIONS_DIR`, `/opt/heelonvault/migrations` in packaged installs.
- **`crates/sqlx-shim`** — local crate published as `sqlx`, re-exporting `sqlx-core`/`sqlx-sqlite` with a pinned feature set; `heelonvault-app` depends on the shim, `heelonvault-core` on upstream `sqlx`.
- **`heelonvault-premium`** (private sibling) — premium impls of core traits + `license_service` (Ed25519) + `psc_config`.

### Session/runtime notes

- Secret editor: `EditorState` fingerprints the loaded form (no plaintext baseline), and
  `EditorNavigation` confirms before leaving or changing sidebar selection. Security clearing
  bypasses confirmation, wipes the draft and invalidates late editor callbacks; never route
  auto-lock through the unsaved-change prompt.
- Closing the main window and auto-lock both trigger a clean logout back to the login screen.
- A successful first-run bootstrap does **not** open a session; bootstrap and `close_request` must share the same completion flag, or closing the wizard quits the app.
- Secret list: two layouts (`SecretViewMode::Grid`/`List`) on the same `secret_flow` FlowBox; keep filter/sort/shortcuts layout-agnostic — switching modes re-renders from the last load (`SecretListCallbacks::rerender`), no DB access.
- CSV import: 3-phase UI (preview → progress → summary); rejected rows go to `HEELONVAULT_LOG_DIR/csv_import_rejects_*.txt`.
- Search: Unicode-normalized index over title/login/email/URL/notes/category/tags/type, field-scoped syntax (`email:`, `tag:`, `type:`), light fuzzy tolerance.

## Paths and environment

- DB: dev `data/heelonvault-rust-dev.db` (`HEELONVAULT_DB_PATH`); personal install `~/.local/share/heelonvault/heelonvault-rust.db`; enterprise `/var/lib/heelonvault/heelonvault-rust.db`. **Never touch `/var/lib/heelonvault-shared`** (legacy Python deployment) from this Rust runtime.
- Logs: `HEELONVAULT_LOG_DIR` (dev `./logs`); level via `RUST_LOG` then `HEELONVAULT_LOG_LEVEL`; packaged prod `~/.local/state/heelonvault/logs`.
- `HEELONVAULT_MIGRATIONS_DIR` overrides the migrations path.

## Documentation layout

- `docs/` (plus root `README*`, `SECURITY*`, `CONTRIBUTING*`, `CODE_OF_CONDUCT*`) is published on doc.heelonvault.heelonys.fr by the sibling `Heelonys_webdoc` repo. Write it for users / admins / security reviewers.
- `docs/internal/` is excluded from the site (development, release, packaging, CI, specs, QA).
- Bilingual docs need the `Language: EN | [FR](…)` / `Langue : FR | [EN](…)` line; no version numbers or "new in vX" in docs (the changelog holds history). Images go in `docs/images/`, referenced relatively.

## References

- Contribution workflow: [CONTRIBUTING.md](CONTRIBUTING.md).
- Architecture: [FR](docs/ARCHITECTURE.md) / [EN](docs/ARCHITECTURE.en.md).
- Development: [EN](docs/internal/DEVELOPMENT.md) / [FR](docs/internal/DEVELOPMENT.fr.md).
- Releases: [FR](docs/internal/RELEASING.md) / [EN](docs/internal/RELEASING.en.md).
- Validation: [manual QA](docs/internal/MANUAL_QA.md), [GTK qualification](docs/internal/GTK_BACKEND_QUALIFICATION.md).
- Automation: [CI](.github/workflows/ci.yml), [supply chain](.github/workflows/supply-chain.yml),
  [PR template](.github/pull_request_template.md).
