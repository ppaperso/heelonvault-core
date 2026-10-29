# Guide de contribution

Langue : FR | [EN](CONTRIBUTING.md)

Merci de contribuer à HeelonVault.

## Périmètre des contributions externes

HeelonVault suit un modèle Open Core : ce dépôt est public, tandis que le dépôt
`heelonvault-premium` est privé et compilé dans l'application. Le workspace ne se charge pas sans
une copie de ce dépôt privé : **les contributions externes sont donc limitées à** :

- la crate `heelonvault-core` (`crates/heelonvault-core/`) : modèles, repositories, services,
  traductions et leurs tests ;
- la documentation (`docs/` et les fichiers Markdown à la racine du dépôt).

L'application GTK (`crates/heelonvault-app/`), les autres crates, les workflows de CI, le
packaging et la publication sont modifiés par les mainteneurs. Pour proposer un changement dans
ces parties, ouvrir une issue qui décrit le besoin : un mainteneur l'implémente ou convient avec
vous d'une autre approche.

Cette règle s'applique à tout contributeur sans accès à `heelonvault-premium`, prestataires
compris. L'accès en écriture à ce dépôt n'est accordé qu'aux personnes également autorisées sur
`heelonvault-premium` (voir [GOVERNANCE.fr.md](GOVERNANCE.fr.md)).

## Pour commencer

Le build et le fonctionnement interne sont décrits dans le
[guide de développement](docs/internal/DEVELOPMENT.fr.md) et dans
[ARCHITECTURE.md](docs/ARCHITECTURE.md).

Sans `heelonvault-premium`, vérifier `heelonvault-core` avec :

```bash
scripts/check-core-standalone.sh            # fmt, clippy, clippy-secure, tests
scripts/check-core-standalone.sh --semver   # + cargo semver-checks (nécessite cargo-semver-checks)
```

Le script copie la crate dans un workspace séparé (par défaut
`~/.cache/heelonvault/core-standalone`) et y exécute les mêmes contrôles qualité que la CI. Il ne
modifie jamais votre copie de travail.

## Workflow

1. **D'abord une issue** : tout changement non trivial part d'une issue, dans laquelle le besoin
   et l'approche sont validés avant de coder. Les corrections de coquilles ou de formulation dans
   la documentation peuvent s'en passer.
2. **Forker** le dépôt sur GitHub et cloner son fork.
3. **Branche thématique** créée depuis un `main` à jour, un seul sujet par branche, nommée
   `<type>/<sujet-court>`, par exemple `fix/csv-import-bom` ou `docs/update-guide-proxy`.
   Types : `feat`, `fix`, `docs`, `refactor`, `test`, `chore`.
4. **Commiter** des changements petits et ciblés. Rebaser sur `main` plutôt que fusionner `main`
   dans sa branche.
5. **Ouvrir une pull request** de sa branche vers `main` et remplir le modèle. L'ouvrir en
   brouillon tant qu'elle n'est pas prête pour la relecture.

Personne ne pousse directement sur `main` : la branche est protégée et n'accepte que des pull
requests.

### Langue

Les messages de commit, pull requests, relectures et commentaires de code sont rédigés en
**anglais**. Les issues peuvent être ouvertes en anglais ou en français. La documentation destinée
aux utilisateurs reste bilingue (FR et EN).

## Règles de code

- Suivre le style, le nommage et le découpage en modules existants.
- Pas de `unwrap()` / `expect()` (interdits par `clippy.toml`), pas d'`unsafe`, aucun
  avertissement : la CI compile avec `-D warnings`. Utiliser des erreurs typées (`thiserror`).
- Pas de `#[allow(...)]` pour faire taire un lint sans justification écrite.
- Aucune valeur secrète ne doit atteindre l'interface, un journal ou un message d'erreur.
- Ajouter des tests pour tout changement de comportement des repositories et services
  (`crates/heelonvault-core/tests/`).
