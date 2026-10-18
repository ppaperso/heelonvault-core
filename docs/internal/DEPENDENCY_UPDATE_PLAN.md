# Plan de mise a jour des dependances

## Objectif et perimetre

Actualiser les graphes Cargo de Core/App et Premium, corriger le controle
supply-chain et conserver la compatibilite des coffres, sauvegardes et phrases de
recuperation. Les deux depots restent des unites de livraison distinctes :
chacun conserve son propre `Cargo.lock`. Aucune base utilisateur n'est modifiee.

## Ordre d'execution et points de controle

1. **Securite, sans migration fonctionnelle**
   - Dans chaque depot, executer `cargo update -p rustls -p quinn-proto`.
   - Verifier avec `cargo audit --deny warnings` et
     `cargo deny --locked --all-features check advisories`.
   - Corriger les versions, jamais ignorer un avis RustSec.
2. **Mises a jour compatibles**
   - Executer `cargo update` dans chaque depot, sans changer BIP39.
   - Examiner ajouts, suppressions, doublons, versions retirees et MSRV.
   - Compiler et executer les tests de sauvegarde avant la migration majeure.
   - Un recul choisi par Cargo pour satisfaire une contrainte exacte upstream
     doit etre explique, pas remplace par un forcage arbitraire.
3. **Outillage et CI**
   - Actualiser les outils et actions utilises, en lisant leurs changements
     majeurs et en conservant le compilateur Rust epingle.
   - Auditer les deux lockfiles ; verifier aussi Premium avec
     `--all-features` depuis son propre repertoire.
   - Faire echouer les controles sur les avertissements apres suppression des
     regles devenues inutiles ; ne pas ajouter d'exceptions permanentes.
   - Garder une resolution `--locked` et activer le workflow quand sa propre
     configuration change.
4. **BIP39 en dernier**
   - Lire les changements upstream et etablir des vecteurs de reference avant
     de changer la contrainte majeure.
   - Tester les correspondances entropie/phrase, le checksum, les entrees
     invalides, le dechiffrement d'une sauvegarde historique et son verificateur.
   - Ne pas changer le format des phrases, leur normalisation ou la derivation
     des cles ; verifier que l'API publique de Core reste compatible.
5. **Validation finale et SBOM**
   - Regenerer `sbom.cyclonedx.json` avec `./scripts/generate-sbom.sh`.
   - Verifier que deux generations ont le meme contenu hors identifiant et
     horodatage.
   - Executer les controles ci-dessous dans Core et dans Premium, ainsi que
     les tests reels (pas uniquement leur compilation).
   - La validation locale Linux ne remplace pas les resultats CI Linux,
     Fedora, macOS et Windows. Ne pas annoncer ces plateformes validees sans
     executions correspondantes.

## Commandes de validation

