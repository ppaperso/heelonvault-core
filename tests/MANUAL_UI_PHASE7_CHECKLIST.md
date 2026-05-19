# Phase 7 - Test Manuel UI (Profil / Securite / 2FA / Admin / Responsive)

## Comment utiliser cette fiche

1. Lance l'application.
2. Execute chaque test dans l'ordre.
3. Coche une seule case par test: PASS ou FAILED.
4. Si FAILED, note la cause dans la colonne Notes.

---

## Informations de campagne

- Date: 19 mai 2026
- Testeur: Patrick
- Build/Commit: 1.1.0
- Environnement (OS): fedora

---

## Legende statut

- [ ] PASS
- [ ] FAILED

---

## Bloc A - Profil utilisateur

| ID | Test | Etapes | Attendu | PASS | FAILED | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| A1 | Ouvrir la vue Profil/Securite | Se connecter, ouvrir la vue profil | Vue chargee sans erreur visuelle | [X] | [ ] | |
| A2 | Modifier nom affiche | Saisir un nouveau nom, cliquer Enregistrer | Message succes + valeur visible apres refresh vue | [X] | [ ] | |
| A3 | Modifier email | Saisir un nouvel email valide, cliquer Enregistrer | Message succes + email persiste apres retour dans la vue | [X] | [ ] | |
| A4 | Changer langue FR/EN | Basculer langue, enregistrer | Textes de la vue mis a jour dans la langue choisie | [X] | [ ] | |

---

## Bloc B - Securite utilisateur

| ID | Test | Etapes | Attendu | PASS | FAILED | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| B1 | Auto-lock 1 minute | Selectionner 1 min | Message succes, valeur conservee en revenant dans la vue | [X] | [] | |
| B2 | Auto-lock 30 minutes | Selectionner 30 min | Message succes, valeur conservee | [X] | [ ] | |
| B3 | Auto-lock Jamais | Selectionner Jamais | Message succes, comportement desarme visible en UI | [X] | [ ] | |
| B4 | Switch affichage mots de passe | Activer puis desactiver le switch | Message succes a chaque changement, preference persistante | [X] | [ ] | |
| B5 | Changement mot de passe - champs vides | Laisser au moins un champ vide, cliquer action | Message erreur explicite | [X] | [ ] | |
| B6 | Changement mot de passe - confirmation differente | Saisir new/confirm differents | Message erreur explicite | [X] | [ ] | |
| B7 | Changement mot de passe - succes | Saisir current/new/confirm corrects | Message succes, champs reinitialises | [X] | [ ] | Rotation master key validee en execution manuelle |
| B8 | Changement mot de passe - succes | Après changement se reloguer avec le nouveau mot de passe | Le logging est un succès | [X] | [ ] | Relogin valide avec le nouveau mot de passe |

---

## Bloc C - 2FA

| ID | Test | Etapes | Attendu | PASS | FAILED | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| C1 | Etat initial badge 2FA | Ouvrir section 2FA | Badge coherent avec l'etat reel (active/desactive) | [ ] | [ ] | |
| C2 | Demarrer activation 2FA | Cliquer Activer 2FA | QR + secret affiches, etat passe en setup | [ ] | [ ] | |
| C3 | Validation code invalide | Saisir code invalide puis confirmer | Message erreur clair, activation refusee | [ ] | [ ] | |
| C4 | Validation code valide | Saisir code valide puis confirmer | Message succes, etat passe a active, badge maj | [ ] | [ ] | |
| C5 | Annuler desactivation 2FA | Cliquer desactiver puis annuler | Etat reste active | [ ] | [ ] | |
| C6 | Confirmer desactivation 2FA | Cliquer desactiver puis confirmer | Message succes, etat passe a desactive, badge maj | [ ] | [ ] | |

---

## Bloc D - Admin avance

| ID | Test | Etapes | Attendu | PASS | FAILED | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| D1 | Visibilite section admin (compte admin) | Se connecter admin, ouvrir Profil | Section Admin avance visible | [ ] | [ ] | |
| D2 | Bouton Ouvrir Utilisateurs | Depuis section Admin avance, cliquer bouton | Navigation vers vue Utilisateurs | [ ] | [ ] | |
| D3 | Bouton Ouvrir Equipes | Depuis section Admin avance, cliquer bouton | Navigation vers vue Equipes | [ ] | [ ] | |
| D4 | Visibilite section admin (compte non-admin) | Se connecter non-admin, ouvrir Profil | Section Admin avance absente | [ ] | [ ] | |
| D5 | Export non-admin bloque | Non-admin, cliquer Export | Message droits admin requis | [ ] | [ ] | |

---

## Bloc E - Responsive / Compact

| ID | Test | Etapes | Attendu | PASS | FAILED | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| E1 | Passage mode compact | Reduire largeur fenetre (< 760px) | Colonnes empilees verticalement, boutons etires | [ ] | [ ] | |
| E2 | Retour mode desktop | Re-agrandir largeur fenetre | Colonnes horizontales, boutons alignes droite | [ ] | [ ] | |
| E3 | Verification visuelle globale | Parcourir sections en compact + desktop | Pas de chevauchement, pas de clipping, pas de rupture visuelle | [ ] | [ ] | |

---

## Bloc F - Journal final

- Nombre de tests PASS:
- Nombre de tests FAILED:
- Decision:
  - [ ] GO
  - [ ] NO GO

### Defauts releves (si FAILED)

- Defaut 1:
- Defaut 2:
- Defaut 3:
