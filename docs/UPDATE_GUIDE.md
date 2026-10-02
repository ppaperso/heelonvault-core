# Mise à jour et déploiement

Langue : FR | [EN](UPDATE_GUIDE.en.md)

Ce guide explique comment passer à une nouvelle version de HeelonVault, puis, pour les
administrateurs, comment réaliser une installation système sous Linux. Pour une première
installation sur un poste, voir [QUICKSTART.fr.md](QUICKSTART.fr.md).

## Avant toute mise à jour

1. Faites un **export chiffré `.hvb`** de vos coffres depuis **Profil & Sécurité > Gestion des
   données**, et conservez-le hors du poste.
2. Notez la version installée (en-tête du [journal des modifications](CHANGELOG.md)) et lisez
   les sections des versions intermédiaires, en particulier les « Changements incompatibles ».
3. Fermez HeelonVault.

Les évolutions du schéma de base sont appliquées **automatiquement** au premier lancement de la
nouvelle version. Elles ne sont pas réversibles : revenir à une version antérieure impose de
restaurer une sauvegarde faite avant la mise à jour.

## Mettre à jour un poste

Téléchargez et vérifiez le nouveau paquet comme pour une première installation
([sections 1 et 2](QUICKSTART.fr.md#1-télécharger) du démarrage rapide), puis :

| Système | Procédure |
| ------- | --------- |
| Windows | Lancez le nouveau `.msi`. Il remplace la version installée ; inutile de désinstaller d'abord. Un `.msi` plus ancien que la version installée est refusé. |
| macOS | Glissez la nouvelle application dans **Applications** et choisissez **Remplacer**. L'étape Gatekeeper du premier lancement est à refaire. |
| Linux (AppImage) | Remplacez l'ancien fichier `.AppImage` par le nouveau, puis rendez-le exécutable (`chmod +x`). |

Vos données ne sont pas touchées par le remplacement de l'application : elles vivent dans un
dossier séparé (voir [Où sont mes données ?](QUICKSTART.fr.md#6-où-sont-mes-données-)).

### AppImage : données créées avec la version 2.0.0

L'AppImage 2.0.0 enregistrait sa base dans un dossier `data/` relatif au dossier depuis lequel
elle était lancée (souvent le dossier personnel : `~/data/heelonvault-rust-dev.db`). Les versions
suivantes utilisent `~/.local/share/heelonvault/heelonvault-rust.db`. Au premier lancement, si ce
nouveau fichier n'existe pas encore, l'AppImage y **copie** automatiquement l'ancienne base
trouvée dans le dossier de lancement ou dans le dossier personnel. L'ancien fichier est laissé
en place : supprimez-le vous-même une fois vos secrets vérifiés.

## Installation système Linux

Réservée aux administrateurs qui veulent une installation dans `/opt/heelonvault` avec intégration
au menu des applications, ou une base partagée sur un serveur. Pour un poste individuel,
l'AppImage suffit.

Les scripts d'installation détectent la distribution (famille Debian/Ubuntu ou
Fedora/RHEL/Rocky/AlmaLinux), installent les dépendances système et proposent deux profils :

| Profil | Base de données | Logs |
| ------ | --------------- | ---- |
| **Personnel** (défaut) | `~/.local/share/heelonvault/heelonvault-rust.db` | `~/.local/state/heelonvault/logs/` |
| **Entreprise** | `/var/lib/heelonvault/heelonvault-rust.db` | `/var/log/heelonvault/` |

Le profil Personnel partage la base de l'AppImage : les deux peuvent coexister.

### Installer

Prérequis : un dépôt `heelonvault-core` au tag voulu, avec le binaire `heelonvault` construit en
mode release et copié à la racine du dépôt (voir la
[documentation de développement](internal/DEVELOPMENT.fr.md)).

```bash
# Aperçu, sans rien modifier
sudo env HEELONVAULT_DRY_RUN=1 ./scripts/install.sh

# Installation
sudo ./scripts/install.sh
```

Le script vérifie l'intégrité du binaire si un fichier `heelonvault.sha256` l'accompagne, copie
l'application et ses migrations dans `/opt/heelonvault`, génère le lanceur `run.sh` et l'entrée
de menu `com.heelonvault.rust.desktop`. Les scripts `install-ubuntu.sh` et `install-rhel.sh`
permettent de forcer une famille de distribution.

En profil Entreprise, le script ne configure que les chemins partagés : la publication réseau
(RDS, VDI, RemoteApp, bastion…) reste à votre charge. Placez la base sur un stockage à faible
latence, idéalement local au serveur d'exécution.

### Mettre à jour

Depuis le dépôt au nouveau tag, avec le nouveau binaire à la racine :

```bash
sudo ./scripts/install.sh
```

Avant de redéployer, le script sauvegarde les bases détectées dans `/var/backups/heelonvault`
(`heelonvault_user_<utilisateur>_backup_AAAAMMJJ_HHMMSS.db` ou
`heelonvault_enterprise_backup_AAAAMMJJ_HHMMSS.db`). Un échec de sauvegarde interrompt la mise
à jour : ne le contournez pas. Vérifiez l'espace disque au préalable (`df -h /var/backups`).

Contrôle après mise à jour :

```bash
test -x /opt/heelonvault/heelonvault && test -x /opt/heelonvault/run.sh && echo OK
stat -c "%a %n" ~/.local/share/heelonvault/heelonvault-rust.db   # attendu : 600
```

### Revenir à la version précédente

```bash
# 1. Réinstaller la version précédente (dépôt au tag précédent, binaire correspondant)
sudo ./scripts/install.sh

# 2. Restaurer la sauvegarde faite avant la mise à jour
ls -lth /var/backups/heelonvault/
cp /var/backups/heelonvault/<sauvegarde>.db ~/.local/share/heelonvault/heelonvault-rust.db
```

### Désinstaller

```bash
sudo ./scripts/remove.sh
```

Les données et les sauvegardes ne sont supprimées que si vous le confirmez explicitement.

## Migration depuis l'ancienne version Python (0.4)

HeelonVault ne lit ni ne modifie jamais les données de l'ancienne application Python. Pour les
reprendre, exportez-les en CSV, puis importez ce fichier depuis **Profil & Sécurité > Gestion des
données > Importer des données (CSV)** :

```bash
# Données dans le dossier par défaut (~/.local/share/passwordmanager), profil donné
./scripts/export-legacy-v0.4-to-csv.py --profile <profil> --output legacy_export.csv

# Ou chemins explicites (déploiement partagé, par exemple sous /var/lib/heelonvault-shared)
./scripts/export-legacy-v0.4-to-csv.py \
  --db-path <chemin>/passwords_<profil>.db \
  --salt-path <chemin>/salt_<profil>.bin \
  --output legacy_export.csv
```

Le fichier CSV contient vos mots de passe **en clair** : supprimez-le de façon sûre dès l'import
terminé.
