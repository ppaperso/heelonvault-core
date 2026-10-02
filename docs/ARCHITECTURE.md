# Architecture du projet

Langue : FR | [EN](ARCHITECTURE.en.md)

Ce document décrit le fonctionnement interne de HeelonVault : organisation du code, démarrage,
modèle de clés, garanties de sécurité à l'exécution. Il s'adresse aux évaluateurs sécurité, aux
administrateurs et aux contributeurs. Pour installer ou utiliser l'application, voir
[QUICKSTART.fr.md](QUICKSTART.fr.md) et [USER_GUIDE.md](USER_GUIDE.md).

## Vue d'ensemble

HeelonVault est une application desktop **local-first** écrite en Rust :

- interface : GTK4 + libadwaita ;
- stockage : SQLite local, schéma versionné par migrations SQLx appliquées au démarrage ;
- chiffrement : AES-256-GCM côté application, dérivation de clé Argon2id ;
- plateformes livrées : Linux x86_64 (AppImage), macOS Apple Silicon (DMG), Windows x64 (MSI) ;
- toolchain : Rust 1.98 (épinglée par `rust-toolchain.toml`), édition 2024.

Aucun serveur n'est requis : les secrets ne quittent pas la machine.

## Couches logiques

```text
UI (gtk4/libadwaita)
  -> Services métier
    -> Repositories (SQLx)
      -> SQLite + migrations
```

## Organisation du code (modèle Open Core)

```text
heelonvault-core/
├── crates/
│   ├── heelonvault-core/        # Bibliothèque publique (crates.io) : modèles, repositories,
│   │                            # services, i18n, erreurs — sans UI ni licence
│   ├── heelonvault-app/         # Binaire GTK4 : assemble core + premium
│   │   ├── src/                 # main.rs (composition), ui/ (fenêtres, dialogues, widgets)
│   │   ├── migrations/          # Migrations SQL appliquées au démarrage
│   │   ├── assets/              # CSS, icônes, images embarqués (GResource)
│   │   └── wix/                 # Définition de l'installeur Windows (MSI)
│   └── sqlx-shim/               # Shim local SQLx (publish = false)
├── linux/  macos/               # Lanceurs AppImage et bundle macOS
├── scripts/                     # Lancement dev, installation Linux système, SBOM
├── docs/                        # Documentation publiée (docs/internal/ : mainteneurs)
├── Cargo.toml                   # Workspace
├── clippy.toml                  # Politique Clippy sécurité
└── rust-toolchain.toml          # Toolchain épinglée
```

- `heelonvault-core` contient tout ce qui ne dépend ni de l'interface ni de la licence. Il
  n'initialise jamais de subscriber `tracing`.
- `heelonvault-app` est l'**assembleur Open Core** : la feature Cargo `premium` (active par
  défaut) est le seul endroit qui choisit entre les implémentations communautaires et celles de
  `heelonvault-premium`.
