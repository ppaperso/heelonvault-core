# Guide de développement

Langue : FR | [EN](DEVELOPMENT.md)

Construire, lancer et tester HeelonVault depuis les sources. Pour les règles de contribution,
voir [CONTRIBUTING.fr.md](../../CONTRIBUTING.fr.md) ; pour le fonctionnement interne,
[ARCHITECTURE.md](../ARCHITECTURE.md).

## Prérequis

- **Rust** : installez [rustup](https://rustup.rs/) ; la toolchain `1.99.1` épinglée par
  `rust-toolchain.toml` est sélectionnée automatiquement.
- **Fichiers de développement GTK4 et libadwaita** :
  - Ubuntu / Debian : `sudo apt install build-essential pkg-config libgtk-4-dev libadwaita-1-dev`
  - Fedora : `sudo dnf install gcc pkgconf-pkg-config gtk4-devel libadwaita-devel`
- **`heelonvault-premium`** cloné dans un dossier voisin (`../heelonvault-premium`).
  `heelonvault-app` le déclare comme dépendance optionnelle par chemin, et Cargo charge le
  manifeste de toute dépendance par chemin, même optionnelle : **sans ce clone, le workspace ne
  se charge pas**, y compris avec `--no-default-features`. Les entrées `[patch]` du `Cargo.toml`
  racine pointent vers ce même dossier. Sans lui, seule `heelonvault-core` peut être vérifiée,
  avec `scripts/check-core-standalone.sh` (voir [CONTRIBUTING.fr.md](../../CONTRIBUTING.fr.md)).

```bash
rustup toolchain install 1.99.1 --profile minimal --component rustfmt --component clippy
rustc --version   # 1.99.1
```

La version minimale supportée reste Rust 1.98 (`rust-version` dans les manifestes).
La CI vérifie aussi la compilation sous `1.98.0` avec `--locked`. La toolchain
épinglée sert au développement et aux livraisons ; `rustup update stable` seul
ne la remplace pas. Aucune modification du compilateur global par défaut n'est
nécessaire.

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

Pour les tests de régression GTK natifs sous Linux :

```bash
dbus-run-session -- bash scripts/run-wayland-tests.sh \
  cargo test --locked --workspace --all-features native_ -- --ignored --nocapture
```

Installer `labwc`, `wlr-randr`, `wayland-utils`, `dbus-daemon` et `at-spi2-core`.
La CI lance la même commande avec un répertoire runtime privé et un compositeur Wayland
répondant, muni d'un siège d'entrée et d'une sortie 1920×1600. Aucun serveur X n'est requis.
`G_DEBUG=fatal-warnings` est obligatoire ; les assertions de largeur compacte restent actives.
Le module de saisie simple isole les tests de routage clavier du démon IBus de la machine ;
il ne valide pas l'intégration IBus. Pour tester Wayland et le moteur de rendu par défaut
dans une session graphique, lancer `G_DEBUG=fatal-warnings GTK_IM_MODULE=simple
GDK_BACKEND=wayland cargo test --locked --workspace --all-features native_ -- --ignored --nocapture`.
Les réglages de saisie et de rendu en production ne changent pas.
Wayland est la seule certification graphique Linux bloquante. X11 n'est pas certifié
([known-issue #76](https://github.com/ppaperso/heelonvault-core/issues/76)) ;
réévaluer la compatibilité XWayland avant la première externalisation. Voir
l'[audit des backends](GTK_BACKEND_QUALIFICATION.md). Les contrôles Rust/sécurité et Windows restent requis.

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
sudo ./scripts/install.sh            # voir docs/UPDATE_GUIDE.md
# avec CARGO_TARGET_DIR défini :
sudo env HEELONVAULT_BINARY="$CARGO_TARGET_DIR/release/heelonvault" ./scripts/install.sh
```

Les paquets officiels (AppImage, MSI) et le SBOM sont construits par la CI à chaque tag
`vX.Y.Z` : voir [RELEASING.md](RELEASING.md). Spécificités Windows :
[RUNBOOK_WINDOWS_PACKAGING.md](RUNBOOK_WINDOWS_PACKAGING.md).

Linux est prioritaire, suivi de Windows 11. Le workflow DMG macOS est conservé mais désactivé,
y compris en lancement manuel, pendant la pause de cette plateforme.

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
