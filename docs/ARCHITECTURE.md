# Architecture du projet (Rust)

Langue : FR | [EN](ARCHITECTURE.en.md)

Version cible documentée: `1.1.0`

## Vue d'ensemble

HeelonVault est désormais un runtime Rust-only orienté desktop GTK.

- Runtime applicatif: racine du dépôt
- UI desktop: GTK4 + libadwaita
- Base de donnees: SQLite
- Migrations SQL: `sqlx::migrate!` au demarrage
- Launchers scripts: `scripts/run.sh` (prod), `scripts/run-dev.sh` (dev)

## Couches logiques

```text
UI (gtk4/libadwaita)
  -> Services metier
    -> Repositories (SQLx)
      -> SQLite + migrations
```

## Structure active

```text
HeelonVault/
├── src/
│   ├── main.rs                     # Bootstrap runtime + UI
│   ├── ui/                         # Fenetres + dialogues GTK/adw
│   ├── services/                   # Regles metier
│   ├── repositories/               # Acces DB (SQLx)
│   ├── models/                     # Types metier
│   ├── config/                     # Constantes/config runtime
│   └── errors.rs                   # Erreurs applicatives
├── migrations/                     # Migrations SQL appliquees au demarrage
├── tests/                          # Tests integration/securite
├── Cargo.toml
├── scripts/run.sh                  # Launcher production
├── scripts/run-dev.sh              # Launcher developpement
├── scripts/install.sh              # Installation unifiée (détection OS)
├── scripts/install-core.sh         # Bibliothèque commune install Linux
├── scripts/install-ubuntu.sh               # Installation Ubuntu / Debian
├── scripts/install-rhel.sh                 # Installation Fedora / RHEL / Rocky / AlmaLinux
├── scripts/remove.sh               # Désinstallation unifiée (détection OS)
├── scripts/remove-core.sh          # Bibliothèque commune désinstallation Linux
├── scripts/remove-ubuntu.sh                # Désinstallation Ubuntu / Debian
├── scripts/remove-rhel.sh                  # Désinstallation Fedora / RHEL / Rocky / AlmaLinux
├── update.sh                       # Mise a jour Rust-only + backup
└── docs/
```

## Flux de demarrage

1. `main.rs` initialise le runtime tokio.
2. Ouverture de la base SQLite via `HEELONVAULT_DB_PATH`.
3. Application des migrations SQL.
4. Construction des repositories/services.
5. Initialisation UI, authentification, puis fenêtre principale.
6. Chargement des secrets et activation de la politique de session.

## Vue UI principale

La fenêtre principale utilise un `GtkStack` racine pour éviter les dialogues modaux sur les flux les plus fréquents.

- `entries_view`: liste principale des secrets;
- `profile_view`: vue inline `Profil & Sécurité`;
- `secret_editor_view`: vue inline de création / modification.

Conséquences:

- la sidebar reste visible pendant les opérations de profil;
- la création et l'édition de secrets se font dans le panneau central;
- le badge profil n'ouvre plus un écran d'édition, mais un popover read-only avec l'historique récent des connexions.

## Session et sécurité runtime

- fermeture de la fenêtre principale: déconnexion propre et retour à l'écran de login;
- auto-lock: même comportement de déconnexion propre;
- historique de connexions stocké dans `login_history`;
- préférence utilisateur persistée `show_passwords_in_edit` pour l'édition des secrets de type mot de passe.

## Recherche

La recherche principale ne se limite plus au titre et à l'URL.

Champs indexés:

- titre;
- login;
- email;
- URL;
- notes;
- catégorie;
- tags;
- type de secret.

Le moteur applique:

- normalisation casse/accents;
- syntaxe champée (`email:`, `tag:`, `type:`...);
- tolérance légère aux fautes pour les tokens suffisamment longs.

## Chemins de donnees

- Dev: `data/heelonvault-rust-dev.db`
- Base utilisateur packagee: `~/.local/share/heelonvault/heelonvault-rust.db`
- Legacy Python a ne pas toucher: `/var/lib/heelonvault-shared` (hors runtime actif)

## Logs (runtime)

- Rotation journaliere active via `tracing-appender` (un fichier par jour).
- Dossier des logs configurable via `HEELONVAULT_LOG_DIR`.
- Niveau global configurable via `RUST_LOG` (prioritaire) puis `HEELONVAULT_LOG_LEVEL`.
- Defauts launchers:
  - Dev (`run-dev.sh`): `HEELONVAULT_LOG_LEVEL=debug`, `HEELONVAULT_LOG_DIR=./logs`
  - Prod (`run.sh`): `HEELONVAULT_LOG_LEVEL=info`, `HEELONVAULT_LOG_DIR=~/.local/state/heelonvault/logs`
- Fichiers de rotation: `heelonvault_YYYYMMDD.log` dans le dossier configure.

Exemples:

```bash
# Compat standard Rust (prioritaire)
RUST_LOG=info,heelonvault_rust::ui=debug ./scripts/run-dev.sh

# Ou via variable applicative
HEELONVAULT_LOG_LEVEL=warn ./scripts/run.sh
```

## Tests et validation

Depuis la racine du dépôt:

```bash
cargo check
cargo test
```

## Notes migration

- Le runtime et les scripts operationnels actifs sont Rust-only.
- Des artefacts legacy peuvent subsister (ex. repertoires vides), sans impact sur l'execution courante.
- Les docs et scripts operationnels doivent rester alignes sur le flux Rust-only.
