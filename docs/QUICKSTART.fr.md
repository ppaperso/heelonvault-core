# Installation et démarrage rapide

Langue : FR | [EN](QUICKSTART.md)

Ce guide couvre l'installation de HeelonVault sur votre poste et le premier lancement, jusqu'à
votre premier secret enregistré. Comptez une dizaine de minutes.

## 1. Télécharger

Les paquets sont publiés sur la page
[Releases GitHub](https://github.com/ppaperso/heelonvault-core/releases/latest). Choisissez le
fichier correspondant à votre système :

| Système | Fichier | Configuration requise |
| ------- | ------- | --------------------- |
| Windows | `heelonvault-windows-x86_64-vX.Y.Z.msi` | Windows 10 ou 11, 64 bits, droits administrateur pour l'installation |
| macOS | `heelonvault-macos-aarch64-vX.Y.Z.dmg` | macOS 13 Ventura ou plus récent, Mac Apple Silicon (M1 et suivants) |
| Linux | `heelonvault-linux-x86_64-vX.Y.Z.AppImage` | Distribution 64 bits récente (x86_64) |

Téléchargez aussi le fichier `.sha256` du même nom : il sert à vérifier que le paquet n'a pas
été altéré.

## 2. Vérifier le téléchargement

Placez le paquet et son fichier `.sha256` dans le même dossier, puis :

- **Linux** : `sha256sum -c heelonvault-linux-x86_64-vX.Y.Z.AppImage.sha256`
- **macOS** : `shasum -a 256 -c heelonvault-macos-aarch64-vX.Y.Z.dmg.sha256`
- **Windows** (PowerShell) : comparez la valeur affichée par
  `Get-FileHash .\heelonvault-windows-x86_64-vX.Y.Z.msi -Algorithm SHA256`
  au contenu du fichier `.sha256`.

La commande doit répondre `OK` (Linux, macOS) ou afficher la même empreinte (Windows). Dans le
cas contraire, ne lancez pas le paquet et téléchargez-le à nouveau.

## 3. Installer

### Windows

1. Double-cliquez sur le fichier `.msi`.
2. Le paquet n'est pas signé numériquement : si SmartScreen affiche « Windows a protégé votre
   ordinateur », cliquez sur **Informations complémentaires**, puis **Exécuter quand même**.
3. Acceptez la demande d'élévation : l'application s'installe dans
   `C:\Program Files\HeelonVault` pour tous les utilisateurs du poste.
4. Lancez **HeelonVault** depuis le menu Démarrer. Le tout premier lancement peut prendre
   quelques secondes, le temps que Windows Defender analyse l'application.

### macOS

1. Ouvrez le fichier `.dmg` et faites glisser **HeelonVault** dans le dossier **Applications**.
2. L'application n'est pas signée par un compte développeur Apple, Gatekeeper la bloque donc au
   premier lancement :
   - **macOS 13 et 14** : clic droit (ou Ctrl+clic) sur HeelonVault dans Applications, choisir
     **Ouvrir**, puis confirmer **Ouvrir** ;
   - **macOS 15 et suivants** : lancez l'application une première fois, fermez l'avertissement,
     puis ouvrez **Réglages Système > Confidentialité et sécurité** et cliquez sur
     **Ouvrir quand même** en face de HeelonVault.

   Cette opération n'est nécessaire qu'une seule fois. Alternative en Terminal :
   `xattr -cr /Applications/HeelonVault.app`.

### Linux (AppImage)

L'AppImage embarque GTK4 et libadwaita : rien d'autre à installer.

```bash
chmod +x heelonvault-linux-x86_64-vX.Y.Z.AppImage
./heelonvault-linux-x86_64-vX.Y.Z.AppImage
```

Si le lancement échoue avec une erreur mentionnant FUSE, installez la bibliothèque `libfuse2`
(Ubuntu 24.04 : `sudo apt install libfuse2t64` ; Fedora : `sudo dnf install fuse-libs`).

Pour faire apparaître HeelonVault dans le menu des applications, rangez l'AppImage dans un
dossier stable (par exemple `~/Applications`) et utilisez un outil d'intégration comme
Gear Lever ou AppImageLauncher.

> Administrateurs : une installation système Linux (profil personnel ou entreprise, base
> partagée) est décrite dans [UPDATE_GUIDE.md](UPDATE_GUIDE.md#installation-système-linux).

## 4. Premier lancement : créer votre compte

Au premier démarrage, un assistant crée le compte administrateur du coffre :

1. Choisissez un **identifiant** et un **mot de passe maître** robuste. C'est le seul mot de passe
   à retenir : il ne peut pas être retrouvé par l'éditeur.
2. Notez la **phrase de récupération de 24 mots** affichée. Elle permet de redéfinir le mot de
   passe maître si vous l'oubliez. Conservez-la hors de l'ordinateur (papier rangé en lieu sûr).
3. Confirmez deux mots tirés au hasard pour prouver que la phrase a bien été notée.
4. L'assistant se ferme et l'écran de connexion s'ouvre, identifiant pré-rempli : connectez-vous.

> Sans mot de passe maître ni phrase de récupération, vos secrets sont **irrécupérables**.
> C'est le prix d'un chiffrement que personne d'autre que vous ne peut ouvrir.

## 5. Premiers pas

- **Créer un secret** : bouton **Ajouter** en haut à droite, puis choisissez le type (mot de
  passe, clé API, clé SSH, document sécurisé…). Le bouton **+** de la barre latérale, lui, crée
  un nouveau coffre.
- **Copier un mot de passe** : sélectionnez la carte puis `Ctrl+C`. Le presse-papiers est vidé
  automatiquement après 20 secondes ; l'icône de presse-papiers de la barre d'en-tête montre le
  compte à rebours, un clic l'efface immédiatement.
- **Sécuriser la session** : dans **Profil & Sécurité**, activez la double authentification
  (TOTP), réglez le délai d'auto-verrouillage et, si vous le souhaitez, un code PIN de
  déverrouillage rapide.
- **Importer vos mots de passe** : **Profil & Sécurité > Gestion des données > Importer des
  données (CSV)** accepte un export CSV (colonnes `name`, `url`, `username`, `password`, `notes`).

Le [guide utilisateur](USER_GUIDE.md) décrit chaque écran en détail.

## 6. Où sont mes données ?

Tout reste sur votre poste, dans une base SQLite chiffrée :

| Système | Base de données | Journaux |
| ------- | --------------- | -------- |
| Windows | `%LOCALAPPDATA%\Heelonys\HeelonVault\data\heelonvault\data\` | `%LOCALAPPDATA%\Heelonys\HeelonVault\data\heelonvault\logs\` |
| macOS | `~/Library/Application Support/fr.Heelonys.HeelonVault/heelonvault/data/` | `~/Library/Application Support/fr.Heelonys.HeelonVault/heelonvault/logs/` |
| Linux | `~/.local/share/heelonvault/` | `~/.local/state/heelonvault/logs/` |

Pour sauvegarder vos secrets, préférez l'export chiffré `.hvb` de **Profil & Sécurité** à une
copie brute du fichier de base.

## 7. Licence Professionnelle (optionnel)

Sans licence, HeelonVault fonctionne en édition **Community**, sans limite de durée ni de
nombre de secrets. Une licence **Professionnelle** active en plus l'administration des
utilisateurs, les équipes et coffres partagés, et les rapports d'audit signés : voir
[HeelonVault Premium](https://doc.heelonvault.heelonys.fr/premium/).

Pour l'activer, copiez le fichier `license.hvl` fourni à cet emplacement, puis relancez
l'application :

| Système | Emplacement |
| ------- | ----------- |
| Windows | `C:\ProgramData\HeelonVault\license.hvl` |
| macOS | `~/Library/Application Support/heelonvault/license.hvl` |
| Linux | `/etc/heelonvault/license.hvl` (droits administrateur requis) |

Le badge « Licence free » de l'écran de connexion est remplacé par le sceau « Certifié par
Heelonys » au nom de votre organisation. Un fichier invalide ou expiré est ignoré : l'application reste en édition Community.

## 8. Désinstaller

- **Windows** : Paramètres > Applications > HeelonVault > Désinstaller.
- **macOS** : glissez HeelonVault depuis Applications vers la Corbeille.
- **Linux** : supprimez le fichier AppImage.

La désinstallation **conserve vos données** (voir section 6). Supprimez ce dossier à la main
seulement si vous voulez effacer définitivement vos secrets, après en avoir fait un export si
nécessaire.

## Aller plus loin

- [Guide utilisateur](USER_GUIDE.md) — tous les écrans et fonctions
- [Mettre à jour](UPDATE_GUIDE.md) — passer à une nouvelle version
- [Sécurité](../SECURITY.fr.md) — modèle de menace, cryptographie, signalement de vulnérabilité
