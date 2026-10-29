# Gouvernance

Langue : FR | [EN](GOVERNANCE.md)

HeelonVault est développé par Heelonys selon un modèle piloté par le mainteneur : les décisions
sont prises par le mainteneur, publiquement chaque fois que c'est possible (issues et pull
requests).

## Rôles

| Rôle | Qui | Responsabilités |
| --- | --- | --- |
| Mainteneur | Patrick Paysan ([ppaperso](https://github.com/ppaperso)), Heelonys | Feuille de route, relecture et merge, publications, traitement des vulnérabilités, licences, administration et accès au dépôt |
| Prestataire | Équipes travaillant sous contrat avec Heelonys | Implémenter les issues qui leur sont confiées, selon [CONTRIBUTING.fr.md](CONTRIBUTING.fr.md) |
| Contributeur | Toute personne | Signaler des problèmes, proposer des changements par pull request dans le périmètre de contribution |

Les prestataires suivent exactement le même processus que les autres contributeurs : fork, pull
request, relecture, merge par le mainteneur. Leur contrat peut définir des livrables
supplémentaires, mais pas un autre chemin vers `main`.

## Décisions

- Les décisions courantes se prennent dans l'issue ou la pull request concernée.
- Les décisions suivantes sont consignées dans une issue **avant** l'implémentation et validées
  par le mainteneur : changements d'architecture, nouvelles dépendances, ruptures de l'API
  publique de `heelonvault-core` (version majeure), changements du modèle de sécurité ou de clés,
  licences et frontière entre les éditions communautaire et premium.
- Faute de consensus, le mainteneur tranche et note la raison dans l'issue.

## Accès

- `main` est protégée : pull request obligatoire, CI complète verte, conversations de relecture
  résolues, approbation du mainteneur, merge en squash uniquement, ni force-push ni suppression.
- **L'accès en écriture** à ce dépôt n'est accordé qu'aux personnes également autorisées sur le
  dépôt privé `heelonvault-premium` : une branche de ce dépôt exécute la CI avec un accès à
  celui-ci. Les autres contributeurs, prestataires compris, travaillent depuis un fork.
- Les accès accordés pour une mission sont revus à son terme et retirés s'ils ne sont plus
  nécessaires.

## Publications et sécurité

- Les versions sont préparées et publiées uniquement par le mainteneur
  ([CHANGELOG.md](docs/CHANGELOG.md)).
- Les vulnérabilités sont traitées selon [SECURITY.fr.md](SECURITY.fr.md).

## Code de conduite

Le [code de conduite](CODE_OF_CONDUCT.md) s'applique à toute personne qui participe. Le mainteneur
veille à son application (`contact@heelonys.fr`).

## Modification de ce document

Ce document est modifié par pull request, approuvée par le mainteneur.
