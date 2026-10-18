# Recette manuelle avant livraison

Langue : FR

Parcours à dérouler sur chaque paquet candidat (AppImage, DMG, MSI) avant de publier une
version. Utiliser une machine ou une VM **vierge** (sans toolchain ni MSYS2 sous Windows : une
bibliothèque manquante du paquet serait sinon chargée silencieusement depuis le système). Les
contrôles automatisés (tests, clippy, semver, SBOM) sont décrits dans
[DEVELOPMENT.fr.md](DEVELOPMENT.fr.md).

## 1. Installation et premier lancement

1. Vérifier l'empreinte SHA-256 du paquet, l'installer selon [QUICKSTART.fr.md](../QUICKSTART.fr.md).
2. Assistant d'initialisation : création du compte, phrase de récupération de 24 mots,
   vérification obligatoire de 2 mots tirés au hasard, copie avec effacement après 60 s.
3. Fin de l'assistant : l'écran de connexion s'affiche, identifiant pré-rempli, message
   « Compte créé, connectez-vous. ». Fermer l'assistant ne quitte pas l'application.
4. Vérifier l'emplacement de la base et des logs (tableau « Où sont mes données ? »), et sous
   Linux les permissions (`stat -c "%a" <base>` → `600`).
5. Mise à jour : installer le paquet de la version précédente, créer des secrets, installer le
   candidat par-dessus, vérifier que les secrets sont toujours là.

## 2. Session

1. Fermer la fenêtre principale : l'écran de connexion réapparaît. Se reconnecter : les secrets
   sont visibles.
2. Échecs de connexion répétés : l'attente avant nouvelle tentative augmente.
3. TOTP activé : un code valide ne peut pas être réutilisé immédiatement.
4. Code PIN (4 à 8 chiffres) depuis `Profil & Sécurité` : badge PIN dans la barre de titre,
   états nominal / avertissement / critique, infobulle du compte à rebours lisible.
5. Auto-verrouillage : le déverrouillage par PIN est proposé quand le PIN est actif.
6. Changer le mot de passe maître, se déconnecter, se reconnecter : tous les coffres s'ouvrent.

## 3. Liste des secrets

1. Clic simple sur une carte : sélection sans ouvrir l'éditeur ; double-clic : éditeur avec
   tous les champs remplis.
2. Bascule cartes / liste : instantanée, sans écran « Chargement » ; filtre, tri et recherche
   conservés ; le choix survit à un redémarrage.
3. Bouton de tri actif mis en évidence.
4. Badges en anglais : un mot de passe faible n'est jamais affiché en vert « robuste ».

## 4. Presse-papiers

1. `Ctrl+C` sur la carte active copie le mot de passe ; `Ctrl+L` le login ; `Ctrl+U` ouvre l'URL.
2. L'indicateur de la barre d'en-tête affiche l'anneau ambré qui se consume pendant 20 s
   (mot de passe, login) ou 60 s (phrase de récupération) ; un clic vide le presse-papiers.
3. Copier un autre texte depuis une autre application : la valeur HeelonVault est effacée
   aussitôt, le nouveau contenu est conservé.
4. Verrouiller la session pendant une copie : rien n'arrive dans le presse-papiers, message
   « Session verrouillée ».
5. Partage révoqué entre deux copies : la copie suivante est refusée.

## 5. Recherche

1. Recherche par titre, login, email, URL, notes, catégorie, tags, type ; syntaxe `email:`,
   `tag:`, `type:` ; `champ: valeur` équivaut à `champ:valeur`.
2. Bouton MultiCoffre : recherche dans tous les coffres, ou seulement le coffre actif.
3. Marqueur « Accès données de santé » : `#sante` retrouve le secret, badge « Santé » affiché.

## 6. Import, export, corbeille

1. Import CSV en 3 étapes (prévisualisation, progression, résumé) ; import partiel en cas de
   lignes invalides ; rapport `csv_import_rejects_*.txt` dans le dossier des logs.
2. Rejet des URL non `http/https`, des fichiers trop volumineux et des champs anormalement longs.
3. Export `.hvb` puis restauration ; sous Linux, permissions `600` sur le fichier exporté.
4. Suppression, restauration depuis la corbeille, purge définitive.

## 7. Licence Professionnelle

1. Sans licence : badge « Licence free », boutons Administration et Équipes masqués, menu
   « Certifier & Exporter » désactivé.
