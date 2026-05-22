# HeelonVault 1.1.0

Langue: FR | [EN](README.en.md)

HeelonVault est un gestionnaire de secrets desktop **local-first**, écrit en Rust et construit
avec GTK4 / libadwaita et SQLite.

> Distribué sous licence Apache 2.0. Voir [LICENSE](LICENSE) pour le logiciel et [LEGAL.md](docs/LEGAL.md) pour les conditions relatives à la marque et au Sceau d'Authenticité.

---

## Fonctionnalités principales

| Domaine | Détail |
| ------- | ------ |
| **Chiffrement** | AES-256-GCM côté application — les secrets ne quittent jamais la machine en clair |
| **Authentification** | Hachage Argon2id (résistant aux GPU) + TOTP 2FA (RFC 6238) |
| **Multi-utilisateur** | Comptes séparés avec coffres isolés par utilisateur |
| **Bootstrap** | Assistant d'initialisation guidé en 3 étapes pour la création du premier compte administrateur |
| **Clé de récupération** | Phrase mnémotechnique 24 mots (style BIP39) générée à l'initialisation ; exportable depuis le profil ; copie avec effacement presse-papier automatique (60 s) |
| **Persistance** | SQLite local, versionné par migrations `sqlx` (14 migrations, sans interruption de service) |
| **Import / Export** | Import CSV, export `.hvb` avec contrôle d'accès RBAC |
| **Journal d'audit** | Traçabilité des actions sensibles (création/modification/suppression de secrets, partages) |
| **Corbeille** | Suppression logique avec restauration et purge définitive |
| **Auto-verrouillage** | Politique configurable : 1 / 5 / 15 / 30 minutes ou jamais |
| **Tableau de bord** | Fenêtre de sécurité dédiée avec score global du coffre |
| **Indicateur de force** | Évaluation `zxcvbn` en temps réel sur chaque mot de passe |
| **Recherche avancée** | Multi-champs (titre, login, email, URL, notes, catégorie, tags, type) avec normalisation Unicode |
| **Licence** | Vérification Ed25519 de la licence signée ; badge visible avant et après login ; fallback Community automatique |
| **Logs structurés** | Tracing JSON rotatif dans `~/.local/state/heelonvault/logs` |

---

## 🛡️ Audit & Conformité

Ce projet est conçu avec une architecture **security-first** pour garantir la conformité RGPD
et la protection des données utilisateurs.

### Licence et transparence

- Distribué sous licence Apache 2.0. Voir [LICENSE](LICENSE) pour le logiciel et [LEGAL.md](docs/LEGAL.md) pour les conditions relatives à la marque et au Sceau d'Authenticité.
- **Inventaire des dépendances** : la totalité des bibliothèques tierces (Rust + système)
  et leurs licences exactes sont documentées dans [THIRD_PARTY_LICENSES.md](docs/THIRD_PARTY_LICENSES.md).
- **SBOM CycloneDX signé** : la release publie `sbom.cyclonedx.json` et `sbom.cyclonedx.json.sha256`, avec attestation de provenance GitHub Actions.
- **Aucune dépendance copyleft** compilée statiquement dans le binaire — les seules bibliothèques
  LGPL (GTK4, libadwaita) sont liées dynamiquement par le système d'exploitation.

### Primitives cryptographiques

- **AES-256-GCM** (authentifié) — chiffrement des secrets via crate `aes-gcm` (RustCrypto).
- **Argon2id** — hachage des mots de passe utilisateur (résistant aux attaques par GPU/ASIC).
- **HMAC-SHA1 / SHA256** — génération des codes TOTP (RFC 6238) via crate `totp-rs`.
- **CSPRNG** — génération des sel/IV via `getrandom` (appel direct aux RNG du noyau).

### Politique de code

Un fichier [`clippy.toml`](clippy.toml) applique globalement l'interdiction des appels
`unwrap()` / `expect()` sur toutes les valeurs `Result` et `Option` :

```toml
# extrait de clippy.toml
disallowed-methods = [
  { path = "std::result::Result::unwrap",  reason = "Use typed errors (thiserror) on sensitive paths" },
  { path = "std::result::Result::expect",  reason = "Avoid panics and secret-leaking failure messages" },
  { path = "std::option::Option::unwrap",  reason = "Handle missing values explicitly" },
  { path = "std::option::Option::expect",  reason = "Handle missing values explicitly" }
]
```

Ceci garantit qu'aucune panique imprévue ne peut exposer de données sensibles en production.

### Signalement de vulnérabilités

Consulter [SECURITY.md](SECURITY.md) pour la politique de divulgation responsable.

---

## Structure du dépôt

