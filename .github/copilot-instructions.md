# Copilot Instructions - Zero Technical Debt Policy

## Core rule

This repository follows a strict zero-technical-debt and zero-security-warning policy.
Any change that introduces unresolved debt must be rejected.

## Mandatory quality gates before merge

1. `cargo fmt --all -- --check`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `cargo check --locked`
4. `cargo test --locked --all-targets --no-run`
5. `cargo audit` with **0 warnings**
6. `cargo deny check advisories` with **0 violations**
7. CI checks must be green on Linux, Fedora, macOS, and Windows

## Dependency policy

- Do not add dependencies with unmaintained/yanked advisories.
- Do not silence advisories via permanent ignore lists.
- If a dependency is unavoidable short-term, open a blocking issue and schedule immediate removal.
- Prefer maintained, minimal-dependency crates.

## Review policy

- No "temporary" warning bypasses.
- No `#[allow(...)]` to hide lint/security debt unless explicitly approved and time-boxed.
- Any exception must include:
  - explicit rationale
  - owner
  - due date
  - tracked issue reference

## Architecture policy for reports/PDF

- Keep deterministic report output.
- Preserve cryptographic integrity markers (hash/signature) in exported reports.
- Prefer simple, auditable implementations over feature-heavy dependency chains.
