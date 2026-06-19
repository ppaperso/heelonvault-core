# Journal des modifications — HeelonVault

Langue: FR | [EN](CHANGELOG.en.md)

Toutes les modifications notables sont documentées ici, par version décroissante.
Format inspiré de [Keep a Changelog](https://keepachangelog.com/).

---

## [Unreleased] — Sprint v1.1.0

### Infrastructure — Edition 2024 et Rust 1.96

- Migration des crates du workspace vers `edition = "2024"`.
- `rust-version` aligné à `1.96` sur `heelonvault-core`, `heelonvault-app` et `sqlx-shim`.
- Ajout de `rust-toolchain.toml` (`1.96.0`) pour un environnement de build/lint homogène local + CI.
- Durcissement de compatibilité Rust 2024: corrections des usages `std::env::*_var` dans les zones de test premium via configuration explicite (sans mutation globale d'environnement).
- Passage clippy 1.96 à zéro warning (`-D warnings`) sur le périmètre core/premium.

### Dépendances

- Mise à jour des dépendances `Cargo.lock` vers les versions compatibles Rust 1.96 les plus récentes.
- Mise à jour de la référence git `heelonvault-premium` consommée par l'app core.
- Validation supply-chain conservée: `cargo audit` (exit 0) et `cargo deny check advisories` (ok).

### 🔒 Sprint 2: Sécurité et Dette Technique

#### Protection contre le Brute-Force par IP

- **Rate Limiting IP-based** : Nouvelle table `login_attempts_ip` pour tracer les tentatives de connexion par adresse IP, indépendamment du username.
- **Politique configurable** : `IpRateLimitPolicy` avec `max_attempts` (20 par défaut), `lock_duration_secs` (3600s), et `window_duration_secs` (3600s).
- **Service combiné** : `CombinedRateLimitService` intègre le rate limiting par username (existante) et par IP (nouvelle) pour bloquer les attaques systématiques.
- **Purge automatique** : `cleanup_expired()` supprime les entrées de lock expirées.
- **Migration** : `0016_ip_rate_limiting.sql` crée la table et l'index.

#### Sécurité Supply-Chain

- **cargo-deny** : Intégration de l'outil d'audit des dépendances pour prévenir les vulnérabilités connues et les problèmes de licences.
- **deny.toml** : Configuration avec vérification des advisories de sécurité, ban des crates problématiques, et validation des licences.

#### Qualité de Code

- **Tests de sécurité SQL Injection** : Nouvelle batterie de tests dans `tests/security_sql_injection.rs` pour valider la résistance aux injections SQL.
- **Correction des imports** : Remplacement de `heelonvault_rust` par `heelonvault_core` dans toute la base de tests (69 occurrences).

### Infrastructure — Polyrepo Open Core (Phase 5f)

- `heelonvault-core v1.1.0` publié sur [crates.io](https://crates.io/crates/heelonvault-core) : la bibliothèque cœur est désormais un artefact public réutilisable.
- `heelonvault-premium` extrait dans un dépôt privé séparé (`ppaperso/HeelonVault-Premium`) : le code premium n'est plus exposé dans le dépôt public.
- `heelonvault-app` référence le premium via dépendance git optionnelle ; le build communautaire (`cargo check --workspace`) ne télécharge jamais le dépôt privé.
- `.cargo/config.toml` : patches locaux configurés pour le développement conjoint (pas de réseau requis en local).
- VSCode workspace mis à jour en mode multi-root (HeelonVault + HeelonVault-Premium).

### Infrastructure — Mise à jour SQLx 0.8 → 0.9 et élimination RSA (Phase 5e)

- Dépendance `sqlx` passée de `0.8` à `0.9` dans `heelonvault-core` et `sqlx-shim` (API compatible, aucune modification applicative).
- **Vulnérabilité RUSTSEC-2023-0071 éliminée** : le crate `rsa` (timing side-channel sur PKCS#1 v1.5) était une dépendance transitive de sqlx 0.8 ; il n'est plus présent dans l'arbre de dépendances.
- `cargo audit` : 0 vulnérabilités actives (431 dépendances analysées).

### Infrastructure — MSRV 1.94 → 1.95

- `rust-version = "1.95"` aligné dans les 4 membres du workspace (`heelonvault-core`, `heelonvault-app`, `heelonvault-premium`, `sqlx-shim`).
- Toolchain stable mise à jour à `1.95.0` (rustc 59807616e, 2026-04-14).
- Aucun nouveau lint Clippy à corriger avec 1.95.0.

### UX tableau de bord — cartes et productivité clavier

- Les cartes secrets utilisent un flux souris/clavier plus prévisible :
  - clic simple = sélection,
  - double-clic = ouverture de l'éditeur.
- Suppression des actions d'édition/suppression directement sur la carte pour réduire le bruit visuel ; les actions de maintenance passent par les écrans dédiés.
- Tri des cartes orienté usage (`usage_count` décroissant) pour prioriser les secrets les plus consultés.
- Ajout/clarification des badges cartes : robustesse, incomplet, doublon, usage.
- Raccourcis globaux sur la carte active : `Ctrl+C` (copie secret), `Ctrl+L` (copie login), `Ctrl+U` (ouverture URL).

### Secrets de santé — marquage manuel et détection locale

- Nouveau marqueur persistant « Accès données de santé » dans le formulaire de création/édition.
- Détection locale automatique à haute confiance pour qualifier certains secrets santé sans configuration utilisateur.
- Nouveau filtre rapide `#sante` dans la recherche pour cibler les secrets marqués/détectés santé.
- Badge « Sante » ajouté sur les cartes concernées pour un repérage immédiat.

### Badge d'état PIN et minuteur de session

- Ajout d'un badge cliquable **« PIN actif »** dans la barre de titre, synchronisé en temps réel avec l'état du cache PIN.
- Trois états visuels progressifs selon le temps restant :
  - **Nominal** (> 2 h) : badge blanc semi-transparent standard.
  - **Avertissement** (≤ 2 h, > 15 min) : bordure et texte ambre — indication visuelle de renouvellement conseillé.
  - **Critique** (≤ 15 min) : fond ambre rempli, texte « PIN · Xm », animation `pulse` 2 s.
- Infobulle permanente sur le badge : _« Session sécurisée par PIN — Expire dans Xh Ym »_.
- Timer GLib 60 s géré par fermeture interne (`glib::timeout_add_local`) : annulation propre via `SourceId::remove()` à chaque chemin de sortie (déconnexion, épuisement des tentatives, quitter).
- Cycle de vie étanche : badge et timer se réinitialisent immédiatement sur tout chemin de sortie ; le double-remove est prévenu par mise à `None` du `SourceId` avant le retour `Break`.
- Méthode `PinCache::remaining(hard_timeout) -> Duration` ajoutée (retourne `ZERO` si déjà expiré).
- Correction : couleur du texte badge illisible sur le fond sombre de la barre de titre (règles CSS `headerbar button.header-pin-badge label`).
- Correction : badge restait « PIN actif » après expiration automatique du cache (synchronisation via `on_pin_state_cb`).
- Correction : bouton « Quitter l'application » manquant sur la fenêtre de déverrouillage PIN.
- Localisation FR/EN : nouvelle clé `pin-tooltip-secure`.

### Déverrouillage rapide par code PIN

- Nouveau service `pin_cache_service` : cache en mémoire de la clé maître protégée par Argon2id (8 Mio, t=3) + AES-256-GCM, jamais persisté sur disque.
- Dialogue `pin_setup_dialog` : activation et désactivation du PIN depuis le profil utilisateur (4 à 8 chiffres).
- Dialogue `pin_unlock_dialog` : saisie du PIN lors du déverrouillage automatique de session.
- Intégration auto-lock : un logout provoque un verrouillage PIN (si actif) plutôt qu'une déconnexion complète, conservant la session en mémoire.
- Sécurité : 3 tentatives maximum par cache, timeout dur 12 h, liaison `user_id` (empêche le rejeu inter-sessions), nonce AES-GCM aléatoire par activation, effacement `zeroize` sur `Drop`.
- Localisation FR/EN complète des messages PIN (entrée, erreur, limite, timeout, activation/désactivation).

### Durcissement mémoire — cycle de vie de la clé maître (PR mémoire #1)

- `try_pin_unlock` retourne désormais `Zeroizing<Vec<u8>>` : la garantie d'effacement est portée par le type.
- Callback `on_unlocked` redesigné en `Option<Zeroizing<Vec<u8>>>` : `Some(clé)` en succès, `None` si le cache est épuisé — supprime l'idiome `Vec::new()` comme signal sémantique.
- Suppression du `key.to_vec()` dans `try_pin_unlock` qui retirait silencieusement la garantie de zeroize.

### Rotation master key (hardening)

- Service `user_service`: flux durci `rotate_master_key_hardened` active avec validation pre/post rotation.
- Rewrap des enveloppes de cles de coffres owner/shared applique via mutation atomique SQL.
- Validation de secrets echantillons branchee dans le mode `VaultAndSampleSecret`.
- Verification manuelle confirmee: changement de master key effectif en execution applicative.
- Verification automatique confirmee: passage large `cargo test` vert.

### UX import CSV (premium)

- `profile_view`: refonte du flux d'import CSV avec 3 etapes utilisateur:
  - previsualisation du fichier (secrets detectes/importables/a revoir),
  - progression visible pendant l'import,
  - bilan final detaille avec les lignes a reprise manuelle.
- `import_service`: import tolerant aux erreurs ligne par ligne avec rapport de synthese (`imported`, `failed`, details par ligne) au lieu d'un echec global opaque.
- `ui/dialogs/import_progress_dialog`: nouveau dialogue dedie a la progression d'import.
- Localisation FR/EN complete des nouveaux messages d'import (preview, progression, resume, erreurs).

### Migration legacy v0.4 -> v1.1

- Ajout du script `scripts/export-legacy-v0.4-to-csv.py` pour exporter les bases legacy vers un CSV compatible import v1.1.
- Support des layouts legacy par `--profile`, `--workspace-uuid` ou `--db-path` + `--salt-path`.

### Dette technique v1.1.0 (issues #5, #6, #7, #8, #9)

- `main`: déplacement de `env::set_var("GSK_RENDERER", "gl")` avant l'initialisation du runtime Tokio.
- `import_service`: suppression du bypass global `clippy::disallowed_methods` et durcissement du parsing CSV (champs requis explicites, erreurs de validation métier).
- `main`: décomposition en sous-unités (flags de démarrage, orchestration runtime/UI, builders de services) pour réduire la complexité et la duplication.
- `team_service`: décomposition de `share_vault_with_team` et `rotate_vault_key` en helpers internes pour isoler résolution de clés membres, persistance des partages, et audit.

### Documentation API (issue #10)

- Ajout/complément de documentation `///` sur les traits de services publics (`vault_service`, `secret_service`) afin de clarifier les préconditions, erreurs et contrats de sécurité.

### Validation-

- Validation automatique exécutée après refactor: `cargo test --workspace` et `cargo clippy --workspace --all-targets -- -D warnings`.

### UX recherche — barre multi-coffre et popover d'aide premium

- `shell.rs`: ajout du bouton toggle **MultiCoffre** à gauche de la barre de recherche ; remplace l'ancienne détection automatique de mode.
- Correction : suppression de `selectable(true)` sur le label du popover d'aide — empêchait le retour du focus à la fenêtre principale après un clic droit (gel apparent de l'interface).
- Correction : `parse_search_terms` — la syntaxe `champ: valeur` (espace après le deux-points) est désormais identique à `champ:valeur`.
- Redesign du popover d'aide (`?`) : structure en trois sections (`caption-heading` + `dim-label` + `monospace`), icône `help-browser-symbolic`, largeur fixe 348 px, `autohide(true)`.
- Refactorisation : `ContentShell` struct avec closure `refresh_i18n` centralisée ; `new_body.inc` délègue tous les mises à jour i18n de la barre en un seul appel.
- Clés i18n FR/EN structurées : `main-search-help-no-prefix-title`, `main-search-help-no-prefix-body`, `main-search-help-prefix-title`, `main-search-help-fields`, `main-search-help-examples`, `main-search-help-fuzzy`.

### Sécurité dépendances (issue #13)

- Mise à jour transitive de la chaîne TLS dans `Cargo.lock`: `rustls-webpki` `0.103.10` -> `0.103.13`, `rustls` `0.23.37` -> `0.23.40`.
- Vérification lockfile: la version vulnérable `rustls-webpki 0.103.10` n'est plus présente.
- Validation post-correction exécutée: `cargo check`, `cargo test --locked --all-targets --no-run`, `cargo clippy --all-targets --all-features -- -D warnings`.
- Impact sécurité attendu: correction de l'alerte Dependabot high et des 2 alertes low associées à `rustls-webpki`, après push sur `main`.

## [1.1.0] — 2026-05-13

### Sécurité authentification et 2FA

- Durcissement du changement de mot de passe: comparaison de l'ancien et du nouveau mot de passe en mode constant-time dans `auth_service`.
- Durcissement du flux login: un mot de passe invalide incrémente désormais correctement `auth_policy.failed_attempts` (les tentatives ne sont plus perdues dans ce chemin).
- Ajout d'un garde anti-rejeu TOTP côté service: un code TOTP déjà validé ne peut pas être réutilisé immédiatement dans la même fenêtre temporelle.

### Anti brute-force et UX de verrouillage

- Ajout d'un backoff progressif sur les tentatives échouées dans `auth_policy_service` (croissance exponentielle bornée), en complément de la fenêtre de verrouillage existante.

### Durcissement import CSV

- Ajout de limites de sécurité sur l'import CSV: taille maximale de fichier, nombre maximal de lignes et longueur maximale par champ.
- Validation stricte des URL importées: seuls les schémas `http://` et `https://` sont acceptés.

### Permissions fichiers sensibles

- Export backup `.bak` et `.hvb`: permissions fichiers durcies en mode propriétaire (`0600` sur Unix).
- Restauration backup: base SQLite restaurée avec permissions durcies (`0600` sur Unix).

### Tests

- Nouveaux tests unitaires ajoutés pour:
  - rejet d'un changement de mot de passe identique,
  - validation URL import CSV,
  - calcul du backoff auth policy.
- Validation complète exécutée: `cargo test` vert après changements.

## [1.0.4] — 2026-04-14

### Sécurité dépendances

- Correction de l'alerte `rand` (issue Dependabot): retrait du chemin vulnérable `rand 0.8.5` du graphe résolu, pin sur `rand 0.9.3`.
- Génération de clé de récupération BIP39 migrée vers entropie `getrandom` explicite, sans feature `bip39/rand`.
- Sortie de l'agrégateur `sqlx` vers un shim SQLite-only (`crates/sqlx-shim`) pour supprimer les dépendances inutiles (`sqlx-mysql`, `sqlx-postgres`) du lockfile/SBOM.

### SBOM, attestation et release

- Industrialisation SBOM CycloneDX 1.4: script local `scripts/generate-sbom.sh`, contrôle CI d'obsolescence (`check-sbom`) et publication release (`generate-sbom-artifact`).
- Publication des artefacts SBOM `sbom.cyclonedx.json` + `sbom.cyclonedx.json.sha256` avec attestation de provenance GitHub (`actions/attest-build-provenance@v4`).
- Homogénéisation des jobs release Linux/Windows/macOS avec checksums et upload via `gh release`.

### CI/CD et robustesse macOS

- Correction du packaging macOS `.app/.dmg` (staging GDK-Pixbuf, symlinks Homebrew résolus, checksum DMG via `shasum -a 256`).
- Ajout explicite de `gdk-pixbuf` dans les dépendances Homebrew des workflows macOS.
- Suppression des warnings Node 20 résiduels en retirant le cache Homebrew `actions/cache@v4` des jobs macOS.

### Documentation et conformité

- Mise à jour de `README.md` / `README.en.md` en 1.0.4 et ajout de la section SBOM signé.
- Alignement de `THIRD_PARTY_LICENSES.md` et `sbom.cyclonedx.json` avec le graphe dépendances final.

## [1.0.3] — 2026-04-10

### Refactoring UI

- Finalisation du découpage des écrans Rust volumineux en modules dédiés pour `login_dialog`, `main_window`, `profile_view` et les flux associés.
- Respect de la contrainte de maintenabilité: aucun fichier UI Rust actif au-dessus de 800 lignes.
- Extraction des helpers de sizing et de sous-composants UI pour réduire le couplage local et clarifier les responsabilités.

### Nettoyage technique

- Suppression des fichiers de split intermédiaires non référencés laissés pendant le refactoring.
- Vérification que les images `assets/images/user-guide` restent uniquement consommées par la documentation, sans embarquement runtime involontaire.

### Validation

- Validation de la version sur `cargo check`, `cargo clippy`, `cargo test` et `cargo fmt --all -- --check`.

### Version

- Passage de la version applicative et documentaire à **1.0.3**.

## [1.0.1] — 2026-04-06

### Documentation produit

- Ajout d'un **guide utilisateur bilingue** (`docs/USER_GUIDE.md` et `docs/USER_GUIDE.en.md`) avec ton orienté manuel utilisateur final.
- Intégration des captures d'écran réelles de l'interface dans les sections écran par écran (initialisation, login, dashboard, création de secrets, profil/sécurité, import/export, administration utilisateurs/équipes, corbeille).
- Structuration du guide avec table des matières et numérotation formelle des écrans/captures.

### CI/CD et packaging Linux

- Smoke test mutualisé (`scripts/smoke-test.sh`) avec mode `--install/--remove`, validation des permissions et des entrées desktop.
- Renforcement des workflows CI/Release : cache Rust, job Fedora en conteneur, checksum externe `.sha256`, attestation de provenance, inclusion des scripts core dans `dist/`.

### Version0

- Bump de la version applicative et documentaire vers **1.0.1**.

## [1.0.0] — 2026-04-02

### Release stable

- Passage officiel en **1.0.0** (sortie stable), suppression du suffixe beta dans la version applicative et la documentation de référence.

### Rapport d'audit PDF

- En-tête premium visuel simplifié: suppression de l'encadré or.
- Nouveau titre principal en noir: **REGISTRE DE TRAÇABILITÉ DES ACCÈS**.
- Journal d'audit exporté sous forme de tableau exploitable (date, action, acteur, cible, détail).

### Traçabilité et lisibilité

- Résolution des identités acteur par nom d'affichage / nom utilisateur dans les exports.
- Enrichissement des cibles d'audit avec noms de coffre et titres de secrets quand disponibles.
- Enrichissement de l'événement `secret.created` avec le titre du secret dans le détail d'audit.

## [0.9.4-beta] — 2026-04-01

### Licence

- Passage de la licence Source-Available propriétaire à **Apache 2.0** : utilisation, modification et redistribution libres ; copyright et marque HEELONYS conservés.

### Système de licence applicative (LicenseService)

- Vérification Ed25519 des licences signées (fichier `~/.config/heelonvault/license.hvl` en dev, `/etc/heelonvault/license.hvl` en prod).
- Format JSON avec champ `payload` (objet JSON ou chaîne sérialisée) et `signature` (hex 128 car. ou base64).
- Fallback automatique sur licence **Community** si aucun fichier n'est présent ou si la vérification échoue.
- Tolérance automatique des espaces et du préfixe `0x` dans les valeurs hexadécimales (`sanitize_hex_input`).
- Journalisation audit `LicenseCheckSuccess` / `LicenseCheckFailure` au démarrage de l'application.

### Badges de licence en interface

- Badge **"Licence free"** / **"Licence pro — CLIENT"** sur la page de login (section hero), visible avant toute authentification.
- Badge de licence dans le bandeau d'en-tête de la fenêtre principale (à côté du badge BETA).
- Style CSS haute-visibilité `.login-license-badge` (dégradé vert sarcelle).
- Clés i18n `license-status-community`, `license-status-professional`, `license-status-invalid` ajoutées en FR/EN.

---

## [0.9.3-beta] — 2026-03-31

### Tableau de bord de sécurité

- Fenêtre de tableau de bord sécurité rendue via WebKitGTK (WebView-first, sans fallback GTK).
- Score de coffre global calculé en temps réel avec évaluation `zxcvbn`.
- Traductions dédiées en FR et EN pour tous les labels du tableau de bord.

### Historique de connexion

- Enregistrement de chaque connexion réussie dans la table `login_history` (migration 0007).
- Affichage de l'historique dans la vue `Profil & Sécurité`.

### Activation TOTP 2FA

- Activation guidée via QR-code dans `Profil & Sécurité`.
- Vérification obligatoire du premier code avant activation définitive.
- Secret TOTP chiffré en base (migration 0009).

### Corrections et robustesse

- Restauration de secret depuis la corbeille : transaction atomique avec restauration automatique du coffre parent si nécessaire (évite l'état "secret invisible").
- Résolution du coffre dans le dialogue d'édition des secrets multi-coffres.
- Correction de la persistance de l'enveloppe de mot de passe au rechargement.

---

## [0.9.2-beta] — 2026-03-27

### Internationalisation et UX

- Sélecteur de langue de login remplacé par des drapeaux FR/EN.
- Correction d'un gel UI lors des changements de langue sur l'écran de login.
- Harmonisation du rafraîchissement i18n dans les zones globales de la fenêtre principale (sidebar, tooltips, placeholders, titres de vues).
- Persistance et application à chaud de la langue utilisateur dans `Profil & Sécurité`.

### Installation, CI/CD et fiabilité release

- Installateur renforcé avec vérification explicite des artefacts critiques (`run.sh`, entrées desktop).
- Installation de deux entrées desktop (`com.heelonvault.rust.desktop` et `heelonvault.desktop`) pour compatibilité environnementale.
- Smoke test installateur ajouté au workflow de release.
- Pipeline CI dédié (`.github/workflows/ci.yml`) : format, lint, build, compilation des tests, validation desktop, smoke test.

### Bootstrap, clé de récupération et sauvegarde sécurisée

- Assistant d'initialisation en 3 étapes dans le dialogue de login : identité → serment (phrase 24 mots) → en attente.
- Phrase mnémotechnique 24 mots (style BIP39) générée à l'initialisation via `BackupService::generate_recovery_key()`.
- Vérification obligatoire de 2 mots tirés au sort avant validation.
- Copie presse-papier avec effacement automatique après 60 secondes.
- Ré-export de la clé de récupération depuis `Profil & Sécurité` pour tout administrateur.
- `BackupApplicationService` : contrôle d'accès RBAC sur les exports/imports `.hvb`.
- Journal d'audit introduit (table `audit_log`, migration 0013).

### Partage équipe, RBAC et UX admin

- Correction du partage de coffre vers une équipe : dérivation de la clé membre depuis `password_envelope` si la clé explicite n'est pas fournie.
- Protection anti faux-positif : échec explicite si aucun membre n'a reçu de clé de coffre.
- Sélecteur explicite de coffre dans le dialogue de partage (plus d'ambiguïté sur la cible).
- Badge ADMIN dans l'en-tête à côté de l'identité connectée.
- Affichage de l'état "coffre partagé" pour les coffres propriétaires.
- Normalisation des labels de badges FR en majuscules.
- Nettoyage i18n : suppression de la clé obsolète `main-vault-shared-badge`.

### Documentation bilingue

- Couverture FR/EN sur l'ensemble des documents Markdown opérationnels.
- Index central de documentation bilingue dans `docs/README.md`.

---

## [0.9.1-beta] — 2026-03-01

### Architecture initiale Rust

- Migration complète de l'architecture Python vers Rust (GTK4 + libadwaita).
- Couche service/repository/model en Rust avec `sqlx` et 9 migrations initiales.
- Authentification Argon2id, chiffrement AES-256-GCM, TOTP RFC 6238.
- Multi-utilisateur avec coffres isolés par utilisateur.
- Recherche multi-champs avec normalisation Unicode.
- Logs structurés JSON rotatifs via `tracing`.
- Politique Clippy sécurité (`clippy.toml`) interdisant `unwrap()`/`expect()` sur les chemins sensibles.
