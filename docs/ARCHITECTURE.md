# Architecture du projet (Rust)

Langue : FR | [EN](ARCHITECTURE.en.md)

Version cible documentée: `1.2.0-rc.1`

## Vue d'ensemble

HeelonVault est un runtime Rust-only orienté desktop GTK. La version **1.2.0-rc.1** introduit des améliorations majeures en matière de sécurité, de flux utilisateur et d'infrastructure.

- Runtime applicatif: racine du dépôt
- UI desktop: GTK4 + libadwaita
- Base de données: SQLite
- Migrations SQL: `sqlx::migrate!` au démarrage
- Launchers scripts: `scripts/run.sh` (prod), `scripts/run-dev.sh` (dev)
- **MSRV**: Rust 1.98
- **Édition**: Rust 2024

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
│   ├── heelonvault-core/          # Bibliothèque publique (crates.io v1.2.0-rc.1)
│   ├── heelonvault-app/           # Binaire GTK4 (assembleur Open Core)
│   └── sqlx-shim/                 # Shim local SQLx (publish = false)
├── migrations/                    # Migrations SQL appliquées au démarrage (19 migrations)
├── assets/                        # Assets GTK embarqués (CSS, icônes)
├── resources/                     # Ressources non délocalisées (fonts)
├── tests/                         # Tests d'intégration
├── docs/                          # Documentation technique
├── Cargo.toml                     # Workspace root (resolver = "2")
├── clippy.toml                    # Politique Clippy sécurité
├── rust-toolchain.toml            # Toolchain épinglée sur Rust 1.98.0
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

## Flux de démarrage

1. `main.rs` applique les variables runtime de rendu GTK (dont `GSK_RENDERER`) **avant** de lancer Tokio.
2. `main.rs` initialise le runtime tokio.
3. Ouverture de la base SQLite via `HEELONVAULT_DB_PATH`.
4. Application des migrations SQL (19 migrations en v1.2.0-rc.1).
5. Construction des repositories/services.
6. Initialisation UI, authentification (avec nouveau flux bootstrap refactoré), puis fenêtre principale.
7. Chargement des secrets et activation de la politique de session.

En installation Linux packagée, `run.sh` exporte explicitement `HEELONVAULT_MIGRATIONS_DIR=/opt/heelonvault/migrations`.
Le flux d'installation valide la copie des migrations (noms + contenu) et échoue si le dossier est absent/invalide.

### Bootstrap Flow Refactor (v1.2.0-rc.1)

Le flux d'initialisation a été entièrement refactoré avec les composants suivants :

- **login_dialog/bootstrap_flow.rs** : Gestion du flux d'initialisation en 3 étapes
- **login_dialog/restore_flow.rs** : Flux de restauration avec gestion des clés de récupération
- **account_key.rs** : Service dédié à la gestion des clés de compte
- **recovery_service.rs** : Service de récupération de compte avec validation sécurisée
- **Migration 0019** : `0019_user_recovery_key_envelope.sql` pour la persistance des enveloppes de clés de récupération

Le nouveau système permet :
- La génération et le stockage sécurisé des enveloppes de clés de récupération
- La restauration de compte via la clé de récupération avec validation en deux étapes
- La gestion des clés utilisateur avec rotation sécurisée

### Système de Récupération de Clé de Compte (Account Key Recovery)

Le commit `7cc2556` introduit un système complet de récupération :

- **Services**: `AccountKeyService`, `RecoveryService`, `RekeyService`
- **Repositories**: Extensions de `UserRepository` et `VaultRepository` pour la gestion des enveloppes
- **Tests**: Suite complète dans `tests/account_rekey_integration.rs` (881 lignes)
- **Flux**:
  1. Génération de l'enveloppe de clé de récupération lors du bootstrap
  2. Stockage sécurisé avec chiffrement AES-256-GCM
  3. Validation et restauration via le dialogue dédié

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

