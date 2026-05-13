# Development Data Directory (Rust)

Language: EN | [FR](README.fr.md)

This folder is used only for local Rust development data.

## Paths

- Dev database: `data/heelonvault-rust-dev.db`
- Packaged user database: `~/.local/share/heelonvault/heelonvault-rust.db`

## Legacy Data Protection

Do not modify or delete `/var/lib/heelonvault-shared`.
This path belongs to legacy Python data and must remain untouched.

## Reset Local Dev Data

```bash
rm -f data/heelonvault-rust-dev.db
```

The dev database will be recreated on next `./scripts/run-dev.sh` launch.
