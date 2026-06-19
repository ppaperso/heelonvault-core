# Guide utilisateur

Langue : FR | [EN](USER_GUIDE.en.md)

Version cible documentée : `1.1.0`

## Objectif

Ce manuel utilisateur présente l'utilisation de HeelonVault dans un contexte opérationnel quotidien. Il s'adresse aux utilisateurs finaux qui doivent protéger, retrouver et maintenir leurs secrets dans l'application sans dépendre de la documentation technique du projet.

Le document suit les principaux écrans du produit et décrit, pour chacun d'eux, l'objectif de la vue, les actions disponibles et les bonnes pratiques associées.

Ce guide utilisateur décrit l'utilisation courante de HeelonVault côté poste de travail :

- premier lancement ;
- connexion et sécurité de session ;
- création, modification et recherche de secrets ;
- import, export et corbeille ;
- bonnes pratiques de sécurité.

## Table des matières

1. Vue générale du parcours utilisateur
2. Écran 1 - Assistant d'initialisation
3. Écran 2 - Connexion
4. Écran 3 - Vue principale du coffre
5. Écran 4 - Création d'un secret
6. Écran 5 - Modification, suppression et corbeille
7. Écran 6 - Recherche et organisation
8. Écran 7 - Profil et sécurité
9. Écran 8 - Import et export
10. Écran 9 - Tableau de bord et audit
11. Bonnes pratiques
12. Dépannage rapide
13. Références utiles

## Emplacements pour captures d'écran

Les captures d'écran pourront être ajoutées plus tard dans ce document aux emplacements prévus. La numérotation ci-dessous permet d'aligner facilement les futures captures avec les sections du manuel.

Exemple de convention conseillée :

- `docs/images/user-guide/login-fr.png`
- `docs/images/user-guide/dashboard-fr.png`
- `docs/images/user-guide/editor-fr.png`

## 1. Vue générale du parcours utilisateur

Le parcours standard d'un utilisateur HeelonVault suit la séquence suivante :

1. initialiser ou ouvrir un coffre ;
2. s'authentifier ;
3. consulter ou rechercher un secret ;
4. créer, modifier, partager ou supprimer un élément ;
5. gérer la sécurité de session et les opérations avancées.

Dans une documentation produit, ce chemin est important car il reflète les écrans réellement manipulés par l'utilisateur final. Les sections suivantes sont donc organisées par écrans fonctionnels.

## 2. Écran 1 - Assistant d'initialisation

Au premier démarrage, HeelonVault affiche un assistant d'initialisation guidé pour créer le premier compte administrateur.

Rôle de l'écran :

- préparer le coffre pour sa première utilisation ;
- créer le premier compte disposant des droits d'administration ;
- enregistrer les éléments de récupération indispensables.

Étapes générales :

1. Choisir un identifiant administrateur.
2. Définir un mot de passe maître fort.
3. Enregistrer la clé de récupération générée.
4. Finaliser l'initialisation pour ouvrir le coffre.

À retenir :

- la clé de récupération doit être conservée dans un emplacement sûr et séparé de la machine ;
- le mot de passe maître conditionne directement la sécurité d'accès au coffre ;
- cette étape ne doit pas être interrompue sans sauvegarder les informations affichées.

Emplacement capture d'écran : Écran 1 - assistant d'initialisation

![Écran 1a - Initialisation étape 1](../assets/images/user-guide/hv_first_init_1.png)

Capture 01a - Assistant d'initialisation, étape 1 (création du compte administrateur).

![Écran 1b - Initialisation étape 2](../assets/images/user-guide/hv_first_init_2.png)

Capture 01b - Assistant d'initialisation, étape 2 (clé de secours 24 mots).

## 3. Écran 2 - Connexion

Après initialisation, l'écran de connexion permet de saisir les identifiants du compte et, si activé, le code TOTP à usage unique.

Rôle de l'écran :

- authentifier l'utilisateur ;
- contrôler l'accès au coffre ;
- appliquer les règles de sécurité configurées pour le compte.

Bonnes pratiques :

- utiliser un mot de passe unique et long ;
- conserver la clé de récupération hors poste ;
- vérifier l'heure système si le TOTP est refusé.

Emplacement capture d'écran : Écran 2 - connexion

![Écran 2 - Connexion](../assets/images/user-guide/hv_login_screen_after_init.png)

Capture 02 - Écran de connexion avec identifiant, mot de passe et accès récupération de base (.hvb).

## 4. Écran 3 - Vue principale du coffre

Une fois connecté, l'utilisateur accède à la vue principale du coffre avec :