### Déverrouillage rapide par code PIN (Nouveau en v1.2.0-rc.1)

- Nouveau service `pin_cache_service`: cache en mémoire de la clé maître protégée par Argon2id (8 Mio, t=3) + AES-256-GCM, jamais persisté sur disque.
- Dialogue `pin_setup_dialog`: activation et désactivation du PIN depuis le profil utilisateur (4 à 8 chiffres).
- Dialogue `pin_unlock_dialog`: saisie du PIN lors du déverrouillage automatique de session.
- Intégration auto-lock: un logout provoque un verrouillage PIN (si actif) plutôt qu'une déconnexion complète, conservant la session en mémoire.
- **Sécurité**: 3 tentatives maximum par cache, timeout dur 12 h, liaison `user_id` (empêche le rejeu inter-sessions), nonce AES-GCM aléatoire par activation, effacement `zeroize` sur `Drop`.
- Badge PIN dans la barre de titre avec minuteur de session (3 états visuels : nominal, avertissement, critique).

### Rotation de clé maître (Hardened)

- Service `user_service`: flux durci `rotate_master_key_hardened` actif avec validation pré/post rotation.
- Rewrap des enveloppes de clés de coffres owner/shared appliqué via mutation atomique SQL.
- Validation de secrets échantillons branchée dans le mode `VaultAndSampleSecret`.
- Vérification manuelle confirmée: changement de master key effectif en exécution applicative.

### Anti Brute-Force par IP (Nouveau en v1.2.0-rc.1)

- **Rate Limiting IP-based**: Nouvelle table `login_attempts_ip` pour tracer les tentatives de connexion par adresse IP.
- **Politique configurable**: `IpRateLimitPolicy` avec `max_attempts` (20 par défaut), `lock_duration_secs` (3600s), et `window_duration_secs` (3600s).
- **Service combiné**: `CombinedRateLimitService` intègre le rate limiting par username (existante) et par IP (nouvelle) pour bloquer les attaques systématiques.
- **Purge automatique**: `cleanup_expired()` supprime les entrées de lock expirées.

### Autres fonctionnalités de sécurité

- fermeture de la fenêtre principale: déconnexion propre et retour à l'écran de login;
- auto-lock: même comportement de déconnexion propre;
- historique de connexions stocké dans `login_history`;
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

### Mode MultiCoffre (Nouveau en v1.2.0-rc.1)

- Bouton toggle **MultiCoffre** ajouté à gauche de la barre de recherche.
- Permet de rechercher dans tous les coffres ou uniquement dans le coffre actif.
- Remplace l'ancienne détection automatique de mode.

Le moteur applique:

- normalisation casse/accents;
- syntaxe champée (`email:`, `tag:`, `type:`...);
- tolérance légère aux fautes pour les tokens suffisamment longs;
- syntaxe unifiée: `champ: valeur` == `champ:valeur`.

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

### Nouvelle suite de tests en v1.2.0-rc.1

- `tests/account_rekey_integration.rs`: Tests complets du système de rekeying (881 lignes)
- `tests/totp_activation_integration.rs`: Tests d'activation TOTP
- Validation: `cargo test --workspace -- --nocapture`

## Notes migration

- Le runtime et les scripts operationnels actifs sont Rust-only.
- Des artefacts legacy peuvent subsister (ex. repertoires vides), sans impact sur l'execution courante.
- Les docs et scripts operationnels doivent rester alignes sur le flux Rust-only.

## Décision architecture - Supply chain zero warning (P2)

Contexte :

- `cargo audit` remontait des crates non maintenues/yanked dans la chaîne PDF historique.
- La politique projet cible `0 warning` (aucune allowlist permanente).

État courant (v1.2.0-rc.1) :

