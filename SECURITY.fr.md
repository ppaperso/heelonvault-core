# Guide de securite (runtime Rust)

Langue : FR | [EN](SECURITY.md)

Derniere mise a jour : 27 mars 2026
Perimetre : runtime actif dans `src/`

Ce document reflete le code Rust actuel.

## 1. Perimetre et modele de menace

HeelonVault est un gestionnaire de secrets desktop local-first.

Objectifs securite :

- proteger les secrets au repos ;
- proteger les materiaux d'authentification ;
- limiter les tentatives de brute force ;
- reduire les fuites accidentelles en UI et logs ;
- garantir la securite de session (auto-lock et deconnexion explicite).

Hypothese principale : si le compte OS est compromis pendant que l'app est deverrouillee, l'impact attaquant reste eleve.

## 2. Cryptographie (Rust)

Primitives actuelles (`src/services/crypto_service.rs`) :

- KDF : Argon2id (v=19)
- Parametres par defaut : memoire 64 MiB, time cost 3, parallelisme 1
- Cle derivee : 32 octets
- Salt : 32 octets
- Chiffrement : AES-256-GCM
- Nonce : 12 octets aleatoires par chiffrement
- RNG : `getrandom` (CSPRNG noyau)
- Buffers sensibles : `secrecy` + `zeroize`

## 3. Authentification

Modele courant (`src/services/auth_service.rs`) :

- mot de passe clair present uniquement en memoire volatile ;
- verification avec comparaison en temps constant ;
- stockage via enveloppe versionnee (`users.password_envelope`) ;
- aucune persistance du mot de passe en clair.

## 4. Identifiant de login

Le champ unique de login accepte :

1. username (priorite 1)
2. email (priorite 2)
3. display_name (priorite 3)

La politique d'echec est appliquee sur le username canonique resolu.

## 5. Brute force et controles de session

Controles actuels (`src/services/auth_policy_service.rs`) :

- seuil : 5 echecs
- fenetre de blocage : 5 minutes
- compteurs persistants en table `auth_policy`
- login reussi : reset des compteurs

Session :

- auto-lock par utilisateur : 0, 1, 5, 10, 15, 30 minutes
- retour ecran login sur logout/auto-lock
- verification TOTP obligatoire si 2FA activee

## 6. Politique de mot de passe

Regles actuelles (`password_service`) :

- longueur min 16, max 128
- minuscule + majuscule + chiffre + symbole
- pas d'espace
- generation par defaut : 24 caracteres

Recommandation operationnelle : utiliser des phrases de passe >= 16 caracteres pour les comptes sensibles.

## 7. Frontieres de protection des donnees

Chiffre en AES-256-GCM :

- payloads secrets
- enveloppes de cles de coffre
- enveloppes de mot de passe

Peut rester en clair pour l'UX/recherche :

- certains metadonnees (titre, tags, URL, etc.)

Recommendation : ne pas placer de secrets sensibles dans ces champs indexables.

## 8. Etat 2FA

2FA/TOTP est activee de bout en bout :

- activation/desactivation depuis le profil ;
- login en deux etapes (mot de passe puis code TOTP) ;
- secret TOTP stocke chiffre en base.

## 8.1 Bootstrap initial et cle de recuperation

Lorsque l'application demarre sans compte administrateur, un assistant en 3 etapes est presente :

1. **Etape identite** — saisie du nom d'utilisateur et du mot de passe (force minimale verifiee, confirmation requise) ;
2. **Etape serment (oath)** — une phrase mnemotechnique de 24 mots (style BIP39) est generee via `BackupService::generate_recovery_key()`. L'utilisateur doit verifier deux mots tires au sort avant de confirmer, prouvant qu'il a bien note la phrase ;
3. **Etape pending** — `AdminService::bootstrap_first_admin()` est appele en arriere-plan ; la session s'ouvre automatiquement apres succes.

Proprietes de securite de la cle de recuperation :

- generee par un RNG cryptographique (`getrandom`) ;
- la phrase n'est jamais persistee en base de donnees ; l'utilisateur en est l'unique gardien ;
- la copie presse-papier declenche un effacement automatique apres 60 secondes ; le presse-papier est aussi vide a la fermeture du dialogue ;
- apres le bootstrap, la cle peut etre re-exportee depuis `Profil & Securite` (admin uniquement) ;
- les exports/imports `.hvb` sont soumis au `BackupApplicationService` qui applique un controle RBAC : seuls les comptes de role admin peuvent effectuer ces operations.

## 9. Journalisation

Couverture actuelle :

- historique des connexions reussies (`login_history`) ;
- traces de seuil d'echec et reset de politique.

Regle : ne jamais journaliser de secrets en clair.

## 10. Tests securite minimaux

Avant release :

1. `cargo check`
2. `cargo test`
3. Suites ciblees :
   - `tests/security_auth.rs`
   - `tests/security_crypto.rs`
   - `tests/totp_activation_integration.rs`
   - `tests/twofa_messages_integration.rs`
   - `tests/backup_security_integration.rs`

## 11. Divulgation responsable

Ne pas ouvrir d'issue publique pour les vulnerabilites.

Canal principal :

- `security@heelonys.fr`

Objet recommande :

- `SECURITY-HeelonVault : titre court`

Merci d'inclure : version impactee, environnement, etapes de reproduction, impact, PoC minimal.

## 12. Priorisation (mini matrice CVSS)

- P1 Critique : traitement immediat, objectif <= 7 jours
- P2 Haut : objectif <= 14 jours
- P3 Moyen : objectif <= 30 jours
- P4 Bas : meilleur effort dans cycle normal

## 13. Roadmap de durcissement

Realise :

- [x] Cycle de vie MFA durci : activation/desactivation TOTP entierement implementee
- [x] Flux de recuperation securise : generation de cle avec effacement presse-papier automatique, verification obligatoire, operations de sauvegarde soumises au RBAC
- [x] Journal d'audit pour les actions sensibles (table `audit_log`, migration 0013)

Priorites court terme :

- unifier toutes les entrees de politique mot de passe sur >= 16 ;
- documenter les profils de durcissement (standard, admin, haute assurance).

References : ANSSI, OWASP Password Storage Cheat Sheet, NIST SP 800-63B.
