# Démarrage rapide (Rust)

Langue : FR | [EN](QUICKSTART.md)

Version rapide documentée : `1.2.0-rc.1`

---

## Prérequis

- **Toolchain Rust** : `1.98.0` (épinglée via `rust-toolchain.toml`)
- **GTK4** : Requis pour l'interface desktop
- **libadwaita** : Bibliothèque compagnon de GTK4
- **SQLite** : Base de données backend

Vérifiez votre environnement :

```bash
rustc --version  # Doit afficher 1.98.0
cargo --version
```

---

## 1. Vérification du build

Vérifiez que le workspace compile sans erreurs :

```bash
cargo check --workspace
```

Pour une compilation complète avec optimisations :

```bash
cargo build --workspace
```

---

## 2. Lancement en développement

Depuis la racine du dépôt :

```bash
./scripts/run-dev.sh
```

**Spécificités de l'environnement de développement** :
- Chemin de la base de développement : `data/heelonvault-rust-dev.db`
- Niveau de log : `debug` (via `HEELONVAULT_LOG_LEVEL=debug`)
- Dossier des logs : `./logs`
- La base de données est créée automatiquement au premier lancement

**Variables d'environnement** (optionnelles) :

```bash
# Modifier le niveau de log
HEELONVAULT_LOG_LEVEL=trace ./scripts/run-dev.sh

# Modifier le dossier des logs
HEELONVAULT_LOG_DIR=/tmp/heelonvault-logs ./scripts/run-dev.sh

# Modifier le chemin de la base de données
HEELONVAULT_DB_PATH=/tmp/heelonvault-dev.db ./scripts/run-dev.sh
```

---

## 3. Lancement des tests

### Tests unitaires et d'intégration

Exécuter tous les tests :

```bash
cargo test --workspace
```

Exécuter des modules de test spécifiques :

```bash
# Tests des repositories
cargo test secret_repository:: -- --nocapture
cargo test user_repository:: -- --nocapture

# Tests des services
cargo test secret_service:: -- --nocapture
cargo test auth_service:: -- --nocapture

# Tests d'intégration
cargo test --workspace --test login_history_integration
cargo test --workspace --test account_rekey_integration  # NOUVEAU en v1.2.0-rc.1
```

### Vérification Clippy

Assurez la qualité du code :

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

---

## 3bis. Vérifications UI recommandées (v1.2.0-rc.1)

### Fonctionnalités de session et PIN (NOUVELLES)

1. **Configuration du PIN** :
   - Ouvrir `Profil & Sécurité` depuis la barre latérale
   - Configurer un code PIN de 4 à 8 chiffres
   - Vérifier que le badge PIN apparaît dans la barre de titre
   - Tester l'auto-verrouillage : la boîte de dialogue de déverrouillage PIN doit apparaître

2. **Badge PIN et minuteur** :
   - Vérifier que le badge PIN affiche l'état correct (nominal, avertissement, critique)
   - Survoler le badge pour voir l'infobulle du compte à rebours de session
   - Vérifier que le texte du badge est lisible sur la barre de titre foncée

3. **Cycle de vie de la session** :
   - Fermer la fenêtre principale avec le bouton de fermeture : l'écran de connexion doit réapparaître
   - Se reconnecter immédiatement : les cartes de secrets doivent être visibles
   - Vérifier que l'auto-verrouillage déclenche le déverrouillage PIN (quand le PIN est activé)

### Navigation par cartes et raccourcis clavier

4. **Sélection de carte** :
   - Cliquer une fois sur une carte de secret : elle doit devenir active sans ouvrir le mode édition
   - Vérifier que la carte active a une mise en évidence visible

5. **Édition de carte** :
   - Double-cliquer sur la même carte : le formulaire d'édition doit s'ouvrir
   - Vérifier que tous les champs sont correctement remplis

6. **Raccourcis clavier** :
   - Sur la carte active, tester les raccourcis :
     - `Ctrl+C` : copier la valeur du secret dans le presse-papier
     - `Ctrl+L` : copier la valeur de connexion dans le presse-papier
     - `Ctrl+U` : ouvrir l'URL dans le navigateur par défaut
   - Vérifier l'effacement automatique du presse-papier après 60 secondes

### Recherche et MultiCoffre (NOUVEAU en v1.2.0-rc.1)