2. Avec `license.hvl` valide : sceau « Certifié par Heelonys » au nom du client (connexion et
   barre d'en-tête), gestion des utilisateurs et des
   équipes, rapports d'audit signés 24 h / 7 j / 30 j.
3. Partage d'un coffre à une équipe : sélecteur de coffre explicite avant confirmation ; un
   membre ouvre le coffre selon son rôle (READ / WRITE / ADMIN) ; icône de partage sur les
   coffres partagés.

## 8. Divers

1. Bouton « ? » de la barre d'en-tête : ouvre https://doc.heelonvault.heelonys.fr.
2. Changer de langue (écran de connexion, puis `Profil & Sécurité`) : l'interface se retraduit
   sans redémarrage, y compris le badge de licence, le menu « Certifier & Exporter » et les
   boutons Pro Santé Connect. Aucun texte ne doit rester dans l'autre langue.

## 9. Centre de sécurité et URL

1. Sans réseau, ouvrir le Centre : analyse locale, compteurs, filtres et correction utilisables.
2. Même mot de passe dans deux coffres accessibles : regroupement ; un token identique ne
   participe pas au groupe. Vérifier une expiration passée et une expiration à moins de 30 jours.
3. Filtrer un coffre, déplier un constat, ouvrir l'élément puis revenir : filtre et contexte
   conservés, diagnostic recalculé après correction.
4. Révoquer un partage avant d'ouvrir un résultat : accès refusé ; relancer l'analyse retire
   les éléments du coffre désormais inaccessible.
5. Verrouiller pendant une analyse ou un contrôle DNS/TLS : aucune réponse tardive ne doit
   réafficher un résultat. Le formulaire d'édition est effacé au verrouillage.
6. URL : format invalide rouge, HTTP orange, HTTPS non vérifié neutre. L'enregistrement
   n'exige jamais de connexion réseau.
7. Contrôle TLS : annuler le consentement ne déclenche pas de connexion ; autoriser un
   domaine de test maîtrisé donne un résultat horodaté. Ne pas utiliser de vrais sites
   malveillants. Vérifier qu'une URL avec paramètres n'envoie que l'origine.
8. Une URL modifiée invalide immédiatement le résultat précédent ; timeout/DNS indisponible
   restent neutres, pas rouges. Adresse interne : contrôle en ligne désactivé.
9. Changement FR/EN, navigation clavier, lecteur d'écran, contraste et taille minimale de
   fenêtre : les cartes et constats doivent rester utilisables. Aucun verdict ne repose
   uniquement sur sa couleur ; aucune mention de réputation vérifiée ou de site « sûr ».

## 10. Finition visuelle 2.1

1. Sur chaque plateforme (Linux GNOME récent, Fedora, Windows, macOS) : toutes les icônes
   de la barre latérale, des cartes de secrets et du Centre de sécurité s'affichent (aucun carré
   vide ni glyphe de secours) et suivent la couleur du texte.
2. Accent système réglé sur une autre couleur (GNOME : bleu, violet…) : sélections, interrupteurs
   et cases à cocher restent teal. Vérifier aussi un environnement libadwaita < 1.6.
3. Pastille « Nouveau » visible au premier lancement, disparue après le premier clic sur le
   Centre et après redémarrage. Compteur de la barre latérale : visible après une analyse avec
   constats, masqué sans constat, effacé au verrouillage.
4. Bouclier du Centre : neutre avant analyse, ambre avec constats ou couverture incomplète,
   vert sans constat. Entrée de navigation surlignée seulement quand la page est affichée.
5. Navigation au clavier (Tab) : anneau de focus visible ; un clic souris n'en laisse pas.
6. Relire l'interface en français : aucun mot sans accent ni anglicisme involontaire.

## 11. Profil & Sécurité et licence

1. **Community, compte admin** : la section Administration n'affiche qu'une ligne « Fonctions
   Professional » sans navigation ; aucun chemin (profil, barre latérale, raccourci) ne mène aux
   pages Utilisateurs/Équipes.
2. **Professional, compte admin** : lignes Utilisateurs et Équipes fonctionnelles ; création
   d'un compte et d'une équipe possibles.
3. **Licence retirée ou expirée** (fichier de licence supprimé, redémarrage) : les opérations
   d'octroi échouent avec le dialogue « disponible avec Professional » ; révoquer un accès et
   retirer un membre restent possibles.
4. **Compte non admin** : ni export `.hvb`, ni section Administration.
5. Changer le mot de passe maître pour une valeur se terminant par une espace, verrouiller, se
   reconnecter avec cette valeur exacte : la connexion réussit. Moins de 16 caractères : refus
   avant tout envoi.
6. Choisir « Jamais » pour le verrouillage : confirmation avant application ; « Annuler »
   conserve le délai précédent.
7. Changer la langue : toute l'interface change immédiatement, sans toucher aux modifications
   non enregistrées du bloc Compte.
8. Modifier l'adresse e-mail : le champ « mot de passe actuel » apparaît ; sans modification,
   « Enregistrer » reste inactif.
9. En-tête « Protection du compte » : activer/désactiver 2FA, PIN et verrouillage met à jour le
   compteur sans recharger la page.
