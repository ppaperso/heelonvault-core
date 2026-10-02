# Guide de développement

Langue : FR | [EN](DEVELOPMENT.md)

Construire, lancer et tester HeelonVault depuis les sources. Pour les règles de contribution,
voir [CONTRIBUTING.fr.md](../../CONTRIBUTING.fr.md) ; pour le fonctionnement interne,
[ARCHITECTURE.md](../ARCHITECTURE.md).

## Prérequis

- **Rust** : installez [rustup](https://rustup.rs/) ; la toolchain `1.98.0` épinglée par
  `rust-toolchain.toml` est sélectionnée automatiquement.
- **Fichiers de développement GTK4 et libadwaita** :
  - Ubuntu / Debian : `sudo apt install build-essential pkg-config libgtk-4-dev libadwaita-1-dev`
  - Fedora : `sudo dnf install gcc pkgconf-pkg-config gtk4-devel libadwaita-devel`
- **`heelonvault-premium`** cloné dans un dossier voisin (`../heelonvault-premium`).
  `heelonvault-app` le déclare comme dépendance optionnelle par chemin, et Cargo charge le
  manifeste de toute dépendance par chemin, même optionnelle : **sans ce clone, le workspace ne
  se charge pas**, y compris avec `--no-default-features`. Les entrées `[patch]` du `Cargo.toml`
  racine pointent vers ce même dossier.

```bash
rustc --version   # 1.98.0
```

## Lancer en développement

```bash
./scripts/run-dev.sh
```

- base : `data/heelonvault-rust-dev.db` (créée au premier lancement) ;
- logs : `./logs`, niveau `debug`.

`run-dev.sh` impose toujours `HEELONVAULT_DB_PATH`, `HEELONVAULT_LOG_DIR` et
`HEELONVAULT_LOG_LEVEL` : on ne peut pas les surcharger à travers lui. `RUST_LOG` reste
prioritaire sur le niveau ; pour d'autres chemins, lancer le binaire directement :

```bash
RUST_LOG=info,heelonvault::ui=debug ./scripts/run-dev.sh
HEELONVAULT_DB_PATH=/tmp/hv-dev.db HEELONVAULT_LOG_DIR=/tmp/hv-logs cargo run -p heelonvault-app
```

## Tests

Les tests d'intégration vivent dans `crates/heelonvault-core/tests/` et utilisent une vraie base
SQLite (utilitaires dans `tests/common/mod.rs`). `crates/heelonvault-app/tests/` contient les
contrôles de cohérence du dépôt : numéro de version commun, et présence dans les deux catalogues
de chaque clé de traduction utilisée par l'interface.

```bash
cargo test --workspace
cargo test -p heelonvault-core --test security_crypto       # un fichier de tests
cargo test -p heelonvault-core nom_du_test                  # un test
```

## Contrôles qualité (ceux de la CI)

`.cargo/config.toml` impose `-D warnings` et `-D unsafe_code` ; `clippy.toml` interdit
`unwrap()` / `expect()`. Avant de pousser :

```bash
cargo fmt -p heelonvault-core -p heelonvault-app -p sqlx -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo clippy-secure
cargo test --workspace --locked
cargo semver-checks -p heelonvault-core
cargo publish --dry-run -p heelonvault-core --locked
cargo audit
cargo deny check advisories
```

`cargo fmt --all` reformaterait aussi le dépôt premium voisin : formater paquet par paquet.

Quand les dépendances changent, régénérer le SBOM et le commiter (sinon le job CI `check-sbom`
échoue) :

```bash
./scripts/generate-sbom.sh
```

## Build release et installation système Linux

```bash
cargo build --release -p heelonvault-app
cp target/release/heelonvault .
sudo ./scripts/install.sh            # voir docs/UPDATE_GUIDE.md
```

Les paquets officiels (AppImage, DMG, MSI, SBOM) sont construits par la CI à chaque tag
`vX.Y.Z` : voir [RELEASING.md](RELEASING.md). Spécificités Windows :
[RUNBOOK_WINDOWS_PACKAGING.md](RUNBOOK_WINDOWS_PACKAGING.md).

Licence premium en développement : sous Linux, un binaire lancé depuis `target/debug` ou
`target/release` lit `~/.config/heelonvault/license.hvl` au lieu de `/etc/heelonvault/license.hvl`.

## Dépannage

- **Fenêtre vide ou défauts de rendu** : forcer le rendu GL, `GSK_RENDERER=gl ./scripts/run-dev.sh`.
- **Base de dev au schéma obsolète** : la mettre de côté, une nouvelle est créée au lancement
  suivant : `mv data/heelonvault-rust-dev.db data/heelonvault-rust-dev.db.bak`.
- **`pkg-config` ne trouve pas `gtk4` ou `libadwaita-1`** : il manque les paquets de
  développement ci-dessus.
- **Pas de logs** : vérifier `HEELONVAULT_LOG_DIR`, puis relancer avec `RUST_LOG=debug`.

## Conventions de documentation

- `docs/` est publié sur [doc.heelonvault.heelonys.fr](https://doc.heelonvault.heelonys.fr) ;
  `docs/internal/` ne l'est pas (voir [README.md](README.md)).
- Les documents bilingues commencent par `Langue : FR | [EN](…)` ou `Language: EN | [FR](…)` :
  le site s'en sert pour détecter la langue.
- Ne pas écrire de numéro de version ni de « nouveau en vX » dans les documents : le journal des
  modifications porte l'historique.
- Un nouveau libellé statique de l'interface doit aussi être réappliqué dans le `refresh_i18n`
  concerné.
