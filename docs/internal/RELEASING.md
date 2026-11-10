# Livrer une version de HeelonVault

Langue : FR | [EN](RELEASING.en.md)

## Règle de version

Un **seul numéro de version** pour tout l'ensemble :

- le produit (`heelonvault-app`, tags `vX.Y.Z`, releases GitHub, changelog) ;
- la crate `heelonvault-core`, publiée sur [crates.io](https://crates.io/crates/heelonvault-core) ;
- `heelonvault-premium`, qui dépend de `heelonvault-core = "X.Y"`.

Le numéro suit [semver](https://semver.org/lang/fr/), appliqué à l'**API publique de `heelonvault-core`** :

| Changement dans `heelonvault-core` | Version |
|---|---|
| Rupture : élément public supprimé ou renommé, signature modifiée, méthode obligatoire ajoutée à un trait public, variante ajoutée à un enum exhaustif | **majeure** (`X+1.0.0`) |
| Ajout compatible : nouvelle fonction, nouveau type, méthode de trait avec implémentation par défaut, variante d'un enum `#[non_exhaustive]` | **mineure** (`X.Y+1.0`) |
| Correctif sans changement d'API | **correctif** (`X.Y.Z+1`) |

Pourquoi : une dépendance `heelonvault-core = "1.1"` accepte toute version de `1.1.0` à `2.0.0` exclu. Cargo installe donc une nouvelle mineure sans rien demander ; si elle casse l'API, le code de l'utilisateur ne compile plus.

Une rupture d'API oblige donc à passer **tout le produit** en majeure. Pour qu'elles restent rares :

- marquer `#[non_exhaustive]` les enums publics appelés à grandir (c'est déjà le cas de `AppError`, `AccessDeniedReason`, `RecoveryFailure`, `AuditAction`) ;
- donner une implémentation par défaut aux méthodes ajoutées à un trait public, quand c'est possible ;
- déprécier avant de supprimer : `#[deprecated]` dans une mineure, suppression dans la majeure suivante.

Les pré-versions (`X.Y.Z-rc.N`) sont taguées pour produire les paquets de test ; elles ne sont pas publiées sur crates.io.

## Garde-fous automatiques

- **Référence de compatibilité** — `cargo test --locked -p heelonvault-core --test release_2_1_1_compatibility --test recovery_phrase_compatibility` : schéma historique figé par empreinte, déchiffrement des quatre types depuis des vecteurs synthétiques fixes et sauvegarde/restauration dans une base distincte. Ne pas régénérer les vecteurs pour faire passer une modification de format. Les anciennes références de documents restent des chemins, pas des fichiers importés.
- **CI — `cargo semver-checks -p heelonvault-core`** : compare l'API à la dernière version publiée sur crates.io. Échoue si la version déclarée ne couvre pas les ruptures.
- **CI — `cargo publish --dry-run -p heelonvault-core`** : la crate doit se construire seule, avec ses dépendances crates.io, hors du workspace.
- **Test `crates/heelonvault-app/tests/release_consistency.rs`** : core, app et premium portent la même version ; premium dépend de cette version de core ; les quatre changelogs (core et premium, FR et EN) ont une section pour elle.

## Checklist de livraison

**Blocage crates.io — [#75](https://github.com/ppaperso/heelonvault-core/issues/75).**
La correction de RUSTSEC-2026-0459 utilise le commit SQLx officiel
`10462fa4414c6c7f1ed1d39bb31678e3b726878c` et `libsqlite3-sys` corrigé dans les deux dépôts.
Exception approuvée : responsable @ppaperso, retrait au plus tard le 16 novembre 2026,
et avant toute publication crates.io. Remplacer les patches Git par une version SQLx publiée,
retirer l'autorisation de source Git et régénérer les deux lockfiles et le SBOM.
La validation des paquets desktop peut continuer ; un dry-run Cargo n'annule pas ce blocage :
les patches de workspace ne sont pas transmis aux utilisateurs de la crate publiée.

**Qualification graphique Linux : Wayland uniquement — [#76](https://github.com/ppaperso/heelonvault-core/issues/76).**
La passe Wayland avec `G_DEBUG=fatal-warnings` est la seule certification graphique Linux
bloquante ; les contrôles Rust/sécurité et la recette Windows restent obligatoires.
X11 non certifié — mono-utilisateur Wayland — à réévaluer à la première externalisation
de l'application. Les avertissements GDK observés sous GTK 4.24.1/X11 restent documentés :
aucun filtre ni assouplissement des assertions n'est autorisé. Aucun correctif X11 amont
n'est requis pour sortir du périmètre actuel ; XWayland est la seule éventuelle cible
de compatibilité X11 à réévaluer, pas une certification acquise. Voir l'[audit](GTK_BACKEND_QUALIFICATION.md).

**Périmètre actuel : Linux en priorité et Windows 11.** Le développement et la diffusion macOS
sont en pause : `macos-dmg-rc.yml` n'a plus de déclencheur sur les tags et son job est désactivé
par `if: ${{ false }}`, même en lancement manuel. Aucun DMG ne doit être ajouté aux nouvelles
releases. Les artefacts historiques ne sont pas supprimés, mais ne sont pas recommandés en
production. La reprise sera réévaluée après validation et déploiement de Linux/Windows 11 sur
d'autres machines. Elle exige de corriger les contrôles de fenêtres et le presse-papiers,
de valider le DMG sur Mac, puis de rétablir explicitement le job, son déclencheur et la documentation.

1. **Versions** — dans `crates/heelonvault-core/Cargo.toml`, `crates/heelonvault-app/Cargo.toml` et `heelonvault-premium/Cargo.toml` (`version` et `heelonvault-core = "X.Y"`).
2. **Lockfiles** — `git diff Cargo.lock` (dans les deux dépôts) ne doit montrer que les lignes de version de nos crates. Si Cargo a aussi changé d'autres dépendances, restaurer le fichier, modifier ces lignes à la main, puis vérifier avec `cargo check --workspace --locked`.
3. **Documentation** — changelogs FR/EN des deux dépôts, titres des README (`# HeelonVault X.Y.Z`), `crates/heelonvault-core/README.md` (ligne `heelonvault-core = "X.Y"`). Les autres documents ne portent volontairement pas de numéro de version : ne pas en rajouter. Si un comportement visible change, mettre à jour `docs/QUICKSTART*.md`, `docs/USER_GUIDE*.md` ou `docs/UPDATE_GUIDE*.md`, et dérouler la recette [MANUAL_QA.md](MANUAL_QA.md).
   Régénérer et relire les captures du guide selon [USER_GUIDE_SCREENSHOTS.md](USER_GUIDE_SCREENSHOTS.md), puis exécuter `bash scripts/check-user-guide-assets.sh --release`. Le manifeste doit correspondre aux sources et à la version préparée ; les captures Linux ne remplacent pas la recette native Windows 11, ni celle de macOS avant sa reprise.
4. **SBOM** — `./scripts/generate-sbom.sh`, puis commiter `sbom.cyclonedx.json`.
5. **Vérifications locales** (depuis `heelonvault-core`) :
   ```bash
   cargo fmt -p heelonvault-core -p heelonvault-app -p sqlx -- --check
   cargo clippy --workspace --all-targets --all-features -- -D warnings
   cargo test --workspace --locked
   cargo check -p heelonvault-app --features licensing
   cargo semver-checks -p heelonvault-core
   cargo publish --dry-run -p heelonvault-core --locked
   ```
6. **Merger premium en premier.** La CI de core récupère la branche `main` de premium : tant que celle-ci exige l'ancienne version de core, la PR core ne compile pas. Merger la PR premium, relancer la CI de la PR core, puis la merger.
7. **Publier la crate** depuis `main` à jour : `cargo publish -p heelonvault-core`. Une publication est définitive (on peut seulement la retirer avec `cargo yank`, sans la supprimer).
8. **Taguer** le commit de merge de core : `git tag vX.Y.Z && git push origin vX.Y.Z`. Le tag déclenche les workflows AppImage, MSI et SBOM, qui créent la release GitHub ; le workflow DMG reste en pause.
9. **Vérifier** la release GitHub (artefacts et sommes SHA-256), la page crates.io et la documentation sur [docs.rs](https://docs.rs/heelonvault-core).
10. **Site de documentation** — [doc.heelonvault.heelonys.fr](https://doc.heelonvault.heelonys.fr) se resynchronise chaque jour sur la dernière release (dépôt `Heelonys_webdoc`, workflow `sync-docs`). Pour publier tout de suite, lancer ce workflow à la main. Seuls `docs/` (hors `docs/internal/`) et les `README`, `SECURITY`, `CONTRIBUTING`, `CODE_OF_CONDUCT` racine sont publiés.
