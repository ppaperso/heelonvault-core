# HeelonVault 2.0.1

Language: EN | [FR](README.md)

[![SBOM](https://img.shields.io/badge/SBOM-CycloneDX%201.4-blue)](sbom.cyclonedx.json) [![Supply chain](https://img.shields.io/badge/supply--chain-cargo--deny-green)](.github/workflows/supply-chain.yml)

HeelonVault is a **local-first** desktop secrets manager: passwords, API keys, SSH keys and
sensitive documents are encrypted and stored on your computer, with no server and no online
account. Written in Rust, with a GTK4 / libadwaita interface, for Windows, macOS and Linux.

> Distributed under the Apache 2.0 License. See [LICENSE](LICENSE) for software terms and [LEGAL.md](docs/LEGAL.md) for trademark and Authenticity Seal terms.

**Full documentation: [doc.heelonvault.heelonys.fr](https://doc.heelonvault.heelonys.fr/en/)**

---

## Installation

Download the package for your system from the
[Releases](https://github.com/ppaperso/heelonvault-core/releases/latest) page:

| System | Package |
| ------ | ------- |
| Windows 10 / 11 (x64) | `heelonvault-windows-x86_64-vX.Y.Z.msi` |
| macOS 13+ (Apple Silicon) | `heelonvault-macos-aarch64-vX.Y.Z.dmg` |
| Linux (x86_64) | `heelonvault-linux-x86_64-vX.Y.Z.AppImage` |

Verification, step-by-step install and first launch:
[Installation and quickstart](docs/QUICKSTART.md).

---

## Core Features

| Area | Details |
| ---- | ------- |
| **Encryption** | Application-level AES-256-GCM — secrets never leave the machine in plaintext |
| **Authentication** | Argon2id key derivation (GPU-resistant) + TOTP two-factor authentication (RFC 6238) |
| **First launch** | Guided wizard creating the administrator account, followed by an explicit sign-in |
| **Recovery key** | 24-word phrase generated at setup; sets a new master password without losing any vault; re-exportable from the profile |
| **Vaults** | Several vaults per account; search the active vault or all of them (MultiVault) |
| **Display** | Cards or compact list, usage-based ordering, badges (strength, duplicate, incomplete, health, shared) |
| **Clipboard** | Password decrypted only at copy time, automatic clearing (20 s, 60 s for the recovery phrase), exposure indicator in the header bar |
| **Search** | Title, login, email, URL, notes, category, tags, type; `field:value` syntax, Unicode normalization, `#sante` shortcut |
| **Keyboard** | On the active card: `Ctrl+C` (password), `Ctrl+L` (login), `Ctrl+U` (open URL) |
| **Session** | Auto-lock (1 to 30 minutes or never), quick-unlock PIN, brute-force protection |
| **Import / Export** | Guided 3-step CSV import, error-tolerant; encrypted `.hvb` export |
| **Trash** | Soft delete with restore and permanent purge |
| **Strength meter** | Real-time `zxcvbn` evaluation of every password |
| **Health marker** | "Health data access" field to single out secrets tied to medical data |
| **Logging** | Daily-rotated JSON logs, free of any secret value |
| **Pro license** | User administration, teams and shared vaults, signed audit reports (Ed25519); without a license, full Community edition |

---

## Audit and Compliance

HeelonVault follows a security-first approach for GDPR-oriented data protection.

### License and transparency

- **Dependency inventory**: complete third-party component list and licenses are documented in [THIRD_PARTY_LICENSES.md](docs/THIRD_PARTY_LICENSES.md).
- **Signed CycloneDX SBOM**: with every version, the SBOM of the shipped binary (core + app + premium) is published on the [GitHub release](https://github.com/ppaperso/heelonvault-core/releases/latest) (`heelonvault-sbom-<version>.cyclonedx.json` + `.sha256`), with a Sigstore build-provenance attestation verifiable via `gh attestation verify heelonvault-sbom-<version>.cyclonedx.json --repo ppaperso/heelonvault-core`.
- **Auditable proprietary component**: `heelonvault-premium` (licensing, administration, teams, audit report) is proprietary and its source is not public. A customer's security teams (IT department, CISO, appointed auditor) can be given read access for audit, under a non-disclosure agreement (NDA): see [SECURITY.md](SECURITY.md#13-supply-chain-security-and-sbom).
- **No statically linked copyleft dependency** — the only LGPL libraries (GTK4, libadwaita) are dynamically linked.

### Cryptographic primitives

- **AES-256-GCM** for authenticated encryption (`aes-gcm` crate, RustCrypto).
- **Argon2id** for deriving keys from the master password.
- **HMAC-SHA1 / SHA256** for TOTP generation (`totp-rs`).
- **CSPRNG** via `getrandom` (kernel RNG) for salts, nonces and keys.

### Code policy

`clippy.toml` bans `unwrap()` / `expect()` on `Result` and `Option`, and the workspace rejects any
`unsafe` block and any compiler warning: an unexpected panic cannot expose sensitive data in an
error message.

### Vulnerability reporting

See [SECURITY.md](SECURITY.md).

---

## Development

```text
heelonvault-core/
├── crates/
│   ├── heelonvault-core/   # Public library (crates.io)
│   ├── heelonvault-app/    # GTK4 / libadwaita binary (migrations, assets, MSI installer)
│   └── sqlx-shim/          # Local SQLx shim
├── linux/  macos/          # AppImage launcher and macOS bundle files
├── scripts/                # Dev launcher, Linux system install, SBOM
└── docs/                   # Documentation (docs/internal/: maintainers)
```

```bash
./scripts/run-dev.sh        # dev database: data/heelonvault-rust-dev.db
cargo test --workspace
```

Prerequisites, quality gates and builds: [development guide](docs/internal/DEVELOPMENT.md).
Contribution rules: [CONTRIBUTING.md](CONTRIBUTING.md).

> **Premium**: `heelonvault-premium` is a proprietary component kept in a separate private
> repository, checked out next to this one to build the binary.

---

## Documentation

| Document | English | French |
| -------- | ------- | ------ |
| Installation and quickstart | [QUICKSTART.md](docs/QUICKSTART.md) | [QUICKSTART.fr.md](docs/QUICKSTART.fr.md) |
| User guide | [USER_GUIDE.en.md](docs/USER_GUIDE.en.md) | [USER_GUIDE.md](docs/USER_GUIDE.md) |
| Updating and deployment | [UPDATE_GUIDE.en.md](docs/UPDATE_GUIDE.en.md) | [UPDATE_GUIDE.md](docs/UPDATE_GUIDE.md) |
| Security | [SECURITY.md](SECURITY.md) | [SECURITY.fr.md](SECURITY.fr.md) |
| Architecture | [ARCHITECTURE.en.md](docs/ARCHITECTURE.en.md) | [ARCHITECTURE.md](docs/ARCHITECTURE.md) |
| Changelog | [CHANGELOG.en.md](docs/CHANGELOG.en.md) | [CHANGELOG.md](docs/CHANGELOG.md) |
| Third-party licenses | [THIRD_PARTY_LICENSES.md](docs/THIRD_PARTY_LICENSES.md) | [THIRD_PARTY_LICENSES.fr.md](docs/THIRD_PARTY_LICENSES.fr.md) |
| Contributing | [CONTRIBUTING.md](CONTRIBUTING.md) | [CONTRIBUTING.fr.md](CONTRIBUTING.fr.md) |
| Code of Conduct | [CODE_OF_CONDUCT.en.md](CODE_OF_CONDUCT.en.md) | [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) |
| Internal documentation (maintainers) | [docs/internal/](docs/internal/README.md) | |

---

> **Current version**: 2.0.1 — detailed release notes in [CHANGELOG.en.md](docs/CHANGELOG.en.md).
