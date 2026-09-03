# Runbook — Packaging Windows MSI HeelonVault

> **Cible :** MSYS2 MINGW64 shell sur Windows x86_64, target Rust `x86_64-pc-windows-gnu`  
> **WiX :** v7 (`wix.exe`)  
> **Scripts d'automatisation :** `scripts/windows/setup_win_env.ps1` (PowerShell) et `scripts/windows/setup_msys2.sh` (Bash)
> **Validé avec :** Nouveau workflow (staging.wxs + collect-staging.sh + setup_windows_resources)

---

## 0. Pré-requis — Configuration Automatique via Scripts

> **✅ NOUVEAU :** Utilisez les scripts dédiés pour configurer l'environnement **automatiquement**.

### 0.1 Configuration Windows (PowerShell - Admin)

**Script :** `scripts/windows/setup_win_env.ps1`

> **À exécuter EN PREMIER dans PowerShell (admin)** :
> Ce script installe et configure **tous les outils Windows** :
> - Git
> - .NET SDK 8+ (requis pour WiX v7)
> - MSYS2 MINGW64
> - WiX v7 (outil global .NET)
> - Acceptation de la licence WiX
> - Configuration du PATH pour MSYS2

```powershell
# Depuis la racine du repo (ou n'importe où)
# EXÉCUTER EN TANT QU'ADMINISTRATEUR
Set-Location C:\dev\heelonvault-core
.\scripts\windows\setup_win_env.ps1
```

> **✅ Résultat attendu :** Toutes les commandes Windows de base sont disponibles.
> **⚠️ IMPORTANT :** Après exécution, **redémarrez MSYS2 MINGW64** pour appliquer les changements de PATH.

### 0.2 Configuration MSYS2 MINGW64 (Bash)

**Script :** `scripts/windows/setup_msys2.sh`

> **À exécuter DEUXIÈMEMENT dans MSYS2 MINGW64** :
> Ce script configure **tout l'environnement de build** :
> - Mise à jour MSYS2
> - Installation des paquets de compilation (git, ntldd, imagemagick, glib2, gtk4, etc.)
> - Installation de Rustup/cargo dans MSYS2
> - Configuration permanente du PATH
> - Vérification complète de tous les outils

```bash
# Lancer MSYS2 MINGW64 (via menu Démarrer ou C:\msys64\mingw64.exe)
# Puis depuis la racine du repo :
cd /c/dev/heelonvault-core
bash scripts/windows/setup_msys2.sh
```

> **✅ Résultat attendu :** Toutes les commandes de build sont disponibles dans MSYS2.
> **⚠️ IMPORTANT :** Ce script **doit** être exécuté **après** `setup_win_env.ps1`.

### 0.3 Vérification finale de la stack

> **Dans 2 terminaux distincts** :

**PowerShell (outils Windows)** :
```powershell
echo "=== Git ==="; git --version
echo "=== Rustup ==="; rustup --version
echo "=== dotnet ==="; dotnet --version
echo "=== WiX ===" wix --version
echo "=== WiX path ==="; (Get-Command wix).Source
```

**MSYS2 MINGW64 (outils build)** :
```bash
echo "=== Rust ===" && rustc --version && cargo --version
echo "=== ntldd ===" && ntldd --version
echo "=== ImageMagick ===" && convert --version | head -1
echo "=== glib-compile-schemas ===" && glib-compile-schemas --version
echo "=== gdk-pixbuf-query-loaders ===" && command -v gdk-pixbuf-query-loaders
echo "=== python3 ===" && python3 --version
echo "=== ls ===" && ls --version 2>&1 | head -1
```

> **✅ Toutes les commandes doivent retourner une version sans erreur.**
> **Si une commande échoue, relancez les scripts correspondants.**

## 1. Build du binaire

> **Ce runbook couvre le build MSI avec Premium intégré**.
> Pour un build Community (sans code premium), utiliser `--no-default-features` et omettre `--features premium`.


### 1.1 Prérequis Premium — Configuration SSH pour MSYS2

> **Pour inclure le code premium dans le MSI** (nécessaire pour ce runbook) :
> Le crate `heelonvault-premium` est référencé dans `Cargo.toml` comme dépendance git :
> ```toml
> heelonvault-premium = { git = "ssh://git@github.com/ppaperso/heelonvault-premium.git", branch = "main", optional = true }
> ```
> **C'est donc `cargo` qui récupère le code via SSH**, et **MSYS2 MINGW64** exécute cargo.

