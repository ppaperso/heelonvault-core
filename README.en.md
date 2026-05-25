# HeelonVault 1.1.0

Language: EN | [FR](README.md)

HeelonVault is a local-first desktop secrets manager built in Rust with GTK4/libadwaita and SQLite.

> Distributed under the Apache 2.0 License. See [LICENSE](LICENSE) for software terms and [LEGAL.md](docs/LEGAL.md) for trademark and Authenticity Seal terms.

---

## Core Features

| Area | Details |
| ---- | ------- |
| **Encryption** | AES-256-GCM at application level; secrets never leave the machine in plaintext |
| **Authentication** | Argon2id password hashing + TOTP 2FA (RFC 6238) |
| **Multi-user** | Isolated accounts and vaults per user |
| **Bootstrap** | Guided 3-step wizard for first-admin account creation on initial startup |
| **Recovery Key** | 24-word BIP39-style mnemonic phrase generated at bootstrap; re-exportable from profile; clipboard copy with automatic 60-second auto-clear |
| **Persistence** | Local SQLite with versioned `sqlx` migrations (14 migrations, zero downtime) |
| **Import / Export** | CSV import, `.hvb` export with RBAC access control |
| **Audit Log** | Traceability for sensitive actions (secret create/update/delete, vault sharing) |
| **Trash** | Soft-delete with restore and permanent purge |
| **Auto-lock** | Configurable policy: 1 / 5 / 15 / 30 minutes or never |
| **Dashboard** | Dedicated security dashboard with global vault score |
| **Strength Meter** | Real-time `zxcvbn` evaluation for each password |
| **Advanced Search** | Multi-field search with Unicode normalization |
| **License** | Ed25519 signature verification for signed licenses; badge visible before and after login; automatic Community fallback |
| **Structured Logs** | Rotating JSON logs in `~/.local/state/heelonvault/logs` |

---

## Audit and Compliance

HeelonVault follows a security-first approach for GDPR-oriented data protection.

### License and transparency

- Distributed under the Apache 2.0 License. See [LICENSE](LICENSE) for software terms and [LEGAL.md](docs/LEGAL.md) for trademark and Authenticity Seal terms.
- **Dependency inventory**: complete third-party component list and licenses are documented in [THIRD_PARTY_LICENSES.md](docs/THIRD_PARTY_LICENSES.md).
- **Signed CycloneDX SBOM**: the SBOM is generated from the `heelonvault-premium` repository; `sbom.cyclonedx.json` and `sbom.cyclonedx.json.sha256` are published manually at release time.
- **LGPL runtime linking**: GTK4/libadwaita are dynamically linked by the operating system.

### Cryptographic primitives

- **AES-256-GCM** for authenticated encryption (`aes-gcm` crate).
- **Argon2id** for password hashing.
- **HMAC-SHA1 / SHA256** for TOTP generation (`totp-rs`).
- **CSPRNG** via `getrandom`.

### Error-handling policy

`clippy.toml` forbids panic-prone `unwrap()` / `expect()` calls on sensitive paths to reduce crash-leak risks.

### Vulnerability reporting

See [SECURITY.md](SECURITY.md).

---

## Repository Structure

```text
heelonvault-core/
├── crates/
│   ├── heelonvault-core/      # Public library (crates.io)
│   ├── heelonvault-app/       # GTK4 / libadwaita binary
│   └── sqlx-shim/             # Local SQLx shim
├── migrations/            # SQL migrations
├── assets/                # Bundled GTK assets (CSS, icons, images)
├── resources/             # Non-migrated resources (fonts)
├── tests/                 # Rust integration tests
├── docs/                  # Technical documentation
├── data/                  # Local dev database
├── logs/                  # Runtime logs
├── LICENSE                # Apache 2.0 license
├── docs/THIRD_PARTY_LICENSES.md  # Third-party dependency licenses
├── scripts/install.sh     # Unified installer (OS detection)
├── scripts/install-ubuntu.sh      # Ubuntu / Debian installer
├── scripts/install-rhel.sh        # Fedora / RHEL / Rocky Linux / AlmaLinux installer
├── scripts/remove.sh      # Unified uninstaller (OS detection)
├── scripts/remove-ubuntu.sh       # Ubuntu / Debian uninstaller
└── scripts/remove-rhel.sh         # Fedora / RHEL / Rocky Linux / AlmaLinux uninstaller
```

