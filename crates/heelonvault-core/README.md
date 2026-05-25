# heelonvault-core

Core open-source library for [HeelonVault](https://github.com/ppaperso/heelonvault-core) — a sovereign,
privacy-first password manager designed for healthcare environments.

## Overview

`heelonvault-core` provides the cryptographic primitives, data models, service traits, and SQLite
repositories that power HeelonVault. It is the community foundation of the HeelonVault ecosystem,
published under the Apache-2.0 license.

## Features

- **AES-256-GCM encryption** for secrets at rest
- **Argon2id key derivation** for master password protection
- **SQLite persistence** via SQLx with compile-time verified migrations
- **Multi-vault RBAC** with owner / shared-read / shared-write / admin roles
- **TOTP two-factor authentication** service (RFC 6238)
- **Audit log** with immutable, append-only journaling
- **Encrypted backup** with integrity verification
- **CSV import** with legacy format support
- **i18n** — English and French out of the box via Fluent

## Usage

```toml
[dependencies]
heelonvault-core = "1.1"
```

## Security Policy

This crate follows a zero-technical-debt policy:

- `cargo clippy -- -D warnings` must be clean on every release
- `cargo audit` with 0 advisories on every release
- No `#[allow(...)]` lint suppressions without explicit rationale and tracking issue

Vulnerabilities may be reported via the
[security policy](https://github.com/ppaperso/heelonvault-core/blob/main/SECURITY.md).

## Premium Extensions

Advanced features (multi-user administration, team sharing, signed audit reports, license
verification) are provided by the separate `heelonvault-premium` crate, which depends on this
library.

## License

Apache-2.0 — see [LICENSE](https://github.com/ppaperso/heelonvault-core/blob/main/LICENSE).
