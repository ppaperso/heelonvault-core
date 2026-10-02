# HeelonVault 2.0.1

Langue : FR | [EN](README.en.md)

[![SBOM](https://img.shields.io/badge/SBOM-CycloneDX%201.4-blue)](sbom.cyclonedx.json) [![Supply chain](https://img.shields.io/badge/supply--chain-cargo--deny-green)](.github/workflows/supply-chain.yml)

HeelonVault est un gestionnaire de secrets desktop **local-first** : mots de passe, clés API,
clés SSH et documents sensibles sont chiffrés et stockés sur votre poste, sans serveur ni compte
en ligne. Écrit en Rust, avec une interface GTK4 / libadwaita, pour Windows, macOS et Linux.

> Distribué sous licence Apache 2.0. Voir [LICENSE](LICENSE) pour le logiciel et [LEGAL.md](docs/LEGAL.md) pour les conditions relatives à la marque et au Sceau d'Authenticité.

**Documentation complète : [doc.heelonvault.heelonys.fr](https://doc.heelonvault.heelonys.fr)**

---

## Installation

Téléchargez le paquet de votre système sur la page
[Releases](https://github.com/ppaperso/heelonvault-core/releases/latest) :

| Système | Paquet |
| ------- | ------ |
| Windows 10 / 11 (x64) | `heelonvault-windows-x86_64-vX.Y.Z.msi` |
| macOS 13+ (Apple Silicon) | `heelonvault-macos-aarch64-vX.Y.Z.dmg` |
| Linux (x86_64) | `heelonvault-linux-x86_64-vX.Y.Z.AppImage` |

Vérification, installation pas à pas et premier lancement :
[Installation et démarrage rapide](docs/QUICKSTART.fr.md).

---

## Fonctionnalités principales

| Domaine | Détail |
| ------- | ------ |
| **Chiffrement** | AES-256-GCM côté application — les secrets ne quittent jamais la machine en clair |
| **Authentification** | Dérivation Argon2id (résistante aux GPU) + double authentification TOTP (RFC 6238) |
| **Premier lancement** | Assistant guidé de création du compte administrateur, puis connexion explicite |
| **Clé de récupération** | Phrase de 24 mots générée à l'initialisation ; permet de redéfinir le mot de passe maître sans perdre les coffres ; réexportable depuis le profil |
| **Coffres** | Plusieurs coffres par compte ; recherche dans le coffre actif ou dans tous (MultiCoffre) |
| **Affichage** | Cartes ou liste compacte, tri par fréquence d'usage, badges (robustesse, doublon, incomplet, santé, partagé) |
| **Presse-papiers** | Mot de passe déchiffré seulement au moment de la copie, effacement automatique (20 s, 60 s pour la phrase de récupération), indicateur d'exposition dans la barre d'en-tête |
| **Recherche** | Titre, login, email, URL, notes, catégorie, tags, type ; syntaxe `champ:valeur`, normalisation Unicode, raccourci `#sante` |
| **Clavier** | Sur la carte active : `Ctrl+C` (mot de passe), `Ctrl+L` (login), `Ctrl+U` (ouvrir l'URL) |
| **Session** | Auto-verrouillage (1 à 30 minutes ou jamais), déverrouillage rapide par code PIN, protection contre la force brute |
| **Import / Export** | Import CSV guidé en 3 étapes, tolérant aux erreurs ; export chiffré `.hvb` |
| **Corbeille** | Suppression logique avec restauration et purge définitive |
| **Robustesse** | Évaluation `zxcvbn` en temps réel de chaque mot de passe |
| **Marqueur santé** | Champ « Accès données de santé » pour isoler les secrets liés à des données médicales |
| **Journalisation** | Journaux JSON à rotation quotidienne, sans aucune valeur secrète |
| **Licence Pro** | Administration des utilisateurs, équipes et coffres partagés, rapports d'audit signés (Ed25519) ; sans licence, édition Community complète |

---

## 🛡️ Audit & Conformité

Ce projet est conçu avec une architecture **security-first** pour garantir la conformité RGPD
et la protection des données utilisateurs.

### Licence et transparence

- **Inventaire des dépendances** : la totalité des bibliothèques tierces (Rust + système)
  et leurs licences exactes sont documentées dans [THIRD_PARTY_LICENSES.md](docs/THIRD_PARTY_LICENSES.md).
- **SBOM CycloneDX signé** : à chaque version, le SBOM du binaire livré (core + app + premium) est publié sur la [release GitHub](https://github.com/ppaperso/heelonvault-core/releases/latest) (`heelonvault-sbom-<version>.cyclonedx.json` + `.sha256`), avec une attestation de provenance Sigstore vérifiable via `gh attestation verify heelonvault-sbom-<version>.cyclonedx.json --repo ppaperso/heelonvault-core`.
- **Composant propriétaire auditable** : `heelonvault-premium` (licence, administration, équipes, rapport d'audit) est propriétaire et son code n'est pas public. Les équipes sécurité d'un client (DSI, RSSI, auditeur mandaté) peuvent y accéder en lecture pour audit, sous accord de confidentialité (NDA) : voir [SECURITY.fr.md](SECURITY.fr.md#14-securite-de-la-chaine-dapprovisionnement-et-sbom).
- **Aucune dépendance copyleft** compilée statiquement dans le binaire — les seules bibliothèques
  LGPL (GTK4, libadwaita) sont liées dynamiquement.

### Primitives cryptographiques

- **AES-256-GCM** (authentifié) — chiffrement des secrets via crate `aes-gcm` (RustCrypto).
- **Argon2id** — dérivation des clés à partir du mot de passe maître (résistant aux attaques par GPU/ASIC).
- **HMAC-SHA1 / SHA256** — génération des codes TOTP (RFC 6238) via crate `totp-rs`.
- **CSPRNG** — génération des sels, nonces et clés via `getrandom` (RNG du noyau).

### Politique de code

Un fichier [`clippy.toml`](clippy.toml) interdit globalement les appels `unwrap()` / `expect()`
sur les valeurs `Result` et `Option`, et le workspace refuse tout bloc `unsafe` et tout
avertissement de compilation. Une panique imprévue ne peut donc pas exposer de donnée sensible
dans un message d'erreur.

### Signalement de vulnérabilités

Consulter [SECURITY.fr.md](SECURITY.fr.md) pour la politique de divulgation responsable.

---

## Développement

```text
heelonvault-core/
├── crates/
│   ├── heelonvault-core/   # Bibliothèque publique (crates.io)
│   ├── heelonvault-app/    # Binaire GTK4 / libadwaita (migrations, assets, installeur MSI)
│   └── sqlx-shim/          # Shim local SQLx
├── linux/  macos/          # Lanceurs AppImage et bundle macOS
├── scripts/                # Lancement dev, installation système Linux, SBOM
└── docs/                   # Documentation (docs/internal/ : mainteneurs)
```

```bash
./scripts/run-dev.sh        # base de dev : data/heelonvault-rust-dev.db
cargo test --workspace
```

Prérequis, contrôles qualité et build : [guide de développement](docs/internal/DEVELOPMENT.fr.md).
Règles de contribution : [CONTRIBUTING.fr.md](CONTRIBUTING.fr.md).

> **Premium** : `heelonvault-premium` est un composant propriétaire maintenu dans un dépôt privé
> séparé, cloné à côté de ce dépôt pour construire le binaire.

---

## Documentation

| Document | Français | English |
| -------- | -------- | ------- |
| Installation et démarrage rapide | [QUICKSTART.fr.md](docs/QUICKSTART.fr.md) | [QUICKSTART.md](docs/QUICKSTART.md) |
| Guide utilisateur | [USER_GUIDE.md](docs/USER_GUIDE.md) | [USER_GUIDE.en.md](docs/USER_GUIDE.en.md) |
| Mise à jour et déploiement | [UPDATE_GUIDE.md](docs/UPDATE_GUIDE.md) | [UPDATE_GUIDE.en.md](docs/UPDATE_GUIDE.en.md) |
| Sécurité | [SECURITY.fr.md](SECURITY.fr.md) | [SECURITY.md](SECURITY.md) |
| Architecture | [ARCHITECTURE.md](docs/ARCHITECTURE.md) | [ARCHITECTURE.en.md](docs/ARCHITECTURE.en.md) |
| Journal des modifications | [CHANGELOG.md](docs/CHANGELOG.md) | [CHANGELOG.en.md](docs/CHANGELOG.en.md) |
| Licences tierces | [THIRD_PARTY_LICENSES.fr.md](docs/THIRD_PARTY_LICENSES.fr.md) | [THIRD_PARTY_LICENSES.md](docs/THIRD_PARTY_LICENSES.md) |
| Contribuer | [CONTRIBUTING.fr.md](CONTRIBUTING.fr.md) | [CONTRIBUTING.md](CONTRIBUTING.md) |
| Code de conduite | [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) | [CODE_OF_CONDUCT.en.md](CODE_OF_CONDUCT.en.md) |
| Documentation interne (mainteneurs) | [docs/internal/](docs/internal/README.md) | |

---

> **Version actuelle** : 2.0.1 — notes de version détaillées dans [CHANGELOG.md](docs/CHANGELOG.md).
