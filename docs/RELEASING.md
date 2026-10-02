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

- **CI — `cargo semver-checks -p heelonvault-core`** : compare l'API à la dernière version publiée sur crates.io. Échoue si la version déclarée ne couvre pas les ruptures.
- **CI — `cargo publish --dry-run -p heelonvault-core`** : la crate doit se construire seule, avec ses dépendances crates.io, hors du workspace.
- **Test `crates/heelonvault-app/tests/release_consistency.rs`** : core, app et premium portent la même version ; premium dépend de cette version de core ; les quatre changelogs (core et premium, FR et EN) ont une section pour elle.

## Checklist de livraison

1. **Versions** — dans `crates/heelonvault-core/Cargo.toml`, `crates/heelonvault-app/Cargo.toml` et `heelonvault-premium/Cargo.toml` (`version` et `heelonvault-core = "X.Y"`).
2. **Lockfiles** — `git diff Cargo.lock` (dans les deux dépôts) ne doit montrer que les lignes de version de nos crates. Si Cargo a aussi changé d'autres dépendances, restaurer le fichier, modifier ces lignes à la main, puis vérifier avec `cargo check --workspace --locked`.
3. **Documentation** — changelogs FR/EN des deux dépôts, titres des README, `docs/UPDATE_GUIDE*.md`, `crates/heelonvault-core/README.md` (ligne `heelonvault-core = "X.Y"`).
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
8. **Taguer** le commit de merge de core : `git tag vX.Y.Z && git push origin vX.Y.Z`. Le tag déclenche les workflows AppImage, DMG, MSI et SBOM, qui créent la release GitHub.
9. **Vérifier** la release GitHub (artefacts et sommes SHA-256), la page crates.io et la documentation sur [docs.rs](https://docs.rs/heelonvault-core).
