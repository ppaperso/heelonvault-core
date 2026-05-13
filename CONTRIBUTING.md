# Contribution Guide (Rust)

Language: EN | [FR](CONTRIBUTING.fr.md)

Thanks for contributing to HeelonVault.

## Scope

- Main application code lives at the repository root.
- Legacy Python code was removed from this repository.
- Keep contributions Rust-first and security-focused.

## Development Setup

Prerequisites:

- Linux
- Rust toolchain (`cargo`, `rustc`)
- GTK4/libadwaita runtime packages for your distro

Setup:

```bash
git clone <repo-url>
cd HeelonVault
cargo check
```

Run application in development mode:

```bash
./scripts/run-dev.sh
```

Database paths:

- Dev: `data/heelonvault-rust-dev.db`
- Packaged user DB: `~/.local/share/heelonvault/heelonvault-rust.db`
- Legacy path `/var/lib/heelonvault-shared` must remain untouched.

## Code Standards

- Follow existing style and naming conventions.
- Prefer small, focused commits.
- Add tests for repository/service behavior changes.
- Do not add secrets or private data in commits.

## Test Commands

From repository root:

```bash
cargo check
cargo test
cargo test secret_repository:: -- --nocapture
cargo test secret_service:: -- --nocapture
```

## Pull Request Checklist

- `cargo check` passes.
- Relevant tests pass.
- Security-sensitive changes are justified in PR description.
- Documentation is updated when behavior changes.

## Security Reports

Do not open a public issue for security vulnerabilities.
Contact: `security@heelonys.fr`