```text
HeelonVault/
├── crates/
│   ├── heelonvault-core/      # Bibliothèque publique (crates.io)
│   ├── heelonvault-app/       # Binaire GTK4 / libadwaita
│   └── sqlx-shim/             # Shim local SQLx
├── migrations/            # 14 migrations SQL (sqlx)
├── assets/                # Assets GTK embarqués (CSS, icônes, images)
├── resources/             # Ressources non délocalisées (fonts)
├── tests/                 # Tests d'intégration Rust
├── docs/                  # Documentation technique et architecture
├── data/                  # Base de données dev locale
├── logs/                  # Logs runtime
├── Cargo.toml             # Workspace root
├── clippy.toml            # Politique Clippy sécurité
├── LICENSE                # Licence Apache 2.0
├── docs/THIRD_PARTY_LICENSES.md  # Inventaire des bibliothèques tierces
├── SECURITY.md            # Politique de divulgation
├── scripts/run-dev.sh     # Lancement développement
├── scripts/run.sh         # Lancement production (généré par les scripts d'installation)
├── scripts/install.sh     # Installateur unifié (détection OS)
├── scripts/install-ubuntu.sh      # Installateur Ubuntu / Debian
├── scripts/install-rhel.sh        # Installateur Fedora / RHEL / Rocky / AlmaLinux
├── scripts/remove.sh      # Désinstallateur unifié (détection OS)
├── scripts/remove-ubuntu.sh       # Désinstallateur Ubuntu / Debian
└── scripts/remove-rhel.sh         # Désinstallateur Fedora / RHEL / Rocky / AlmaLinux
```

> **Premium** : `heelonvault-premium` est maintenu dans un dépôt privé séparé.
> La version communautaire de ce dépôt n'y accède pas.

---

## Lancement rapide

### Développement

```bash
./scripts/run-dev.sh
```

Base de données dev : `data/heelonvault-rust-dev.db`

### Vérification build et lint

```bash
cargo check --workspace
cargo clippy --workspace -- -D warnings
```

### Installation Linux packagée

Le tarball de release (`heelonvault-linux-x86_64.tar.gz`) installe :

- le binaire dans `/opt/heelonvault/`;
- un lanceur GNOME `com.heelonvault.rust.desktop` (App ID GTK correspondant);
- les icônes dans le thème hicolor système;
- un profil de déploiement à choisir pendant l'installation :
  - **Personnel** : base SQLite `~/.local/share/heelonvault/heelonvault-rust.db`, logs `~/.local/state/heelonvault/logs`;
  - **Entreprise** : base SQLite `/var/lib/heelonvault/heelonvault-rust.db`, logs `/var/log/heelonvault`.

```bash
tar -xzf heelonvault-linux-x86_64.tar.gz
cd heelonvault-linux-x86_64
sudo ./scripts/install.sh
```

Prévisualisation sans modifier le système (dry-run) :

```bash
sudo env HEELONVAULT_DRY_RUN=1 ./scripts/install.sh
```

En cas de besoin (forçage explicite), vous pouvez lancer `scripts/install-ubuntu.sh` ou `scripts/install-rhel.sh`.

Sécurité release : si le fichier `heelonvault.sha256` est présent dans l'archive, l'installateur vérifie automatiquement l'intégrité du binaire avant installation.

Note mode Entreprise : l'installateur configure uniquement les chemins système partagés.
La publication réseau (RDS/VDI/RemoteApp, reverse proxy, bastion, etc.) reste à réaliser manuellement.
Pour des performances optimales, la base de données du mode Entreprise doit résider sur un stockage à faible latence, idéalement local au serveur d'exécution.

Désinstallation :

```bash
sudo ./scripts/remove.sh
```

En cas de besoin, vous pouvez lancer `scripts/remove-ubuntu.sh` ou `scripts/remove-rhel.sh` explicitement.

Consulter [QUICKSTART.md](docs/QUICKSTART.md) pour les détails post-installation.

### Tests

```bash
cargo test
```

---

## Documentation

| Fichier | Contenu |
| ------- | ------- |
| [CHANGELOG.md](docs/CHANGELOG.md) | Journal des modifications (FR) |
| [CHANGELOG.en.md](docs/CHANGELOG.en.md) | Changelog (EN) |
| [QUICKSTART.md](docs/QUICKSTART.md) | Installation et premiers pas |
| [QUICKSTART.fr.md](docs/QUICKSTART.fr.md) | Guide de démarrage rapide (FR) |
| [docs/README.md](docs/README.md) | Index central de la documentation bilingue |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Architecture technique détaillée |
| [docs/ARCHITECTURE.en.md](docs/ARCHITECTURE.en.md) | Technical architecture (EN) |
| [docs/USER_GUIDE.md](docs/USER_GUIDE.md) | Guide utilisateur détaillé |
| [docs/USER_GUIDE.en.md](docs/USER_GUIDE.en.md) | User guide (EN) |
| [docs/UPDATE_GUIDE.md](docs/UPDATE_GUIDE.md) | Procédure de mise à jour |
| [docs/UPDATE_GUIDE.en.md](docs/UPDATE_GUIDE.en.md) | Production update guide (EN) |
| [SECURITY.md](SECURITY.md) | Politique de sécurité et divulgation |
| [SECURITY.fr.md](SECURITY.fr.md) | Politique de sécurité (FR) |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Contribution guide (EN) |
| [CONTRIBUTING.fr.md](CONTRIBUTING.fr.md) | Guide de contribution (FR) |
| [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) | Code de conduite (FR) |
| [CODE_OF_CONDUCT.en.md](CODE_OF_CONDUCT.en.md) | Code of Conduct (EN) |
| [LICENSE](LICENSE) | Licence Apache 2.0 |
| [THIRD_PARTY_LICENSES.md](docs/THIRD_PARTY_LICENSES.md) | Inventaire complet des dépendances tierces |
| [THIRD_PARTY_LICENSES.fr.md](docs/THIRD_PARTY_LICENSES.fr.md) | Guide FR des licences tierces |

---

> Les notes de version détaillées sont dans [CHANGELOG.md](docs/CHANGELOG.md).