- `heelonvault-premium` est un composant **propriétaire**, dans un dépôt privé distinct
  (administration multi-utilisateur, équipes, rapport d'audit, vérification de licence). Son code
  est présent dans le binaire livré ; son activation dépend d'une licence signée vérifiée à
  l'exécution. Son code source peut être ouvert en lecture pour audit, sous accord de
  confidentialité (voir [SECURITY.fr.md](../SECURITY.fr.md)).

## Flux de démarrage

1. `main.rs` fixe les variables de rendu GTK (dont `GSK_RENDERER`) **avant** de lancer Tokio.
2. Initialisation du runtime Tokio et des logs.
3. Ouverture de la base SQLite (chemin résolu comme décrit dans [Chemins de données](#chemins-de-données)).
4. Application des migrations SQL. Le dossier est cherché dans cet ordre :
   `HEELONVAULT_MIGRATIONS_DIR`, puis à côté de l'exécutable, puis dans le dossier courant. Les
   paquets AppImage, DMG et MSI renseignent ce chemin vers leurs migrations embarquées.
5. Construction des repositories et services (implémentations premium si la feature est active).
6. Premier lancement : assistant d'initialisation. Sinon : écran de connexion, puis fenêtre
   principale.

### Fin de l'assistant d'initialisation

Une initialisation réussie n'ouvre **pas** de session. `bootstrap_flow.rs` efface la clé de compte
(drop du `BootstrapResult`), positionne le drapeau partagé avec le handler `close_request` (sinon
la fermeture serait traitée comme une annulation et quitterait l'application), ferme l'assistant
puis appelle `on_bootstrap_completed(username)`. `main.rs` repasse en mode connexion et présente
un `LoginDialog` normal, sur lequel `show_account_created()` pré-remplit l'identifiant.

## Modèle de clés

- **Clé de compte** (`services/account_key.rs`) : clé aléatoire de 32 octets qui enveloppe toutes
  les clés de coffre d'un compte. Elle n'est stockée que chiffrée :
  - sous le mot de passe maître (enveloppe de mot de passe, `auth_service`) ;
  - sous la phrase de récupération de 24 mots (dérivation séparée par domaine).
- **Clés de coffre** : une par coffre, enveloppée par la clé de compte du propriétaire et, pour
  un coffre partagé, par celle de chaque membre.
- **Secrets** : chiffrés en AES-256-GCM sous la clé de leur coffre.

Conséquences :

- **Changement de mot de passe maître** (`UserService::change_master_password`) : la clé de
  compte ne change pas, seule l'enveloppe de mot de passe est réécrite. Les coffres et les secrets
  ne sont pas rechiffrés. Un ancien compte (format antérieur à la clé de compte) est migré lors
  de ce changement : clés de coffre, secret TOTP et matériel de récupération sont réenveloppés
  dans une seule transaction (`rekey_service.rs`).
- **Récupération** : la phrase de récupération rouvre la clé de compte et permet de définir un
  nouveau mot de passe sans perdre les coffres (`recovery_service.rs`, `login_dialog/restore_flow.rs`).
  Un vérificateur stocké permet de contrôler la phrase sans la conserver.

## Vue UI principale

La fenêtre principale utilise un `GtkStack` racine pour éviter les dialogues modaux sur les flux
les plus fréquents :

- `entries_view` : liste principale des secrets ;
- `secret_editor_view` : création / modification inline ;
- `profile_view` : page `Profil & Sécurité` ;
- `users_view` / `teams_view` : administration (premium).

La barre latérale reste visible pendant les opérations de profil et d'édition. Le badge profil
ouvre un popover en lecture seule avec l'historique récent des connexions.

### Affichage des secrets : cartes ou liste

`entries_view` repose sur un unique `GtkFlowBox` (`secret_flow`), quel que soit le mode d'affichage (`SecretViewMode::Grid` / `List`, défini dans `ui/view_preferences.rs`) :

- `center::apply_view_mode_to_flow` règle le conteneur (grille multi-colonnes, ou une colonne pleine largeur + classe CSS `main-secret-list`) ;
- `SecretCard::new(data, mode)` construit une carte ou une ligne compacte à partir des mêmes briques (titre, badges, actions), en conservant les mêmes boutons d'action ;
- le mode courant vit dans `FilterRuntime::view_mode` ; la bascule (`events::setup_view_mode_handlers`) persiste le choix puis reconstruit les widgets depuis le dernier chargement (`SecretListCallbacks::rerender`), sans requête ni déchiffrement. Un rechargement complet n'a lieu que si rien n'a encore été chargé.

Filtre, tri, recherche, compteurs et raccourcis clavier ne dépendent donc pas du mode. Le choix est persisté par installation dans `ui_view_preferences.json` (même répertoire que `ui_main_window_state.json`).

### Liste des secrets : aucune valeur en clair dans l'interface

`window/refresh.rs` construit un unique `SecretFlowContext` partagé par deux callbacks (`SecretListCallbacks`) : `reload` (base de données) et `rerender` (widgets seuls). `secret_flow.rs` est découpé en trois temps :

1. **Chargement** (`refresh_secret_flow`, thread dédié) : pour chaque secret, `get_secret()` déchiffre la valeur, dont on ne dérive que `has_secret`, `is_weak` et une empreinte SHA-256 ; le clair reste dans le `SecretBox` du service (effacé au drop). `finalize_rows` marque les doublons puis abandonne les empreintes **dans le thread de chargement**. La copie de la clé maître transmise est un `Zeroizing<Vec<u8>>`.
2. **Rendu** (`render_secret_rows`) : construit cartes ou lignes à partir de `LoadedSecrets` (métadonnées + compteurs d'utilisation vivants, `Rc<Cell<u32>>`), conservé dans le contexte pour `rerender`.
3. **Copie à la demande** (`PasswordCopier::copy`) : instantané de la clé de session (refus si verrouillée), `open_vault_for_user` (revérifie les droits — un partage révoqué n'est plus copiable), `get_secret`, copie via `sensitive_clipboard`, puis effacement. Le bouton est désactivé et un drapeau `in_flight` bloque les ré-entrées (`Ctrl+C` émet `clicked` même sur un bouton insensible).

Invariant : `SecretRowView` / `SecretRowData` ne contiennent **jamais** de valeur secrète. Le login et l'URL ne sont pas chiffrés (`metadata_json`) et restent donc copiables directement. Le surcoût par copie (quelques requêtes SQLite + un déchiffrement AES) est imperceptible.

Limite connue : `get_secret()` relit chaque secret en base alors que `list_by_vault()` vient de le charger (N+1 requêtes). La corriger demande une nouvelle méthode sur le trait public `SecretService` de `heelonvault-core` : reportée à un lot dédié.

### Indicateur d'exposition du presse-papiers

`ui/sensitive_clipboard.rs` publie un `Exposure` (`Idle`, `Decrypting`, `InClipboard { kind, copied_at, expires_at }`) à chaque changement : copie (`copy_sensitive(text, SensitiveKind, délai)`), expiration, `clear_now()`, remplacement du presse-papiers par une autre application (signal `changed` avec `is_local() == false` : la valeur est effacée tout de suite, sans toucher au nouveau contenu), et déchiffrement en cours (`begin_decrypting()` renvoie un garde RAII tenu par `PasswordCopier`).

- `subscribe()` appelle l'abonné immédiatement puis à chaque changement, jusqu'à ce qu'il renvoie `ControlFlow::Break`. Les notifications sont émises **après** libération de l'état interne (un abonné peut rappeler `clear_now()`), et les abonnements créés pendant une notification sont conservés.
- `ui/widgets/clipboard_indicator.rs` : bouton de la barre d'en-tête (icône + anneau dessiné en cairo dans un `DrawingArea`). Il ne détient que des références faibles, si bien que l'abonnement s'éteint avec la fenêtre principale recréée à chaque connexion. L'anneau n'est animé (tick callback) que pendant une exposition ; au repos il ne coûte rien. Un clic appelle `clear_now()`.
- Les libellés ne revendiquent que ce que l'application maîtrise : jamais « aucun secret en mémoire ».

## Session et sécurité à l'exécution

### Déverrouillage rapide par code PIN

- `pin_cache_service` : cache en mémoire de la clé maître, protégé par Argon2id (8 Mio, t=3) +
  AES-256-GCM, **jamais persisté sur disque**.
- `pin_setup_dialog` : activation / désactivation depuis le profil (4 à 8 chiffres).
- `pin_unlock_dialog` : saisie du PIN au déverrouillage après auto-verrouillage.
- Garde-fous : 3 tentatives par cache, expiration dure à 12 h, liaison au `user_id`, nonce
  AES-GCM aléatoire par activation, effacement `zeroize` au `Drop`.
- Badge PIN dans la barre de titre avec minuteur de session (nominal, avertissement, critique).

### Anti force brute

- Par identifiant : délai progressif entre les échecs de connexion.
- Par adresse (`ip_rate_limit_service`, table `login_attempts_ip`) : `IpRateLimitPolicy` par
  défaut à 20 tentatives par fenêtre d'une heure, puis verrouillage d'une heure. Les deux
  politiques sont combinées par `CombinedRateLimitService` ; `cleanup_expired()` purge les
  verrous expirés.
- TOTP : un code valide ne peut pas être rejoué immédiatement.

### Fin de session

- Fermer la fenêtre principale et l'auto-verrouillage provoquent une déconnexion propre et le
  retour à l'écran de connexion (ou au déverrouillage par PIN s'il est actif).
- L'historique des connexions est stocké dans `login_history`.

### Durcissement mémoire

- Les clés transitent dans des types qui s'effacent à la libération (`SecretBox`, `Zeroizing`).
  `try_pin_unlock` renvoie un `Zeroizing<Vec<u8>>` ; le callback de déverrouillage reçoit
  `Option<Zeroizing<Vec<u8>>>`.
- `clippy.toml` interdit `unwrap()` / `expect()` sur `Result` et `Option` : une panique ne doit
  pas pouvoir exposer de donnée sensible dans un message d'erreur.
- `.cargo/config.toml` impose `-D warnings` et `-D unsafe_code` à tout le workspace.

## Import CSV

- UI en 3 phases : prévisualisation, progression (`import_progress_dialog`), résumé final.
- Traitement ligne par ligne, tolérant aux erreurs, avec bilan agrégé (importés, en échec,
  détail par ligne).
- Les lignes rejetées sont écrites dans `csv_import_rejects_*.txt`, dans le dossier des logs.

## Recherche

Champs indexés : titre, login, email, URL, notes, catégorie, tags, type de secret.

- normalisation casse / accents (Unicode) ;
- syntaxe par champ (`email:`, `tag:`, `type:`…), `champ: valeur` équivaut à `champ:valeur` ;
- tolérance légère aux fautes pour les termes suffisamment longs ;
- bouton **MultiCoffre** à gauche de la barre de recherche : recherche dans tous les coffres ou
  seulement dans le coffre actif.

## Chemins de données

Le chemin de la base peut toujours être imposé par `HEELONVAULT_DB_PATH`, celui des logs par
`HEELONVAULT_LOG_DIR`. Sans ces variables :

| Installation | Base de données | Logs |
| ------------ | --------------- | ---- |
| Windows (MSI) | `%LOCALAPPDATA%\Heelonys\HeelonVault\data\heelonvault\data\heelonvault-rust.db` | `…\heelonvault\logs\` |
| macOS (DMG) | `~/Library/Application Support/fr.Heelonys.HeelonVault/heelonvault/data/heelonvault-rust.db` | `…/heelonvault/logs/` |
| Linux (AppImage, installation système « Personnel ») | `~/.local/share/heelonvault/heelonvault-rust.db` | `~/.local/state/heelonvault/logs/` |
| Linux (installation système « Entreprise ») | `/var/lib/heelonvault/heelonvault-rust.db` | `/var/log/heelonvault/` |
| Développement (`scripts/run-dev.sh`) | `data/heelonvault-rust-dev.db` | `./logs/` |

Sous Linux, ce sont les lanceurs (AppRun de l'AppImage, `run.sh` de l'installation système) qui
renseignent ces variables ; sous Windows et macOS, l'application applique elle-même ces défauts.

## Logs

- Rotation quotidienne via `tracing-appender` ; fichiers JSON `heelonvault_YYYYMMDD.log`.
- Niveau : `RUST_LOG` (prioritaire), sinon `HEELONVAULT_LOG_LEVEL`. Défaut : `info` pour un
  build release, `debug` pour un build de développement. Les modules sensibles (crypto, secrets,
  authentification, coffres) restent plafonnés à `warn` sauf s'ils sont nommés explicitement
  dans le filtre.
- Les journaux ne contiennent aucune valeur secrète (vérifié par la suite de tests
  `privacy_no_secret_in_logs`).

## Chaîne d'approvisionnement

- Politique **zéro avertissement** : `cargo audit` et `cargo deny check` doivent être propres
  avant toute fusion, sans exception permanente.
- L'ancienne chaîne PDF (`genpdf`) a été retirée : le rapport d'audit premium utilise un
  générateur PDF interne minimal, qui conserve l'empreinte SHA-256 et la signature Ed25519.
- SBOM CycloneDX du binaire livré, publié et attesté à chaque release : voir
  [SECURITY.fr.md](../SECURITY.fr.md#14-securite-de-la-chaine-dapprovisionnement-et-sbom).
- La crate `heelonvault-core` suit semver : `cargo semver-checks` en CI refuse une rupture d'API
  sans version majeure.

## Tests

Les tests d'intégration vivent dans `crates/heelonvault-core/tests/` et utilisent une vraie base
SQLite. Ils couvrent notamment la cryptographie, l'authentification et la force brute, le
contrôle d'accès, l'injection SQL, l'absence de secret dans les logs, le RGPD (effacement,
portabilité), la sauvegarde et la migration des clés de compte (`account_rekey_integration`).
Pour les lancer, voir [DEVELOPMENT.fr.md](internal/DEVELOPMENT.fr.md).