- ✅ **RUSTSEC-2023-0071 éliminé** : le crate `rsa` (timing side-channel PKCS#1 v1.5) a été supprimé de l'arbre de dépendances lors de la mise à jour sqlx 0.8 → 0.9 (Phase 5e).
- ✅ **Corrections de sécurité MSRV 1.98**: `crossbeam-epoch` 0.9.18 → 0.9.20 (RUSTSEC-2026-0204), `webbrowser` 1.2.1 → 1.2.4 (RUSTSEC-2026-0257), `event-listener` 5.4.1 → 5.4.2 (RUSTSEC-2026-0221), remplacement des versions yanked `chacha20` et `spin`.
- ✅ **`cargo audit` : 0 vulnérabilité, 0 warning** sur l'ensemble des dépendances.
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

## Durcissement Mémoire (Nouveau en v1.2.0-rc.1)

### Cycle de vie de la clé maître (PR mémoire #1)

- `try_pin_unlock` retourne désormais `Zeroizing<Vec<u8>>`: la garantie d'effacement est portée par le type.
- Callback `on_unlocked` redesigné en `Option<Zeroizing<Vec<u8>>>`: `Some(clé)` en succès, `None` si le cache est épuisé — supprime l'idiome `Vec::new()` comme signal sémantique.
- Suppression du `key.to_vec()` dans `try_pin_unlock` qui retirait silencieusement la garantie de zeroize.

### Politique Clippy Sécurité

Le fichier [`clippy.toml`](clippy.toml) applique globalement l'interdiction des appels `unwrap()` / `expect()` sur toutes les valeurs `Result` et `Option` :

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

## Infrastructure Open Core

### Publication crates.io

- `heelonvault-core v1.2.0-rc.1` publié sur [crates.io](https://crates.io/crates/heelonvault-core)
- `heelonvault-premium` extrait dans un dépôt privé séparé
- `heelonvault-app` référence le premium via dépendance git optionnelle
- Le build communautaire (`cargo check --workspace`) ne télécharge jamais le dépôt privé

### Patches locaux

```toml
# Cargo.toml (workspace root)
[patch.crates-io]
heelonvault-core = { path = "crates/heelonvault-core" }

[patch.'ssh://git@github.com/ppaperso/heelonvault-premium.git']
heelonvault-premium = { path = "../heelonvault-premium" }
```

## Scripts opérationnels

Tous les scripts sont maintenus en FR/EN :

- `scripts/README.md` et `scripts/README.fr.md` : Documentation des scripts
- `scripts/install.sh` : Installateur unifié avec détection OS
- `scripts/run.sh` / `scripts/run-dev.sh` : Lanceurs production/développement
- `scripts/smoke-test.sh` : Tests de fumée pour validation post-installation
- `scripts/generate-sbom.sh` : Génération du SBOM CycloneDX
- `scripts/generate-license.sh` : Génération des fichiers de licence
- `scripts/export-legacy-v0.4-to-csv.py` : Migration depuis v0.4

## Résumé des changements majeurs v1.2.0-rc.1

| Catégorie | Changement | Impact |
|----------|------------|--------|
| Infrastructure | MSRV Rust 1.96 → 1.98 | Build/lint homogène |
| Infrastructure | Édition 2021 → 2024 | Compatibilité future |
| Sécurité | Système PIN + auto-lock | UX améliorée |
| Sécurité | Rate limiting par IP | Protection brute-force |
| Sécurité | Rotation clé maître durcie | Conformité |
| Sécurité | cargo-deny intégré | Supply-chain hardening |
| Sécurité | Zeroizing pour clés | Protection mémoire |
| UX | Bootstrap flow refactor | Expérience utilisateur |
| UX | Account key recovery | Récupération de compte |
| UX | MultiCoffre toggle | Recherche globale |
| UX | Import CSV 3 étapes | Tolérance aux erreurs |
| UX | Badge PIN + timer | Visibilité session |
| Architecture | 19 migrations SQL | Schéma mis à jour |
| Tests | Suite account_rekey | Validation complète |
