---
name: heelonvault-dev
description: "HeelonVault working guide for Claude Code — local-first desktop secrets manager in Rust/GTK4/libadwaita + SQLx/SQLite (github.com/ppaperso/heelonvault-core + private sibling heelonvault-premium). Canonical guide: AGENTS.md at the repo root (also auto-loaded via the root CLAUDE.md pointer)."
user-invocable: true
---

# heelonvault-dev (Claude Code skill)

**Canonical, always up to date: [`AGENTS.md`](../../../AGENTS.md) at the repository root.**
Claude Code also auto-loads the root `CLAUDE.md`, which points here and to `AGENTS.md`.

Quick rules (full detail in `AGENTS.md`):

1. Never `.unwrap()` / `.expect()` on `Result`/`Option` (banned by `clippy.toml`); typed `thiserror` errors (`AppError`).
2. No `#[allow(...)]` without written rationale + owner + due date + tracked issue.
3. Never let a secret value reach the UI, a log or an error message.
4. No hardcoded user-facing text: `heelonvault_core::tr!` / `tr_args` + key in **both** `crates/heelonvault-core/locales/{fr,en}/main.ftl`; re-apply new labels in the relevant `refresh_i18n`.
5. No new dependency without an issue; no permanent `cargo audit`/`deny` ignores.
6. One version for product + `heelonvault-core` crate + premium; API break = major release; all 4 changelogs must carry the current version (`release_consistency.rs`).
7. User-visible change → FR + EN changelog entry + relevant guides (`docs/QUICKSTART*`, `USER_GUIDE*`, `UPDATE_GUIDE*`).
8. Commits / PR titles / reviews / code comments in **English**, `<type>(<scope>): <summary>`; squash-only merges to protected `main`; merge the **premium PR before the core PR**.
9. Dependency change → regenerate and commit `sbom.cyclonedx.json` (`./scripts/generate-sbom.sh`).
10. Linux graphical certification is **Wayland-only** via `scripts/run-wayland-tests.sh` with `G_DEBUG=fatal-warnings` (X11 not certified, #76); macOS is paused; the SQLx `[patch.crates-io]` pin (RUSTSEC-2026-0459, #75) must not be removed before 2026-11-16 / crates.io publication.

CI-aligned gates (always `--locked`): `cargo fmt --all -- --check`, `cargo check --workspace --all-features`, `cargo clippy --locked --workspace --all-targets --all-features` (+ the clippy-secure lint set), `cargo test --workspace --all-features`, `cargo deny --locked --all-features check --deny warnings`, `cargo audit --deny warnings` (+ premium lockfile), `cargo semver-checks -p heelonvault-core`, `cargo publish --dry-run -p heelonvault-core`.