> **Premium**: `heelonvault-premium` lives in a separate private repository.
> The community version of this repo never accesses it.

---

## Quick Start

### Development

```bash
./scripts/run-dev.sh
```

Dev database: `data/heelonvault-rust-dev.db`

### Build and lint

```bash
cargo check --workspace
cargo clippy --workspace -- -D warnings
```

### Packaged Linux installation

The installer asks for a deployment profile:

- **Personal**: SQLite DB in `~/.local/share/heelonvault/heelonvault-rust.db`, logs in `~/.local/state/heelonvault/logs`.
- **Enterprise**: SQLite DB in `/var/lib/heelonvault/heelonvault-rust.db`, logs in `/var/log/heelonvault`.

```bash
tar -xzf heelonvault-linux-x86_64.tar.gz
cd heelonvault-linux-x86_64
sudo ./scripts/install.sh
```

Preview mode without changing the system (dry-run):

```bash
sudo env HEELONVAULT_DRY_RUN=1 ./scripts/install.sh
```

If needed, you can still run `scripts/install-ubuntu.sh` or `scripts/install-rhel.sh` explicitly.

Release security: if `heelonvault.sha256` is present in the archive, installer verifies binary integrity before installation.

Enterprise mode note: installer only configures shared system paths.
Network publication (RDS/VDI/RemoteApp, reverse proxy, bastion, etc.) must be handled manually.
For optimal performance, Enterprise mode database should be hosted on low-latency storage, ideally local to the execution server.

Uninstall:

```bash
sudo ./scripts/remove.sh
```

If needed, you can still run `scripts/remove-ubuntu.sh` or `scripts/remove-rhel.sh` explicitly.

See [QUICKSTART.md](docs/QUICKSTART.md) and [QUICKSTART.fr.md](docs/QUICKSTART.fr.md).

### Tests

```bash
cargo test
```

---

## Bilingual Documentation Index

Central index: [docs/README.md](docs/README.md)

| Document | English | French |
| -------- | ------- | ------ |
| Changelog | [CHANGELOG.en.md](docs/CHANGELOG.en.md) | [CHANGELOG.md](docs/CHANGELOG.md) |
| Overview | [README.en.md](README.en.md) | [README.md](README.md) |
| Quickstart | [QUICKSTART.md](docs/QUICKSTART.md) | [QUICKSTART.fr.md](docs/QUICKSTART.fr.md) |
| Contributing | [CONTRIBUTING.md](CONTRIBUTING.md) | [CONTRIBUTING.fr.md](CONTRIBUTING.fr.md) |
| Security | [SECURITY.md](SECURITY.md) | [SECURITY.fr.md](SECURITY.fr.md) |
| Code of Conduct | [CODE_OF_CONDUCT.en.md](CODE_OF_CONDUCT.en.md) | [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) |
| Architecture | [docs/ARCHITECTURE.en.md](docs/ARCHITECTURE.en.md) | [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) |
| User Guide | [docs/USER_GUIDE.en.md](docs/USER_GUIDE.en.md) | [docs/USER_GUIDE.md](docs/USER_GUIDE.md) |
| Update Guide | [docs/UPDATE_GUIDE.en.md](docs/UPDATE_GUIDE.en.md) | [docs/UPDATE_GUIDE.md](docs/UPDATE_GUIDE.md) |
| Data folder | [data/README.md](data/README.md) | [data/README.fr.md](data/README.fr.md) |
| Scripts | [scripts/README.md](scripts/README.md) | [scripts/README.fr.md](scripts/README.fr.md) |
| Tests | [tests/README.en.md](tests/README.en.md) | [tests/README.md](tests/README.md) |
| Third-party licenses | [THIRD_PARTY_LICENSES.md](docs/THIRD_PARTY_LICENSES.md) | [THIRD_PARTY_LICENSES.fr.md](docs/THIRD_PARTY_LICENSES.fr.md) |

---

> Detailed release notes are in [CHANGELOG.en.md](docs/CHANGELOG.en.md).