- la liste des secrets ;
- les fonctions de recherche et filtrage ;
- les actions de création, modification, suppression et partage ;
- l'accès au profil et à la sécurité.

Rôle de l'écran :

- servir de point d'entrée pour toutes les opérations courantes ;
- centraliser la navigation dans les données du coffre ;
- offrir un accès rapide aux actions prioritaires.

Emplacement capture d'écran : Écran 3 - vue principale

![Écran 3 - Vue principale du coffre](../assets/images/user-guide/hv_dashboard_empty.png)

Capture 03 - Vue principale du coffre avec recherche, catégories, audit de sécurité et zone centrale.

## 5. Écran 4 - Création d'un secret

Pour ajouter un secret :

1. Ouvrir l'action de création.
2. Choisir le type ou la catégorie adaptée.
3. Renseigner les champs utiles : titre, login, mot de passe, URL, notes, tags.
4. Cocher « Accès données de santé » si le secret est lié à des données médicales.
5. Vérifier l'indicateur de robustesse.
6. Enregistrer.

Recommandations :

- utiliser des titres explicites ;
- renseigner les tags pour faciliter la recherche ;
- utiliser le marqueur « Accès données de santé » uniquement pour les secrets réellement sensibles au sens santé ;
- éviter les notes contenant des informations non nécessaires.

Dans une logique produit, cet écran est central : il doit permettre une saisie rapide sans compromettre la qualité ni la sécurité des données enregistrées.

Emplacement capture d'écran : Écran 4 - éditeur de secret

![Écran 4a - Sélection du type de secret](../assets/images/user-guide/hv_add_menu.png)

Capture 04a - Choix du type de secret (password, api_token, ssh_key, secure_document).

![Écran 4b - Formulaire mot de passe](../assets/images/user-guide/hv_add_password1.png)

Capture 04b - Création d'un secret de type mot de passe (vue générale du formulaire).

![Écran 4c - Formulaire mot de passe, zone de validité](../assets/images/user-guide/hv_add_password2.png)

Capture 04c - Paramètres complémentaires d'un secret mot de passe (notes, validité, enregistrement).

![Écran 4d - Formulaire token API](../assets/images/user-guide/hv_add_apikey.png)

Capture 04d - Création d'un secret de type token API.

![Écran 4e - Formulaire clé SSH](../assets/images/user-guide/hv_add_sshkey.png)

Capture 04e - Création d'un secret de type clé SSH.

![Écran 4f - Formulaire document sécurisé](../assets/images/user-guide/hv_add_securedoc.png)

Capture 04f - Création d'un secret de type document sécurisé.

## 6. Écran 5 - Modification, suppression et corbeille

Chaque secret peut être modifié depuis l'éditeur intégré. La suppression passe par la corbeille afin d'éviter une perte immédiate.

Dans le tableau principal :

- un clic simple sur une carte la sélectionne sans ouvrir l'éditeur ;
- un double-clic ouvre la modification du secret sélectionné.

Rôle de l'écran :

- permettre la maintenance du contenu du coffre ;
- sécuriser la suppression grâce à une étape intermédiaire ;
- offrir une restauration rapide en cas d'erreur.

Flux recommandé :

1. Modifier le secret si nécessaire.
2. Utiliser la suppression logique pour l'envoyer en corbeille.
3. Restaurer le secret en cas d'erreur.
4. Purger définitivement seulement après validation.

Emplacement capture d'écran : Écran 5 - corbeille

![Écran 5 - Corbeille et restauration](../assets/images/user-guide/hv_trash.png)

Capture 05 - Corbeille avec actions de restauration et purge des éléments supprimés.

## 7. Écran 6 - Recherche et organisation

HeelonVault prend en charge une recherche multi-champs en temps réel sur l'ensemble des données des secrets.

### Mode de recherche

- **Coffre actif (défaut)** : la recherche porte sur le coffre sélectionné dans le panneau latéral.
- **MultiCoffre** : activez le bouton **MultiCoffre** à gauche de la barre de recherche pour étendre la recherche à tous les coffres accessibles simultanément.

### Recherche sans préfixe

Un terme tapé seul est recherché dans tous les champs : titre, type, login, email, URL, notes, catégorie, tags et nom du coffre. La correspondance floue tolère une faute de frappe.

### Raccourci thématique `#sante`

Le raccourci `#sante` affiche les secrets marqués « Accès données de santé » et ceux détectés automatiquement avec une confiance élevée.

### Syntaxe `champ:valeur`

Pour cibler un champ précis, utilisez la syntaxe `champ:valeur` (avec ou sans espace après le deux-points) :