7. **Fonctionnalités de recherche** :
   - Utiliser la barre de recherche pour trouver des secrets par titre, connexion, email, URL, notes, catégorie, tags ou type
   - Tester la syntaxe par champ : `email:`, `tag:`, `type:`
   - Tester la syntaxe avec espace après les deux-points : `champ: valeur` == `champ:valeur`

8. **Bouton bascule MultiCoffre** :
   - Cliquer sur le bouton bascule **MultiCoffre** (à gauche de la barre de recherche)
   - Vérifier que la recherche fonctionne sur tous les coffres quand activé
   - Vérifier que la recherche est limitée au coffre actif quand désactivé

9. **Marqueur santé** :
   - En création/édition, cocher « Accès données de santé »
   - Enregistrer le secret
   - Vérifier que la recherche avec `#sante` trouve le secret marqué
   - Vérifier que le badge « Sante » apparaît sur les cartes correspondantes

### Import et récupération (NOUVEAU en v1.2.0-rc.1)

10. **Import CSV** (Fonctionnalité Premium) :
    - Aller dans `Profil & Sécurité` > Import
    - Sélectionner un fichier CSV
    - Vérifier le flux en 3 étapes : prévisualisation, progression, résumé final
    - Vérifier la tolérance aux erreurs : les imports partiels doivent fonctionner
    - Vérifier le fichier `csv_import_rejects_*.txt` dans `HEELONVAULT_LOG_DIR` si des lignes sont rejetées

11. **Récupération de compte** (NOUVEAU en v1.2.0-rc.1) :
    - Lors du bootstrap : vérifier la génération de la clé de récupération (phrase mnémotechnique de 24 mots au format BIP39)
    - Vérifier la vérification obligatoire de 2 mots tirés au hasard
    - Vérifier la copie dans le presse-papier avec effacement automatique après 60 secondes
    - Depuis `Profil & Sécurité` : vérifier la ré-exportation de la clé de récupération

---

## 4. Build de production

Compilation pour release :

```bash
cargo build --release
```

Le binaire sera créé à l'emplacement :

```bash
./target/release/heelonvault
```

### Installateur Linux packagé

L'archive (`heelonvault-linux-x86_64.tar.gz`) déploie :

- **Chemin du binaire** : `/opt/heelonvault/heelonvault`
- **Lanceur** : `/opt/heelonvault/run.sh`
- **Entrée desktop** : `/usr/share/applications/com.heelonvault.rust.desktop`
- **Ancienne entrée desktop** : `/usr/share/applications/heelonvault.desktop`
- **Base utilisateur** : `~/.local/share/heelonvault/heelonvault-rust.db`
- **Logs utilisateur** : `~/.local/state/heelonvault/logs`
- **Dossier des migrations** : `/opt/heelonvault/migrations` (19 migrations en v1.2.0-rc.1)

Installation :

```bash
tar -xzf heelonvault-linux-x86_64.tar.gz
cd heelonvault-linux-x86_64
sudo ./scripts/install.sh
```

### Variables d'environnement de production

Le `run.sh` généré exporte :

```bash
HEELONVAULT_MIGRATIONS_DIR=/opt/heelonvault/migrations
HEELONVAULT_DB_PATH=~/.local/share/heelonvault/heelonvault-rust.db
HEELONVAULT_LOG_DIR=~/.local/state/heelonvault/logs
HEELONVAULT_LOG_LEVEL=info
```

---

## Vérifications post-installation (Ubuntu)

### Vérifications du binaire et du lanceur

```bash
# Vérifier que le binaire existe et est exécutable
test -x /opt/heelonvault/heelonvault

# Vérifier que le lanceur existe et est exécutable
test -x /opt/heelonvault/run.sh

# Vérifier que les entrées desktop existent
test -f /usr/share/applications/com.heelonvault.rust.desktop
test -f /usr/share/applications/heelonvault.desktop

# Valider le format de l'entrée desktop
desktop-file-validate /usr/share/applications/com.heelonvault.rust.desktop

# Tester le lancement via l'entrée desktop
gtk-launch com.heelonvault.rust
```

### Vérifications de la base de données et des migrations

```bash
# Vérifier que le dossier de la base de données existe
ls -la ~/.local/share/heelonvault/

# Vérifier le dossier et les fichiers de migration (19 migrations en v1.2.0-rc.1)
ls -la /opt/heelonvault/migrations/ | wc -l  # Doit afficher 19 + 1 (en-tête)

# Vérifier le contenu de chaque fichier SQL de migration
grep -c "CREATE TABLE\|ALTER TABLE\|CREATE INDEX" /opt/heelonvault/migrations/*.sql
```