#### Votre Configuration SSH Actuelle (Validée)

Vous avez configuré MSYS2 avec :
- **Clé privée :** `~/.ssh/premium_deploy_key`
- **Clé publique :** `~/.ssh/premium_deploy_key.pub`
- **Fichier config :** `~/.ssh/config` avec un host dédié

**Votre configuration `~/.ssh/config` :**
```
Host github.com-heelonvault-premium
    HostName github.com
    User git
    IdentityFile ~/.ssh/premium_deploy_key
    IdentitiesOnly yes
    StrictHostKeyChecking accept-new
```

**✅ Vérification que tout est en place :**
```bash
# Dans MSYS2 MINGW64 :
echo "=== Vérification SSH ==="
echo "Clé privée : $(test -f ~/.ssh/premium_deploy_key && echo '✅ PRÉSENTE' || echo '❌ ABSENTE')"
echo "Clé publique : $(test -f ~/.ssh/premium_deploy_key.pub && echo '✅ PRÉSENTE' || echo '❌ ABSENTE')"
echo "Config : $(test -f ~/.ssh/config && echo '✅ PRÉSENT' || echo '❌ ABSENT')"

# Test d'accès au repo premium
git ls-remote git@github.com-heelonvault-premium:ppaperso/heelonvault-premium.git
```

> **✅ Résultat attendu :**
> - Les 3 fichiers sont présents
> - `git ls-remote` retourne une liste de refs **sans erreur**

