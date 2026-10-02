# Documentation interne / Internal documentation

Ce dossier regroupe la documentation destinée aux **mainteneurs et contributeurs** :
développement, livraison, packaging, CI, spécifications. Il n'est **pas publié** sur
[doc.heelonvault.heelonys.fr](https://doc.heelonvault.heelonys.fr) : le script de
synchronisation du site (`Heelonys_webdoc`, `docs-sync.config.json`) exclut `docs/internal/`.

This folder holds documentation for **maintainers and contributors** (development, release,
packaging, CI, specifications). It is **not published** on the documentation site: the site's
sync script excludes `docs/internal/`.

## Règle de rangement / Where does a document go?

| Le document s'adresse à… | Emplacement | Publié sur le site |
| ------------------------ | ----------- | ------------------ |
| Utilisateurs, administrateurs, évaluateurs sécurité | `docs/` (ou racine : `README`, `SECURITY`, `CONTRIBUTING`, `CODE_OF_CONDUCT`) | Oui |
| Mainteneurs, contributeurs, CI | `docs/internal/` | Non |

Un lien depuis une page publiée vers un fichier de ce dossier reste valide : le site le
réécrit en lien GitHub.

## Contenu / Contents

| Sujet | FR | EN |
| ----- | -- | -- |
| Environnement de développement, build, tests | [DEVELOPMENT.fr.md](DEVELOPMENT.fr.md) | [DEVELOPMENT.md](DEVELOPMENT.md) |
| Recette manuelle de l'interface avant livraison | [MANUAL_QA.md](MANUAL_QA.md) | — |
| Livrer une version (semver, checklist) | [RELEASING.md](RELEASING.md) | [RELEASING.en.md](RELEASING.en.md) |
| Packaging Windows (MSI, diagnostic) | [RUNBOOK_WINDOWS_PACKAGING.md](RUNBOOK_WINDOWS_PACKAGING.md) | — |
| Rotation durcie de la clé maître (spécification) | — | [MASTER_KEY_ROTATION_HARDENING_SPEC.md](MASTER_KEY_ROTATION_HARDENING_SPEC.md) |
| Plan d'optimisation CI (mai 2026, non appliqué tel quel) | — | [CI_OPTIMIZATION_PLAN.md](CI_OPTIMIZATION_PLAN.md) |
| Liens externes du projet | — | [EXTERNAL_LINKS.md](EXTERNAL_LINKS.md) |
| Charte graphique | [charte/CHARTE_GRAPHIQUE.md](charte/CHARTE_GRAPHIQUE.md) | [charte/CHARTE_GRAPHIQUE.en.md](charte/CHARTE_GRAPHIQUE.en.md) |