| Clés acceptées | Champ recherché |
| --- | --- |
| `title`, `titre`, `name`, `nom` | Titre |
| `login`, `user`, `username`, `identifiant` | Login |
| `email`, `mail` | Email |
| `url`, `site`, `domaine`, `domain` | URL |
| `notes`, `note` | Notes |
| `category`, `categorie`, `cat` | Catégorie |
| `tag`, `tags` | Tags |
| `type`, `kind` | Type de secret |
| `vault`, `coffre`, `vault-name` | Nom du coffre |

Exemples : `login:alice` · `coffre:perso` · `titre:gmail` · `url:google`

Le bouton `?` à droite de la barre affiche ce récapitulatif directement dans l'application.

### Raccourcis clavier sur la carte active

Quand une carte est sélectionnée, les actions rapides suivantes sont disponibles :

- `Ctrl+C` : copier le mot de passe ;
- `Ctrl+L` : copier le login (si présent) ;
- `Ctrl+U` : ouvrir l'URL (si présente).

### Bonnes pratiques

Renforcer la pertinence de la recherche avec une organisation cohérente :

- adopter une convention de nommage stable ;
- utiliser les tags de manière cohérente ;
- regrouper les secrets par type, usage ou équipe selon le contexte.

Emplacement capture d'écran : Écran 6 - recherche

![Écran 6 - Recherche et navigation](../assets/images/user-guide/hv_dashboard_empty.png)

Capture 06 - Barre de recherche avec toggle MultiCoffre et bouton d'aide, navigation latérale.

## 8. Écran 7 - Profil et sécurité

Depuis le profil, l'utilisateur peut consulter les réglages liés à la sécurité et à la session, notamment :

- l'activation TOTP ;
- la politique d'auto-verrouillage ;
- le changement de mot de passe maître avec rotation des enveloppes de clés de coffre ;
- l'activation du code PIN de déverrouillage rapide ;
- certaines préférences d'affichage selon le rôle et la configuration.

### Code PIN de déverrouillage rapide

HeelonVault propose un déverrouillage rapide par code PIN pour éviter de ressaisir le mot de passe maître après chaque verrouillage automatique.

**Activation** (section Profil → Sécurité de session) :

1. Cliquer sur « Activer le code PIN ».
2. Saisir un code PIN de 4 à 8 chiffres.
3. Confirmer le code PIN.
4. Le PIN est actif immédiatement pour la session en cours.

**Utilisation lors du déverrouillage automatique** :

- Lors d'un verrouillage par inactivité (délai configuré), la fenêtre de saisie du PIN s'affiche.
- Saisir le code PIN et appuyer sur « Déverrouiller ».
- En cas d'erreur, 3 tentatives sont autorisées avant que le cache ne soit effacé.
- Après 3 échecs ou 12 h d'inactivité, le système bascule automatiquement vers la connexion par mot de passe maître.
- Le bouton « Utiliser le mot de passe » permet de revenir à tout moment à la connexion complète.

**Désactivation** :

- Depuis le profil, cliquer sur « Désactiver le code PIN » pour supprimer le cache immédiatement.

**Indicateur de temps de session dans la barre de titre** :

Lorsque le PIN est actif, un badge « PIN actif » apparaît dans la barre de titre de la fenêtre. Ce badge évolue visuellement en fonction du temps restant avant l'expiration du cache :

- **Nominal** (plus de 2 h restantes) : texte blanc semi-transparent, comportement standard.
- **Avertissement** (entre 15 min et 2 h) : bordure et texte ambre — envisager de se redéconnecter pour renouveler la session si besoin.
- **Critique** (moins de 15 min) : badge fond ambre avec animation pulsante, texte affiche « PIN · Xm » (X = minutes restantes). Le cache expirera dans la minute indiquée.

Survoler le badge affiche une infobulle indiquant le temps exact restant (ex. « Expire dans 1h 23m »). Cliquer sur le badge ouvre le panneau de gestion PIN dans la vue profil.

Le badge et son minuteur sont automatiquement supprimés lorsque le cache expire, que la session est déverrouillée avec le mot de passe maître, ou que l'application est fermée.

**Limites de sécurité à retenir** :

- Le PIN ne remplace pas le mot de passe maître ; il accélère uniquement le déverrouillage de session.
- Le cache PIN est strictement en mémoire vive et disparaît à la fermeture de l'application.
- Ne pas choisir un PIN identique à un code déjà utilisé par ailleurs (téléphone, carte bancaire).

Points d'attention généraux :

- activer le TOTP dès que possible ;
- utiliser un délai d'auto-verrouillage court sur poste partagé ;
- après un changement de mot de passe maître, vérifier rapidement l'accès aux coffres principaux ;
- ne jamais laisser une session ouverte sans surveillance.