Depuis la racine de chaque depot :

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo check --locked
cargo test --locked --all-targets --no-run
cargo test --locked --all-targets --all-features
cargo audit --deny warnings
cargo deny --locked --all-features check advisories --deny warnings
```

Depuis Core, verifier aussi l'ensemble des politiques :

```bash
cargo deny --locked --all-features check --deny warnings
cargo semver-checks -p heelonvault-core
```

## Livraison et retour arriere

Livrer les modifications des deux depots de facon coordonnee, avec les
lockfiles et le SBOM correspondant exactement aux sources. Aucun commit,
push ou declenchement de release n'est implicite dans ce plan.

En cas de regression, isoler la phase responsable et corriger avant de
continuer. Un retour arriere de dependances doit etre explicite, ne jamais
retablir une version vulnerable et ne jamais supprimer le travail d'autrui.

## Resultats

Toutes les phases locales ont ete executees dans l'ordre indique :

| Controle | Core/App | Premium autonome |
| --- | --- | --- |
| Correctifs RustSec | Rustls 0.23.46, Quinn-proto 0.11.19 | Identiques |
| `cargo fmt --all -- --check` | OK | OK |
| Clippy, toutes cibles et fonctionnalites, avertissements interdits | OK | OK |
| `cargo check --locked` | OK | OK |
| Compilation des tests, toutes cibles | OK | OK |
| Tests reels, toutes cibles et fonctionnalites | 322 reussis, aucun ignore | 18 reussis, aucun ignore |
| Audit des lockfiles, avertissements interdits | Aucun avis ni avertissement | Aucun avis ni avertissement |
| Cargo-deny, toutes fonctionnalites, avertissements interdits | Toutes politiques OK | Advisories OK |

- Les six tests de sauvegarde existants et quatre tests de compatibilite sont
  passes avant et apres migration BIP39. Les donnees publiques figees avec
  BIP39 2 couvrent une sauvegarde HVB v2 et un verificateur de phrase, sans
  conserver de donnees utilisateur.
- BIP39 3 conserve les API utilisees : aucune adaptation du code applicatif,
  de la derivation des cles ou des formats persistants n'a ete necessaire.
- `cargo semver-checks` : 223 controles reussis, aucune rupture de l'API
  publique par rapport a la version publiee. Les doctests passent avec les
  deux exemples deja ignores dans le depot.
- Le SBOM du binaire comporte 408 composants ; deux generations sont
  identiques apres retrait de l'identifiant et de l'horodatage.
- `cargo update --dry-run --verbose` ne propose plus de modification dans
  les deux depots. Les versions transitives inferieures aux dernieres stables
  restent imposees par upstream : `crypto-common` 0.1.6 permet
  `generic-array` 0.14.9, alors que 0.1.7 impose exactement 0.14.7 ;
  `axum` impose exactement `matchit` 0.8.4. Les anciennes series de SHA-2 et
  autres doublons existants restent necessaires aux contraintes upstream.
- L'outillage CI utilise cargo-deny 0.20.2, cargo-audit 0.22.2 et
  cargo-semver-checks 0.51.0 ; cargo-cyclonedx 0.5.9 est deja a jour. Les
  actions Checkout, Cache, Upload Artifact et Attest Build Provenance sont
  actualisees. Les runtimes Node 24 et les entrees utilisees ont ete verifies.
  Le controle supply-chain natif evite le build Docker et l'installation
  d'un second compilateur dans le conteneur.
- Actionlint valide integralement CI et supply-chain, ainsi que la structure
  de tous les workflows. L'analyse avec ShellCheck sur tous les workflows
  retrouve huit remarques preexistantes dans les scripts de packaging
  Linux/macOS et de release SBOM (SC2129, SC2012, SC2010). Leur presence
  a ete confirmee sur les fichiers de `HEAD` avant modification ; aucune
  regle de suppression n'a ete ajoutee et ces scripts hors perimetre ne
  sont pas reecrits.

### Limites et integration inter-depots

La preparation des commits et PR ne publie pas de crate, de tag ou de paquet.
Les plateformes Fedora, macOS et Windows restent a valider dans la CI ;
le succes local Linux ne vaut pas certification multiplateforme.

Les checks utilisent le Core local pour Premium. Les deux revisions et leurs
lockfiles doivent donc etre testes ensemble avant fusion : un checkout qui
melange le Core anterieur a la migration BIP39 et le nouveau lockfile Premium
(ou l'inverse) ne satisfait pas `--locked`. Pour des PR inter-depots, utiliser
les revisions correspondantes dans les checkouts de validation, puis revenir
aux branches de livraison coherentes. Ne pas contourner ce controle par une
regeneration silencieuse du lockfile en CI.

La maintenance prepare une livraison corrective 2.0.2 : les versions Core,
App et Premium sont alignees, ainsi que les deux lockfiles, les changelogs,
les README, le SBOM et l'inventaire des licences. Le shim SQLx garde sa
version upstream propre ; la contrainte Premium `heelonvault-core = "2.0"`
reste compatible. Cette preparation ne publie ni crate ni paquet.

## Mise a jour du compilateur Rust

Apres validation du graphe de dependances, la toolchain de developpement et
de livraison des deux depots passe de 1.98.0 a 1.99.1. Les manifestes gardent
`rust-version = "1.98"` : mettre a jour le compilateur ne releve pas la MSRV.
Le compilateur global par defaut et les versions deja installees sont conserves.

Ordre applique :

1. Installer `1.99.1` avec les composants `rustfmt` et `clippy`, sans changer
   les fichiers de pin ni executer `cargo update`.
2. Valider avec `cargo +1.99.1` dans chaque depot : formatage, compilation
   verrouillee, Clippy strict, compilation puis execution des tests avec toutes
   les fonctionnalites. Verifier aussi l'API publique depuis Core.
3. Confirmer la MSRV avec `cargo +1.98.0 check --locked --all-targets
   --all-features` dans les deux depots (ajouter `--workspace` dans Core).
4. Apres succes, epingler `1.99.1` dans les deux `rust-toolchain.toml` et
   aligner la documentation. Ajouter les deux checks MSRV a la CI.
5. Verifier la selection effective de Rust/Cargo sans `+version`, les audits,
   la compatibilite de l'API publique et la coherence du SBOM.

Les controles avant bascule reussissent sous Rust 1.99.1 sans modification
du code applicatif, ajout d'exception Clippy ou reformatage des sources.
Les deux checks MSRV sous 1.98.0 reussissent egalement. La validation distante
des paquets Linux/Fedora/macOS/Windows reste necessaire avant livraison.

Apres bascule, les deux repertoires selectionnent effectivement
`rustc 1.99.1` et `cargo 1.99.1` sans argument `+version`. Bilan :

- Core/App : 322 tests reussis ; Premium : 18 tests reussis.
- Formatage, Clippy strict, compilation et compilation des tests reussis
  dans les deux depots sous le nouveau compilateur.
- Audits sans avis ni avertissement ; politiques cargo-deny Core toutes
  categories et Premium advisories validees.
- API publique : 223 controles semver reussis ; doctests sans echec
  (deux exemples Core deja ignores).
- Actionlint valide CI et supply-chain, y compris les nouveaux checks MSRV.
- Les empreintes SHA-256 des deux lockfiles sont identiques avant et apres
  cette migration. Le SBOM regenere conserve exactement le meme contenu
  normalise et les memes 408 composants.

  ## Verification de la preparation 2.0.2

  Apres alignement des versions produit et regeneration des inventaires :

  - Les controles de compilation, formatage, Clippy, tests, audit et MSRV sont
    reexecutes sur les sources 2.0.2 des deux depots.
  - `cargo publish --dry-run --allow-dirty -p heelonvault-core --locked`
    construit la crate hors du workspace sans publier.
  - Les PR sont creees sur des branches issues de `main` a jour, afin de ne
    pas reinclure les anciens commits de l'installation Linux deja fusionnes.
  - Ordre de fusion : Premium d'abord, puis relance des checks Core, puis
    Core. La CI Core recupere `main` de Premium ; avant cette premiere fusion,
    les controles `--locked` et de version commune peuvent echouer car ils
    voient encore Premium 2.0.1. Ce blocage attendu ne doit pas etre ignore.
  - Aucun tag de release, publication crates.io ou fusion de PR n'est
    effectue dans cette preparation.