### Vérifications PIN et session (NOUVELLES en v1.2.0-rc.1)

```bash
# Vérifier les tables liées au PIN
sqlite3 ~/.local/share/heelonvault/heelonvault-rust.db "SELECT name FROM sqlite_master WHERE type='table' AND name LIKE '%pin%';"

# Vérifier les tables de rate limiting (par IP)
sqlite3 ~/.local/share/heelonvault/heelonvault-rust.db "SELECT name FROM sqlite_master WHERE type='table' AND name='login_attempts_ip';"

# Vérifier la table des enveloppes de clé de récupération (NOUVELLE en v1.2.0-rc.1)
sqlite3 ~/.local/share/heelonvault/heelonvault-rust.db "SELECT name FROM sqlite_master WHERE type='table' AND name='user_recovery_key_envelopes';"
```

### Vérifications des permissions

```bash
# Vérifier les permissions de la base de données (doit être 0600)
stat -c "%a" ~/.local/share/heelonvault/heelonvault-rust.db

# Vérifier les permissions du dossier des logs
stat -c "%a" ~/.local/state/heelonvault/
```

---

## Notes de migration Legacy

### Depuis v0.4 vers v1.2.0-rc.1

Les anciens installateurs pouvaient stocker la base dans `/opt/heelonvault/data/heelonvault-rust-dev.db`. 
Le lanceur packagé copie ce fichier vers le dossier utilisateur au premier démarrage si nécessaire.

Pour une migration manuelle :

```bash
# Utiliser le script de migration fourni
./scripts/export-legacy-v0.4-to-csv.py --db-path /var/lib/heelonvault-shared/old.db \
  --salt-path /var/lib/heelonvault-shared/salt.txt \
  --output legacy_export.csv

# Puis importer via le flux d'import CSV (Fonctionnalité Premium)
```

### Depuis v1.1.0 vers v1.2.0-rc.1

Le schéma de la base de données a été mis à jour avec 5 nouvelles migrations (14 → 19 au total) :

- Migration 0015 : Index supplémentaires pour les performances
- Migration 0016 : Table de rate limiting par IP (`login_attempts_ip`)
- Migration 0017 : Table de cache PIN
- Migration 0018 : Table d'état de session
- Migration 0019 : Table des enveloppes de clé de récupération utilisateur (Récupération de clé de compte)

Ces migrations sont appliquées automatiquement au premier lancement.

---

## Résolution des problèmes

### Problèmes courants

**Problème : GSK_RENDERER non défini**

Solution : Assurez-vous que les variables de rendu GTK sont définies avant l'initialisation de Tokio :

```bash
# Vérifier si la variable est exportée
echo $GSK_RENDERER

# Exécuter avec un rendu explicite
gsk_renderer=gl ./scripts/run-dev.sh
```

**Problème : La base de données existe déjà avec un schéma obsolète**

Solution : Sauvegardez et laissez les migrations s'exécuter :

```bash
mv data/heelonvault-rust-dev.db data/heelonvault-rust-dev.db.bak
./scripts/run-dev.sh  # Va créer une nouvelle base avec le schéma actuel
```

**Problème : Dépendances GTK4 manquantes**

Solution (Ubuntu/Debian) :

```bash
sudo apt-get install libgtk-4-dev libadwaita-1-dev
```

**Problème : Les logs n'apparaissent pas**

Solution : Vérifiez les variables d'environnement :

```bash
# Vérifier que le dossier des logs existe
mkdir -p ./logs

# Exécuter avec des paramètres de log explicites
HEELONVAULT_LOG_LEVEL=debug HEELONVAULT_LOG_DIR=./logs ./scripts/run-dev.sh
```

---

## Ressources supplémentaires

- [Index de la documentation complète](../README.md)
- [Détails de l'architecture](ARCHITECTURE.md)
- [Guide utilisateur](USER_GUIDE.md)
- [Journal des modifications](CHANGELOG.md)

---

> **Note** : Pour les fonctionnalités premium (import CSV, partage d'équipe, etc.), assurez-vous que votre licence est correctement configurée dans `~/.config/heelonvault/license.hvl` (dev) ou `/etc/heelonvault/license.hvl` (prod).