> **Si `Permission denied (publickey)`** :
> 1. Vérifiez les permissions : `chmod 600 ~/.ssh/premium_deploy_key`
> 2. Vérifiez que la clé publique est sur GitHub : [https://github.com/settings/keys](https://github.com/settings/keys)
> 3. Vérifiez le contenu du config : `cat ~/.ssh/config`
> 4. Testez l'authentification générale : `ssh -T git@github.com-heelonvault-premium`

> **✅ Lorsque `git ls-remote` fonctionne, vous pouvez passer à l'étape 1.2 (Build avec Premium).**

### 1.2 Build avec Premium (pour le MSI)

```bash
# Depuis la racine du repo, dans MSYS2 MINGW64
# --features premium active le téléchargement et la compilation du code premium
cargo build --release --locked --target x86_64-pc-windows-gnu \
  -p heelonvault-app \
  --features premium
```

> `--target x86_64-pc-windows-gnu` est explicite et obligatoire : c'est ce chemin (`target/x86_64-pc-windows-gnu/release/`) que `wix/main.wxs` attend, indépendamment du host/toolchain par défaut.

**Vérification** :

```bash
ls -lh target/x86_64-pc-windows-gnu/release/heelonvault.exe
file target/x86_64-pc-windows-gnu/release/heelonvault.exe
# Attendu : PE32+ executable (GUI) x86-64
```

> **Important** : Le binaire doit être de type **GUI** (subsystem "windows") et non **CUI** (console).
>
> **Note sur le licensing** : Le code premium est **inclus** dans le binaire. L'affichage des fonctionnalités premium dépend de la **présence d'une licence valide** (`LicenseService::load_license()`). Sans licence, l'application fonctionne en mode Community avec les menus premium masqués.

### 1.3 Build Community (optionnel - sans code premium)

> **Si vous ne voulez PAS inclure le code premium** (build open-source uniquement) :

```bash
cargo build --release --locked --target x86_64-pc-windows-gnu \
  -p heelonvault-app \
  --no-default-features
```

> **⚠️ Attention** : Ce build **n'inclut pas** le code premium et ne peut pas afficher les fonctionnalités premium même avec une licence valide.

---

## 2. Staging — copie des fichiers pour le MSI

**⚠️ NOUVEAU WORKFLOW (remplace collect-dlls.sh) :**

Le script `collect-staging.sh` **ne génère PAS de XML WiX**. Il copie uniquement les fichiers nécessaires dans `wix/staging/`.
Le fragment WiX `staging.wxs` utilise `<Files Include="wix\staging\**\*">` pour inclure automatiquement tous les fichiers.

```bash
bash scripts/collect-staging.sh \
  --binary   target/x86_64-pc-windows-gnu/release/heelonvault.exe \
  --msys2    /mingw64 \
  --staging  wix/staging
```

**Note importante :** Le paramètre `--out` a été supprimé car aucun fichier .wxs n'est généré.

### Sorties attendues

| Chemin | Contenu |
| -------- | --------- |
| `wix/staging/heelonvault.exe` | Binaire principal |
| `wix/staging/*.dll` | DLLs mingw64 transitives (30-80 fichiers) |
| `wix/staging/share/glib-2.0/schemas/gschemas.compiled` | Schémas GLib compilés |
| `wix/staging/lib/gdk-pixbuf-2.0/2.10.0/loaders/*.dll` | Loaders pixbuf (DLLs uniquement) |
| `wix/staging/lib/gdk-pixbuf-2.0/2.10.0/loaders.cache` | **❌ NON PRÉSENT** — Exclu intentionnellement |
| `wix/staging/share/themes/Adwaita/**` | Thème GTK4 Adwaita (filtré : CSS, PNG, SVG, index.theme) |
| `wix/staging/share/icons/Adwaita/**` | Icônes Adwaita (filtré : tailles standard uniquement) |
| `wix/staging/migrations/*.sql` | Migrations SQL copiées depuis `migrations/` |
| `wix/staging/heelonvault.ico` | Icône multi-résolution |

**✅ Améliorations clés :**
- **loaders.cache n'est PAS copié** — GDK_PIXBUF_MODULEDIR est configuré dans le code Rust pour un scan dynamique
- **Filtrage agressif** : uniquement les fichiers essentiels (tailles d'icônes standard, fichiers thème nécessaires)
- **Taille réduite** : ~80-120 Mo au lieu de ~500 Mo

### Vérifications post-script

```bash
# Nombre de DLLs stagés (doit être > 0, typiquement 30-80)
ls wix/staging/*.dll | wc -l

# gschemas.compiled présent
ls -lh wix/staging/share/glib-2.0/schemas/gschemas.compiled

# loaders.cache DOIT être ABSENT (exclu intentionnellement)
! test -f wix/staging/lib/gdk-pixbuf-2.0/2.10.0/loaders.cache && echo "OK: loaders.cache absent"

# Vérification alternative : zero fichier .cache dans staging
find wix/staging -name "*.cache" -type f | wc -l  # Doit afficher 0

# Thème et icônes Adwaita staged (doit être > 0)
find wix/staging/share/themes/Adwaita -type f | wc -l
find wix/staging/share/icons/Adwaita -type f | wc -l

# Migrations staged — doit matcher le nombre de fichiers dans migrations/
ls wix/staging/migrations/*.sql | wc -l
ls migrations/*.sql | wc -l

# Icône présente
ls -lh wix/staging/heelonvault.ico
```

### Point d'attention : chemin de l'icône

`collect-staging.sh` cherche l'icône source ici :

```text
assets/icons/hicolor/256x256/apps/heelonvault.png
```

Ce chemin est **relatif au répertoire de travail** au moment de l'appel du script.  
Lance le script **depuis la racine du repo**, pas depuis `crates/heelonvault-core/`.

Si l'icône est manquante, le script continue avec un warning — mais le build WiX échouera car `main.wxs` référence `wix\staging\heelonvault.ico`.

---

## 3. Build MSI

**⚠️ NOUVEAU : Plus que 2 fichiers .wxs au lieu de 3**

```bash
# Depuis la racine du repo, dans un terminal avec wix.exe dans le PATH
# (PowerShell ou MSYS2 si wix.exe est accessible)
wix build \
  wix/main.wxs \
  wix/staging.wxs \
  -d ProductVersion=1.1.0 \
  -d BinaryPath=target\x86_64-pc-windows-gnu\release\heelonvault.exe \
  -o heelonvault-windows-x86_64.msi
```

> **Nouvelles variables obligatoires :**
> - `ProductVersion=X.Y.Z` — comme avant (X.Y.Z sans suffixe `-rc.N`)
> - `BinaryPath=...` — **NOUVEAU** : chemin vers le binaire (remplace le chemin en dur dans main.wxs)

> `main.wxs` déclare `Version="$(var.ProductVersion)"` et `Source="$(var.BinaryPath)"` — des variables de préprocesseur. Les flags `-d` sont **obligatoires**.

### Erreurs fréquentes et résolutions

| Erreur | Cause probable | Fix |
| -------- | ---------------- | ----- |
| `Undefined preprocessor variable: ProductVersion` | `-d ProductVersion=...` manquant | Ajouter le flag `-d ProductVersion=X.Y.Z` à `wix build` |
| `Undefined preprocessor variable: BinaryPath` | `-d BinaryPath=...` manquant | Ajouter le flag `-d BinaryPath=target\x86_64-pc-windows-gnu\release\heelonvault.exe` |
| `Cannot find source file: wix\staging\heelonvault.ico` | Chemin relatif dans `main.wxs` | Lancer `wix build` depuis la racine du repo |
| `bind.FileVersion` vide | EXE sans version resource | Normal pour un build Rust non signé, WiX utilise `0.0.0.0` |

**✅ Les erreurs liées à dlls.wxs ont disparu :**
- `Duplicate symbol 'DllComponents'` — plus applicable
- `Unresolved reference to symbol 'GDKPIXBUF_LOADERS_DIR'` — résolu par staging.wxs

Vérification :

```bash
ls -lh heelonvault-windows-x86_64.msi
# Attendu : fichier > 10 MB (binaire + DLLs + thème + icônes + migrations embarqués)
```

### Vérifier la version réellement embarquée

```powershell
# PowerShell — lit la propriété ProductVersion directement depuis le MSI
$installer = New-Object -ComObject WindowsInstaller.Installer
$database = $installer.GetType().InvokeMember("OpenDatabase", "InvokeMethod", $null, $installer, @("heelonvault-windows-x86_64.msi", 0))
$view = $database.GetType().InvokeMember("OpenView", "InvokeMethod", $null, $database, @("SELECT Value FROM Property WHERE Property='ProductVersion'"))
$view.GetType().InvokeMember("Execute", "InvokeMethod", $null, $view, $null)
$record = $view.GetType().InvokeMember("Fetch", "InvokeMethod", $null, $view, $null)
$record.GetType().InvokeMember("StringData", "GetProperty", $null, $record, 1)
```

---

## 4. Génération du checksum

```bash
# PowerShell
Get-FileHash heelonvault-windows-x86_64.msi -Algorithm SHA256 |
  Select-Object -ExpandProperty Hash |
  ForEach-Object { "$_ heelonvault-windows-x86_64.msi" } |
  Out-File -Encoding ASCII heelonvault-windows-x86_64.msi.sha256

# Ou dans MSYS2
sha256sum heelonvault-windows-x86_64.msi > heelonvault-windows-x86_64.msi.sha256
```

---

## 5. Smoke test local

### 5.1 Installation

```text
msiexec /i heelonvault-windows-x86_64.msi /l*v install.log
```

Vérifie `install.log` si le code retour est non-zéro.

### 5.2 Lancement

- Ouvrir le menu Démarrer → HeelonVault
- Ou `"C:\Program Files\HeelonVault\heelonvault.exe"`
- **L'application se lance comme une application GUI native** (sans fenêtre console visible)
- L'application doit démarrer sans crash ni dialog d'erreur GLib/GTK

> **Note** : Depuis cette version, HeelonVault s'exécute comme une **application GUI native** sur Windows (subsystem "windows"). La fenêtre console ne s'affiche plus. Les logs sont écrits dans `%LOCALAPPDATA%\heelonvault\logs\`.

> Pour activer les logs console lors du développement, utilisez : `set HEELONVAULT_CONSOLE=1` avant de lancer l'application.

### 5.3 Désinstallation

```text
msiexec /x heelonvault-windows-x86_64.msi /l*v uninstall.log
```

Vérifier que `C:\Program Files\HeelonVault\` est supprimé.

### 5.4 Logs et Debug

**Emplacement des logs** :
```text
%LOCALAPPDATA%\heelonvault\logs\heelonvault-YYYY-MM-DD.log
```

**Format** : JSON structuré (un objet par ligne)

**Sur Windows, il n'y a pas de console de debug activable** : le binaire est compilé en mode GUI (`windows_subsystem = "windows"`), donc `stdout` n'est jamais visible, quelle que soit la valeur de `HEELONVAULT_CONSOLE`. Le seul moyen de debug est de consulter les fichiers de logs ci-dessus (`type` ou un éditeur de texte).

> `HEELONVAULT_CONSOLE=1` n'a d'effet que sur Linux/macOS, où il ajoute un `console_layer` en plus du `file_layer` déjà toujours actif. Ne pas s'attendre à un comportement équivalent sur Windows.

### 5.5 Checklist QA minimale

- [ ] Installation silencieuse sans erreur
- [ ] Raccourci Start Menu présent et fonctionnel
- [ ] Application se lance
- [ ] Icônes de l'UI visibles (pas d'icônes blanches/manquantes — signe d'un thème Adwaita non embarqué)
- [ ] Pas de DLL manquante au démarrage (pas de popup "msvcrt.dll not found" etc.)
- [ ] Désinstallation propre (dossier supprimé, raccourci supprimé)
- [ ] Checksum SHA256 vérifié : `certutil -hashfile heelonvault-windows-x86_64.msi SHA256`

---

## 6. Artefacts à livrer à QA

```text
heelonvault-windows-x86_64.msi         ← installeur
heelonvault-windows-x86_64.msi.sha256  ← checksum
```

Ces deux fichiers constituent la GitHub Release pour chaque tag RC.

---

## 7. Versioning, tags et release GitHub

### 7.1 Convention de version

- RC test: `vX.Y.Z-rc.N` (exemple: `v1.1.1-rc.1`)
- Stable: `vX.Y.Z`

### 7.2 Pourquoi tag + release

- Le tag fige la version et le commit de build.
- La release sert de point unique de distribution QA (MSI + SHA256).

### 7.3 Procédure recommandée

1. Valider localement ce runbook (build + smoke test).
2. Créer et pousser un tag RC.
3. Publier la release GitHub associée au tag RC avec les 2 artefacts.
4. Après validation QA, promouvoir en tag stable.

### 7.4 Commandes Git (exemple RC)

```bash
git tag -a v1.1.1-rc.1 -m "Windows RC 1.1.1"
git push origin v1.1.1-rc.1
```

---

## Annexe — Séquence complète en une fois

> **Pour un MSI avec Premium intégré** (recommandé) :

```bash
# 1. Build binaire (avec features premium)
cargo build --release --locked --target x86_64-pc-windows-gnu \
  -p heelonvault-app \
  --features premium

# 2. Staging — copie des fichiers (remplace collect-dlls.sh, AUCUN XML généré)
bash scripts/collect-staging.sh \
  --binary   target/x86_64-pc-windows-gnu/release/heelonvault.exe \
  --msys2    /mingw64 \
  --staging  wix/staging

# 3. Build MSI (depuis racine repo, wix.exe dans PATH)
#    NOUVEAU : staging.wxs au lieu de dlls.wxs, + variable BinaryPath
wix build \
  wix/main.wxs \
  wix/staging.wxs \
  -d ProductVersion=1.2.0-rc.1 \
  -d BinaryPath=target\x86_64-pc-windows-gnu\release\heelonvault.exe \
  -o heelonvault-windows-x86_64.msi

# 4. Checksum
sha256sum heelonvault-windows-x86_64.msi > heelonvault-windows-x86_64.msi.sha256
```

> **Pour un build Community (sans Premium)** : Remplacer l'étape 1 par :
> ```bash
> cargo build --release --locked --target x86_64-pc-windows-gnu -p heelonvault-app --no-default-features
> ```
> 
> **Note sur le runtime Windows :** Le code Rust dans `main.rs` configure automatiquement `GDK_PIXBUF_MODULEDIR` et autres variables d'environnement via `setup_windows_resources()`. Cela permet à GTK4 de trouver les loaders dynamiquement sans besoin de `loaders.cache`.

---


---

## Annexe — Référence pour automatisation future (HORS SCOPE)

> **⚠️ CE RUNBOOK EST 100% MANUEL** — Aucune CI/CD n'est configurée.
> Cette section est conservée à titre de **référence technique uniquement**.

**Rappel des changements clés vs ancien workflow :**
- `collect-dlls.sh` → `collect-staging.sh` (pas de génération XML)
- `dlls.wxs` → `staging.wxs` (fragment statique avec `<Files Include>`)
- `--out wix/dlls.wxs` → **supprimé** (aucune sortie XML)
- `loaders.cache` → **exclu** (GDK_PIXBUF_MODULEDIR configuré dans le code Rust)

**Si automatisation future :**
- Remplacer `collect-dlls.sh` par `collect-staging.sh` dans les scripts CI
- Remplacer `wix/dlls.wxs` par `wix/staging.wxs` dans la commande `wix build`
- Ajouter le flag `-d BinaryPath=...` à la commande `wix build`
- **Important :** La note sur `loaders.cache` et chemins Windows **n'est plus applicable** car loaders.cache n'est plus inclus