- L'API publique de `heelonvault-core` suit semver : préférer les méthodes de trait par défaut,
  `#[non_exhaustive]` et la dépréciation aux ruptures ([RELEASING.md](docs/internal/RELEASING.md)).
- Pas de nouvelle dépendance sans en avoir discuté dans l'issue. Ne jamais commiter de secret ni
  de donnée personnelle.

## Pull requests

Le [modèle de pull request](.github/pull_request_template.md) contient la checklist. En résumé :

- la PR référence son issue (`Closes #123`) et ne traite qu'un sujet ;
- les contrôles qualité passent, et la description liste les commandes de validation exécutées ;
- `sbom.cyclonedx.json` est régénéré si les dépendances ont changé ;
- les changements visibles sont reportés dans le journal des modifications (FR et EN) et dans le
  guide concerné (`docs/QUICKSTART*`, `docs/USER_GUIDE*`, `docs/UPDATE_GUIDE*`) ;
- les changements sensibles pour la sécurité sont justifiés dans la description de la PR.

Le titre de la PR devient le message du commit sur `main` (squash merge) : l'écrire sous la forme
`<type>(<portée>): <résumé>`, par exemple `fix(import): accept a UTF-8 BOM in CSV headers`.

### Intégration continue depuis un fork

GitHub ne donne pas aux pull requests issues d'un fork l'accès au dépôt privé. Sur une telle PR :

- le job **`heelonvault-core (standalone)`** exécute `scripts/check-core-standalone.sh --semver` :
  il doit être vert ;
- le check complet `check, clippy, fmt, test` échoue à l'étape `Resolve Premium revision`. C'est
  attendu : avant le merge, un mainteneur exécute la CI complète sur une copie de votre branche
  (voir ci-dessous).

## Relecture et merge

- **Qui relit et merge** : uniquement le mainteneur désigné dans
  [GOVERNANCE.fr.md](GOVERNANCE.fr.md). Les contributeurs ne mergent pas, y compris leurs propres
  pull requests.
- **Conditions de merge**, toutes obligatoires :
  1. la CI complète (`check, clippy, fmt, test`) est verte sur le commit mergé ;
  2. toutes les conversations de relecture sont résolues ;
  3. le mainteneur a explicitement approuvé la pull request.
- **Méthode de merge** : squash uniquement. La branche est supprimée après le merge.
- Tout nouveau push annule une approbation précédente : le mainteneur relit les derniers
  changements.
- **Pull requests issues d'un fork** : une fois la relecture terminée, le mainteneur pousse votre
  branche sur une branche interne de ce dépôt, où la CI complète s'exécute, puis la merge en squash
  en vous créditant comme auteur (`Co-authored-by`). Votre pull request d'origine est fermée avec
  un lien vers le commit mergé.
- **Règle du mainteneur pour cette copie** : la CI interne récupère `heelonvault-premium` et
  exécute le code proposé (tests, `build.rs`, nouvelles dépendances, fichiers de workflow) à côté.
  La branche n'est poussée en interne **qu'après** relecture de tout son diff, en particulier
  `.github/`, `build.rs`, `Cargo.toml` / `Cargo.lock` et tout code qui accède au réseau ou au
  système de fichiers ; jamais pour simplement voir si la CI passe. Une pull request qui modifie
  `.github/` n'est pas copiée : ce changement est fait par un mainteneur.
- Comptez une première réponse sous cinq jours ouvrés. Une pull request sans activité de son
  auteur pendant 90 jours peut être fermée ; elle peut être rouverte à tout moment.

## Documentation

`docs/` est publié sur [doc.heelonvault.heelonys.fr](https://doc.heelonvault.heelonys.fr) ;
`docs/internal/` est réservé aux mainteneurs et n'est pas publié. Garder les deux langues
synchronisées.

## Code de conduite

La participation à ce projet est régie par le [code de conduite](CODE_OF_CONDUCT.md).

## Signalements de sécurité

Ne pas ouvrir d'issue publique pour une vulnérabilité : voir [SECURITY.fr.md](SECURITY.fr.md).
Contact : `security@heelonys.fr`
