# Scripts

Language: EN | [FR](README.fr.md)

This folder contains operational shell scripts used by the Rust repository.
Run scripts from the repository root.

## Available Scripts

- `scripts/backup-prod-before-tests.sh`: creates a production backup archive before manual tests.
- `scripts/fix-permissions.sh`: repairs permissions and ACL for legacy shared-data deployments.

## Rust Development

For development and tests, use root-level scripts and Rust commands:

```bash
./scripts/run-dev.sh
cargo check
cargo test
```

Data path notes:

- Packaged user data: `~/.local/share/heelonvault`
- Rust dev data: `data/`
- Legacy Python data: `/var/lib/heelonvault-shared` (do not modify)
