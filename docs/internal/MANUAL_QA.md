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
4. Sélectionner une carte puis cliquer dans la recherche : gauche/droite déplace le curseur
   sans changer de carte ; `Maj` + flèche sélectionne le texte. Corriger une faute au milieu
   d'un mot, tester début/fin et navigation par mots, puis copier/couper/coller du texte.
   `Ctrl+C` ne doit jamais copier le mot de passe de la carte depuis un champ texte.
5. `Ctrl+F` (`Cmd+F` sur macOS) sélectionne la recherche existante ; taper un nouveau terme
   puis presser immédiatement `Entrée` : seul le premier résultat correspondant reçoit le
   focus. Sans résultat, le focus reste dans la recherche. Refaire en modes cartes et liste.
6. Après sélection d'une carte, tester les flèches et `Ctrl+C/L/U` dans les champs de l'éditeur,
   les notes multilignes, le profil, un champ mot de passe et un réglage numérique : aucune
   navigation dans les cartes ni action rapide. Revenir à la liste et vérifier ses raccourcis.

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

1. Sans réseau, se connecter sans ouvrir le Centre : badge `...`, puis compteur des éléments
   concernés. Même vérification après verrouillage puis déverrouillage par PIN. Ouvrir le Centre :
   bilan déjà disponible, filtres et correction utilisables, pas de nouvelle analyse inutile.
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
10. Depuis la liste ou le profil, créer/modifier/supprimer un secret, restaurer/purger la
    corbeille, importer un CSV et créer/supprimer un coffre : le badge s'actualise sans ouvrir
    le Centre. Un import ne déclenche pas une analyse par ligne. Vérifier aussi ajout/retrait
    d'un membre d'équipe, suppression d'équipe et partage de coffre dans l'application.
11. Pendant l'analyse, revenir à la liste : elle continue et son résultat actualise le badge.
    Plusieurs demandes rapprochées sont regroupées ; un résultat dépassé ne réapparaît pas.
    Annuler puis actualiser fonctionne. Une erreur ou un coffre indisponible n'affiche jamais
    un badge vert `0`. Au verrouillage, toute analyse programmée ou en cours est annulée.

## 10. Finition visuelle 2.1

1. Sur chaque plateforme (Linux GNOME récent, Fedora, Windows, macOS) : toutes les icônes
   de la barre latérale, des cartes de secrets et du Centre de sécurité s'affichent (aucun carré
   vide ni glyphe de secours) et suivent la couleur du texte.
2. Accent système réglé sur une autre couleur (GNOME : bleu, violet…) : sélections, interrupteurs
   et cases à cocher restent teal. Vérifier aussi un environnement libadwaita < 1.6.
3. Pastille « Nouveau » visible au premier lancement, disparue après le premier clic sur le
   Centre et après redémarrage. Badge de la barre latérale : `...` pendant l'analyse,
   nombre d'éléments concernés, `0` vert seulement après un bilan complet sans constat,
   `?` sans résultat et `!` en cas d'erreur/couverture incomplète sans constat chiffré.
4. Bouclier du Centre : neutre avant analyse, ambre avec constats ou couverture incomplète,
   vert sans constat. Entrée de navigation surlignée seulement quand la page est affichée.
5. Navigation au clavier (Tab) : anneau de focus visible ; un clic souris n'en laisse pas.
6. Relire l'interface en français : aucun mot sans accent ni anglicisme involontaire.

### Boutons, menus et stabilité visuelle

Sur **chaque paquet natif** (AppImage, MSI et DMG), en français puis en anglais :

1. Connexion, PIN, profil et Centre de sécurité : boutons principaux teal foncé arrondis,
   secondaires clairs avec bordure, destructifs rouges. Au survol, appui et focus clavier,
   aucun changement de taille ni déplacement du texte ; la désactivation enlève le relief.
2. Aide de recherche, historique de connexion, certification et listes déroulantes :
   une seule surface de popover ; pas de cadres/ombres répétés autour des lignes ou icônes.
   Pas de texte blanc sur fond blanc dans les popovers de l'en-tête sombre.
3. Ouvrir une liste déroulante au clavier, naviguer avec les flèches et fermer avec Échap.
   Focus visible, survol distinct de la sélection, choix actif conservé. Les actions
   destructives restent rouges au survol et au focus ; les éléments désactivés restent neutres.
4. Survoler et sélectionner les cartes de chaque type, puis basculer cartes/liste :
   aucun saut vertical ni décalage des titres, des logins ou des boutons de copie.
5. Vue et tri : option active sur surface claire avec trait teal ; survoler une autre
   option ne doit pas lui donner l'aspect de l'option sélectionnée.
6. Tester une petite fenêtre et les libellés longs avec la police par défaut du système.
   Les commandes ne doivent pas être coupées ; les décorations natives restent utilisables.
7. Refaire à 100 %, 150 % et 200 % lorsque disponibles (Windows : mise à l'échelle de
   l'affichage ; macOS : écran Retina et résolution mise à l'échelle ; Linux : réglage
   d'affichage). Vérifier aussi le déplacement de fenêtre entre deux écrans d'échelles
   différentes. Sous Linux, `GDK_SCALE=2` permet un contrôle HiDPI entier, mais ne remplace
   ni une échelle fractionnaire ni une recette Windows/macOS.
8. Réduire/désactiver les animations système : focus, sélection et désactivation restent
   compréhensibles sans animation. Tester le rendu Cairo en plus du moteur par défaut
   sous Linux (`GSK_RENDERER=cairo`) pour éviter une dépendance à un effet GPU.
9. En-tête principal : « Connexion [nom] » en texte sombre sur fond clair, flèche lisible au
   repos, au survol et pendant l'ouverture de l'historique. Création **et** modification :
   « Retour » en blanc sur teal, avec bordure et focus visibles. Modification : « Supprimer »
   et son icône en blanc sur rouge, y compris au survol/appui ; état désactivé neutre.
   Le scénario GTK contrôle aussi les couleurs calculées des libellés/icônes avec un seuil
   de contraste de 4,5:1 contre le fond le moins favorable de ces états activés.

Le scénario partagé `native_gtk_smoke` vérifie la stabilité des mesures et positions dans
les états interactifs et à largeur compacte. Avec `HEELONVAULT_UI_SNAPSHOT_DIR`, il produit
aussi des captures des boutons, des sélecteurs et du popover, en plus des pages réelles.
Ces contrôles Linux ne certifient pas les backends Windows/macOS : la recette des trois
paquets reste obligatoire avant publication. Aucun nouveau widget, moteur de rendu,
police ou dépendance système n'est requis par cette finition.

## 11. Profil & Sécurité et licence

- Option « Afficher le mot de passe actuel dans l'éditeur » désactivée : ouvrir un mot de
  passe en modification, vérifier le champ vide et l'encart explicatif. Activer l'option,
  rouvrir l'éditeur : mot de passe prérempli (toujours masqué par le widget), encart absent.
  Désactiver puis rouvrir : encart de nouveau présent. En création, avec ou sans coffre
  présélectionné, cet encart est absent quel que soit le réglage.

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
