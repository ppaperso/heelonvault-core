# Guide de contribution

Langue : FR | [EN](CONTRIBUTING.md)

Merci de contribuer à HeelonVault.

## Pour commencer

Prérequis, build, tests et contrôles qualité sont décrits dans le
[guide de développement](docs/internal/DEVELOPMENT.fr.md). Le fonctionnement interne est
détaillé dans [ARCHITECTURE.md](docs/ARCHITECTURE.md).

> Construire le workspace nécessite actuellement le dépôt privé `heelonvault-premium`, cloné à
> côté de celui-ci (voir le guide de développement). Les corrections de documentation et les
> changements limités à la crate `heelonvault-core` restent vérifiables avec
> `cargo test -p heelonvault-core` depuis une copie autonome de cette crate.

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
- Préférer des commits petits et ciblés. Ne jamais commiter de secret ni de donnée personnelle.

## Checklist de pull request

- Les contrôles qualité du guide de développement passent (`fmt`, `clippy`, `test`,
  `semver-checks`).
- `sbom.cyclonedx.json` est régénéré si les dépendances ont changé.
- Les changements visibles sont reportés dans le journal des modifications (FR et EN) et dans le
  guide concerné (`docs/QUICKSTART*`, `docs/USER_GUIDE*`, `docs/UPDATE_GUIDE*`).
- Les changements sensibles pour la sécurité sont justifiés dans la description de la PR.

## Documentation

`docs/` est publié sur [doc.heelonvault.heelonys.fr](https://doc.heelonvault.heelonys.fr) ;
`docs/internal/` est réservé aux mainteneurs et n'est pas publié. Garder les deux langues
synchronisées.

## Signalements de sécurité

Ne pas ouvrir d'issue publique pour une vulnérabilité : voir [SECURITY.fr.md](SECURITY.fr.md).
Contact : `security@heelonys.fr`