Cet écran correspond à l'espace de gestion de la confiance utilisateur. C'est ici que se concentrent les réglages qui influencent directement le niveau de protection du coffre.

Emplacement capture d'écran : Écran 7 - profil et sécurité

![Écran 7 - Profil et sécurité](../assets/images/user-guide/hv_userprofil.png)

Capture 07 - Paramètres de profil, sécurité de session, TOTP, import/export et préférences.

## 9. Écran 8 - Import et export

Selon les autorisations disponibles, HeelonVault permet :

- l'import CSV ;
- l'export au format `.hvb` ;
- des opérations encadrées par les règles RBAC.

Le flux d'import CSV est désormais explicite et guidé :

- **Étape 1 - Prévisualisation** : après sélection du fichier, l'application affiche le nombre de secrets détectés, le nombre importable, et les lignes à revoir manuellement.
- **Étape 2 - Progression** : pendant l'import, une fenêtre dédiée affiche l'avancement (traités/importés/en échec) avec mise à jour en continu.
- **Étape 3 - Résumé final** : l'application affiche un bilan détaillé (total/importés/en échec) et liste les premières lignes non importées avec la raison pour correction manuelle.

Avant un import :

- vérifier le format et l'encodage du fichier ;
- nettoyer les colonnes inutiles ;
- confirmer la destination correcte du coffre.
- lire le résumé final et corriger les lignes signalées avant un second import ciblé.
- consulter le chemin du rapport de rejets si affiché (fichier `logs/csv_import_rejects_*.txt`).

Avant un export :

- limiter l'opération au strict besoin ;
- protéger le fichier exporté ;
- supprimer l'artefact après usage si possible.

Emplacement capture d'écran : Écran 8 - import / export

![Écran 8 - Import / Export depuis le profil](../assets/images/user-guide/hv_userprofil.png)

Capture 08 - Zone Gestion des données (export .hvb et import CSV) accessible dans Profil & Sécurité.

## 10. Écran 9 - Tableau de bord et audit

Le tableau de bord de sécurité donne une vue synthétique de l'état du coffre. Les journaux d'audit permettent de tracer les actions sensibles.

Le tableau de bord met en avant la productivité quotidienne :

- tri prioritaire des cartes par fréquence d'usage ;
- badges visuels sur les cartes (robustesse, incomplet, doublon, usage, santé) ;
- sélection claire de la carte active pour enchaîner rapidement les actions clavier.

Rôle de l'écran :

- visualiser rapidement les points d'attention ;
- suivre les événements récents ;
- appuyer les revues de sécurité et de conformité.

Utilisations courantes :

- identifier les secrets faibles ;
- vérifier les événements récents ;
- suivre les suppressions, modifications et partages.

Emplacement capture d'écran : Écran 9 - tableau de bord sécurité

![Écran 9a - Tableau de bord sécurité](../assets/images/user-guide/hv_dashboard_empty.png)

Capture 09a - Tableau de bord principal et indicateurs d'audit de sécurité.

![Écran 9b - Gestion des équipes](../assets/images/user-guide/hv_team.png)

Capture 09b - Vue d'administration des équipes (partage de coffres, gestion des membres).

![Écran 9c - Gestion des utilisateurs](../assets/images/user-guide/hv_users.png)

Capture 09c - Vue d'administration des utilisateurs (création, rôles, réinitialisation, suppression).

## 11. Bonnes pratiques

- Utiliser un mot de passe maître unique et robuste.
- Activer le TOTP dès l'activation du compte.
- Stocker la clé de récupération hors de la machine.
- Verrouiller ou fermer la session en quittant le poste.
- Réviser régulièrement les secrets obsolètes.
- Limiter les exports aux besoins réels.

## 12. Dépannage rapide

### Impossible de se connecter

- vérifier le nom du compte ;
- vérifier le mot de passe ;
- vérifier l'heure système si le TOTP échoue.

### L'application semble verrouillée trop vite

- vérifier le délai d'auto-verrouillage dans les paramètres de session.

### Un secret a disparu

- vérifier la corbeille avant toute conclusion ;
- consulter le journal d'audit si disponible.

### L'import CSV échoue avec une erreur de déchiffrement

- vérifier que le coffre cible est bien accessible avec la session en cours ;
- se déconnecter puis se reconnecter si un changement de mot de passe maître vient d'être effectué ;
- relancer l'import et contrôler le résumé des lignes rejetées.

## Références utiles

- [QUICKSTART.md](QUICKSTART.md)
- [ARCHITECTURE.md](ARCHITECTURE.md)
- [UPDATE_GUIDE.md](UPDATE_GUIDE.md)
