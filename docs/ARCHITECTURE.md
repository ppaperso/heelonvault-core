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

HeelonVault est organisé en **workspace Cargo** (modèle Open Core) :

```text
HeelonVault/
├── crates/
│   ├── heelonvault-core/          # Bibliothèque publique (crates.io v1.1.0)
│   ├── heelonvault-app/           # Binaire GTK4 (assembleur Open Core)
│   └── sqlx-shim/                 # Shim local SQLx (publish = false)
├── migrations/                    # Migrations SQL appliquées au démarrage
├── assets/                        # Assets GTK embarqués (CSS, icônes)
├── resources/                     # Ressources non délocalisées (fonts)
├── tests/                         # Tests d'intégration
├── docs/                          # Documentation technique
├── Cargo.toml                     # Workspace root (resolver = "2")
├── .cargo/config.toml             # Flags de compilation
├── scripts/run.sh                 # Launcher production
├── scripts/run-dev.sh             # Launcher développement
├── scripts/install.sh             # Installation unifiée (détection OS)
├── scripts/install-core.sh        # Bibliothèque commune install Linux
├── scripts/install-ubuntu.sh      # Installation Ubuntu / Debian
├── scripts/install-rhel.sh        # Installation Fedora / RHEL / Rocky / AlmaLinux
├── scripts/remove.sh              # Désinstallation unifiée (détection OS)
├── scripts/remove-core.sh         # Bibliothèque commune désinstallation Linux
├── scripts/remove-ubuntu.sh       # Désinstallation Ubuntu / Debian
├── scripts/remove-rhel.sh         # Désinstallation Fedora / RHEL / Rocky / AlmaLinux
└── docs/
```

> **Premium** : `heelonvault-premium` est maintenu dans un dépôt Git privé séparé
> (`ppaperso/heelonvault-premium`). Il est référencé dans `heelonvault-app`
> comme dépendance git optionnelle (`features = ["licensing"]`). Le build
> communautaire n'y accède jamais.

## Flux de demarrage

1. `main.rs` applique les variables runtime de rendu GTK (dont `GSK_RENDERER`) avant de lancer Tokio.
2. `main.rs` initialise le runtime tokio.
3. Ouverture de la base SQLite via `HEELONVAULT_DB_PATH`.
4. Application des migrations SQL.
5. Construction des repositories/services.
6. Initialisation UI, authentification, puis fenêtre principale.
7. Chargement des secrets et activation de la politique de session.

En installation Linux packagée, `run.sh` exporte explicitement `HEELONVAULT_MIGRATIONS_DIR=/opt/heelonvault/migrations`.
Le flux d'installation valide la copie des migrations (noms + contenu) et échoue si le dossier est absent/invalide.

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
- changement de mot de passe maître via `rotate_master_key_hardened`:
  - rewrap des enveloppes de clés de coffres owner/shared,
  - application atomique SQL des mutations critiques,
  - validation pré/post rotation en mode `VaultAndSampleSecret`;
- préférence utilisateur persistée `show_passwords_in_edit` pour l'édition des secrets de type mot de passe.

## Import CSV (pipeline)

Le flux d'import CSV combine une UX guidée et un traitement métier tolérant aux erreurs:

- UI en 3 phases: prévisualisation, progression, résumé final;
- dialogue dédié `import_progress_dialog` pour le suivi live;
- traitement ligne par ligne côté service avec bilan agrégé (`imported`, `failed`, détails par ligne);
- génération d'un rapport de rejets `csv_import_rejects_*.txt` dans `HEELONVAULT_LOG_DIR` (ou `./logs` par défaut) lorsque des lignes sont rejetées.

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

Depuis la racine du dépôt :

```bash
# Build communautaire
cargo check --workspace
cargo test --workspace

# Build premium (nécessite l'accès au dépôt privé ou le patch local déclaré dans Cargo.toml)
cargo check -p heelonvault-app --features licensing
```

## Notes migration

- Le runtime et les scripts operationnels actifs sont Rust-only.
- Des artefacts legacy peuvent subsister (ex. repertoires vides), sans impact sur l'execution courante.
- Les docs et scripts operationnels doivent rester alignes sur le flux Rust-only.

## Décision architecture - Supply chain zero warning (P2)

Contexte :

- `cargo audit` remontait des crates non maintenues/yanked dans la chaîne PDF historique.
- La politique projet cible `0 warning` (aucune allowlist permanente).

État courant :

- ✅ **RUSTSEC-2023-0071 éliminé** : le crate `rsa` (timing side-channel PKCS#1 v1.5) a été supprimé de l'arbre de dépendances lors de la mise à jour sqlx 0.8 → 0.9 (Phase 5e). `cargo audit` : **0 vulnérabilité** sur 431 dépendances.
- ⏳ **PDF** : la dépendance `genpdf` historique reste à traiter (aucune advisory active à ce jour, mais chaîne peu maintenue). La décision de la remplacer par un writer PDF minimal interne est maintenue.

Décision retenue pour PDF :

1. Remplacer `genpdf` par une architecture PDF maintenue ou un writer interne minimal.
2. Supprimer les features de dépendances transitives inutiles.
3. Enforcer en CI une politique bloquante sur advisories, crates yanked et non maintenues.

Contraintes d'implémentation :

- conserver la génération de rapport d'audit PDF (pas de régression fonctionnelle) ;
- conserver hash SHA-256 + signature Ed25519 dans le document ;
- valider Linux/Fedora/macOS/Windows avant merge.

Définition de done (obligatoire) :

- `cargo audit` => 0 warning ;
- `cargo clippy --all-targets --all-features -- -D warnings` => OK ;
- CI multi-plateforme => verte ;
- aucune exception permanente ajoutée dans la policy.
