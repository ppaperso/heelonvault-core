# Runbook — Packaging Windows MSI HeelonVault

> Validé sur les fichiers réels : `collect-dlls.sh`, `wix/main.wxs`, `Cargo.toml`  
> Cible : MSYS2 MINGW64 shell sur Windows x86_64, target Rust `x86_64-pc-windows-gnu`  
> WiX : v7 (`wix.exe`), pas candle/light — voir `docs/msi_build_procedure.md` pour la procédure pas-à-pas complète (VM dédiée)

---

## 0. Pré-requis — Stack à installer

### 0.0 Outils de base Windows

> **À installer EN PREMIER** (dans PowerShell ou CMD, **avant toute autre étape**) :

```powershell
# 1. Git — OBLIGATOIRE pour cloner le dépôt et les dépendances
winget install --id Git.Git -e --source winget
# Alternative : télécharger depuis https://git-scm.com/download/win

# 2. Rustup — gestionnaire de toolchains Rust
winget install --id Rustlang.Rustup -e --source winget
# Alternative : https://win.rustup.rs/

# 3. .NET SDK 8+ — requis pour WiX v7 (dotnet tool)
winget install --id Microsoft.DotNet.SDK.8 -e --source winget
# Alternative : https://dotnet.microsoft.com/download/dotnet/8.0
```

> **Vérifications post-installation** :
> ```powershell
> git --version
> rustup --version
> dotnet --version    # Doit afficher 8.x.x ou plus
> ```

> **⚠️ ERREUR COURANTE** : Si vous obtenez `git: The term 'git' is not recognized...`, c'est que Git n'est pas encore installé.
> **Solution** : Installez Git **avant** de tenter de cloner le dépôt (voir ci-dessus).

### 0.0.5 Installation de MSYS2 MINGW64

> **À installer après les outils de base** (toujours dans PowerShell ou CMD) :

```powershell
# Installer MSYS2 avec le package MINGW64
winget install --id MSYS2.MSYS2 -e --source winget
# Alternative : télécharger depuis https://www.msys2.org/
```

> **Lancement de MSYS2 MINGW64** :
> - Via le menu Démarrer : `MSYS2 MinGW 64-bit` (rechercher "MinGW" ou "MSYS2")
> - Ou depuis l'explorateur : `C:\msys64\mingw64.exe`
> - **Ne pas utiliser** `msys2.exe` ou `bash.exe` — il faut spécifiquement **`mingw64.exe`**

> **Première mise à jour MSYS2** (obligatoire après installation) :
> ```bash
> # Dans la fenêtre MSYS2 MINGW64 qui s'ouvre :
pacman -Syu
# Si pacman demande de fermer la fenêtre, relancer MSYS2 MINGW64 et exécuter à nouveau :
pacman -Syu
# Attendre la fin complète avant de continuer
```

> **⚠️ Compatibilité Rustup entre PowerShell et MSYS2** :
> Rustup installé via `winget` dans PowerShell **n'est pas disponible** dans MSYS2 (environnements PATH séparés).
> **Deux options** :
> 
> **Option 1 — Recommandée** : Installer rustup **directement dans MSYS2** :
> ```bash
> # Dans MSYS2 MINGW64, après la mise à jour pacman :
> curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
> # Suivre les instructions, choisir "Proceed with installation (default)"
> # ⚠️ IMPORTANT : Le script rustup modifie ~/.bashrc mais celui-ci n'est pas toujours sourcé automatiquement dans MSYS2.
> # Pour une solution immédiate dans la session actuelle :
> export PATH="$PATH:/c/Users/$USER/.cargo/bin"
> # Pour une solution permanente, ajoutez ces lignes à votre ~/.bashrc :
> echo 'export PATH="$PATH:/c/Users/'$USER'/.cargo/bin"' >> ~/.bashrc
> echo 'export PATH="$PATH:/c/Users/'$USER'/.rustup/toolchains/stable-x86_64-pc-windows-gnu/bin"' >> ~/.bashrc
> # Puis fermez et relancez MSYS2 MINGW64
> ```
> 
> **Option 2** : Ajouter le PATH Windows à MSYS2 (si vous préférez utiliser rustup installé via winget) :
> ```bash
> # Dans MSYS2 MINGW64, $USERPROFILE = /c/Users/<votre_utilisateur>
> export PATH="$PATH:$USERPROFILE/.cargo/bin"
> export PATH="$PATH:$USERPROFILE/.rustup/toolchains/stable-x86_64-pc-windows-gnu/bin"
> # Vérifier :
> rustup --version
> ```

### 0.1 Récupération du code source

> **Deuxième étape** — cloner le dépôt **après avoir installé Git** :

```powershell
# Dans PowerShell ou CMD, choisir un dossier de travail (ex: C:\dev)
cd C:\dev

# Cloner le dépôt (remplacer par le branch si nécessaire)
git clone https://github.com/ppaperso/heelonvault-core.git
cd heelonvault-core
```

> **Vérification** (toujours en PowerShell, MSYS2/Git Bash pas encore installés à ce stade) :
> ```powershell
> Get-Location
> # Attendu (suffixe) : ...\heelonvault-core
>
> Test-Path Cargo.toml, wix\main.wxs, scripts\collect-dlls.sh
> # Attendu : True, True, True (un booléen par chemin, dans l'ordre)
> ```

> **Note** : Toutes les commandes de ce runbook doivent être lancées depuis ce répertoire (`heelonvault-core/`).

### 0.2 Rust toolchain (target Windows natif)

> **Se positionner dans le repo depuis MSYS2** — MSYS2 (installé en 0.0.5) a son propre `$HOME` et ses propres chemins (`/c/...`), distincts de PowerShell. Ouvrir **MSYS2 MINGW64** (via menu Démarrer ou `C:\msys64\mingw64.exe`) et naviguer explicitement vers le dossier cloné :
> ```bash
> cd /c/dev/heelonvault-core   # adapter si un autre dossier a été choisi en 0.1
> pwd && test -f Cargo.toml && test -f rust-toolchain.toml
> # Attendu : aucune sortie d'erreur
> ```

```bash
# Dans MSYS2 MINGW64, depuis la racine du repo (voir ci-dessus)
# Installer la toolchain pinnée dans rust-toolchain.toml
rustup set default-host x86_64-pc-windows-gnu
rustup toolchain install $(grep channel rust-toolchain.toml | cut -d'"' -f2)-x86_64-pc-windows-gnu
rustup default $(grep channel rust-toolchain.toml | cut -d'"' -f2)-x86_64-pc-windows-gnu
rustup show   # vérifier que x86_64-pc-windows-gnu est actif
```

> **Si `rust-toolchain.toml` est manquant** (ex: clone shallow) :
> ```bash
> # Installer la version MSRV requise (1.98.0 pour ce repo)
> rustup install 1.98.0-x86_64-pc-windows-gnu
> rustup default 1.98.0-x86_64-pc-windows-gnu
> ```

### 0.3 Paquets MSYS2 MINGW64

> **À exécuter dans MSYS2 MINGW64** (installé en 0.0.5, lancé via menu Démarrer ou `C:\msys64\mingw64.exe`) :

```bash
pacman -S --needed \
  mingw-w64-x86_64-rust \
  mingw-w64-x86_64-pkgconf \
  mingw-w64-x86_64-ntldd \
  mingw-w64-x86_64-imagemagick \
  mingw-w64-x86_64-glib2 \
  mingw-w64-x86_64-gdk-pixbuf2 \
  mingw-w64-x86_64-gtk4 \
  mingw-w64-x86_64-graphene \
  mingw-w64-x86_64-libepoxy \
  mingw-w64-x86_64-libadwaita \
  python3
```

> GTK4 + `libadwaita` (pas GTK3) — l'app utilise libadwaita, qui requiert aussi le thème d'icônes Adwaita (collecté automatiquement par `collect-dlls.sh`, section 2).

### 0.4 WiX v7

```powershell
# Hors MSYS2, dans PowerShell (nécessite .NET SDK 8+ installé en 0.0)
dotnet tool install --global wix
wix eula accept wix7   # obligatoire (OSMF EULA v1.1) sinon le build échoue
wix --version          # doit afficher 7.x.x
```

> **Vérifications** :
> ```powershell
> where wix    # Doit retourner un chemin (ex: C:\Users\...\.dotnet\tools\wix.exe)
> ```
>
> `wix.exe` doit être dans le PATH Windows. Si l'organisation dépasse 10 000 $/an de revenus sur des projets utilisant WiX, un sponsoring OSMF est requis (voir `https://docs.firegiant.com/wix/osmf/`).

### 0.5 Vérification de la stack complète

> **À exécuter dans 2 terminaux distincts** :

**1. Dans PowerShell (outils Windows)** :
```powershell
echo "=== Git ==="; git --version
echo "=== Rustup ==="; rustup --version
echo "=== dotnet ==="; dotnet --version    # Doit afficher 8.x.x ou plus
echo "=== WiX ==="; wix --version
echo "=== WiX path ==="; where wix
```

**2. Dans MSYS2 MINGW64 (outils build)** :
```bash
echo "=== Rust ===" && rustc --version && cargo --version
echo "=== ntldd ===" && ntldd --version
echo "=== ImageMagick ===" && convert --version | head -1
echo "=== glib-compile-schemas ===" && glib-compile-schemas --version
echo "=== gdk-pixbuf-query-loaders ===" && gdk-pixbuf-query-loaders --version 2>&1 | head -1
echo "=== python3 ===" && python3 --version
echo "=== wix ===" && wix --version 2>&1 | head -1
```

**Résultats attendus** : Chaque commande retourne une version **sans erreur**. Si une commande échoue, revenir à la section correspondante (0.0, 0.2, 0.3, ou 0.4).

---

## 1. Build du binaire

> **Ce runbook couvre le build MSI avec Premium intégré**.
> Pour un build Community (sans code premium), utiliser `--no-default-features` et omettre `--features premium`.

### 1.1 Prérequis Premium

> **Pour inclure le code premium dans le MSI** (nécessaire pour ce runbook) :

> ⚠️ **À clarifier avant de suivre cette section** : comment `--features premium` récupère-t-il concrètement le code du repo privé — dépendance git dans `Cargo.toml` (résolue par `cargo` lui-même), submodule, ou script dans `build.rs` ? Le mécanisme détermine où l'authentification SSH doit être disponible (variable selon que c'est `cargo`, `git`, ou un sous-processus qui fait l'appel). À documenter ici une fois confirmé — le reste de cette section suppose une dépendance git classique.

> **Vérifier l'accès SSH au repo premium — depuis MSYS2 MINGW64**, pas PowerShell : c'est ce shell qui exécute `cargo build` en 1.2, donc c'est son `$HOME`/agent SSH qui doit être configuré, pas celui de PowerShell (les deux ont des `~/.ssh` distincts par défaut).

```bash
# Dans MSYS2 MINGW64
git ls-remote git@github.com:ppaperso/heelonvault-premium.git
# Doit lister les refs (branches/tags) du repo sans erreur.
# Un simple "ssh -T git@github.com" confirme l'authentification GitHub générale
# mais PAS l'accès en lecture à ce repo précis — préférer ls-remote.
```

> **Si l'accès échoue** :
> - Vérifier qu'une clé SSH existe dans le `$HOME` de MSYS2 : `ls ~/.ssh/id_ed25519.pub 2>/dev/null || ls ~/.ssh/id_rsa.pub 2>/dev/null`
> - Si absente, soit en générer une nouvelle dans MSYS2, soit copier celle déjà utilisée côté Windows (`cp /c/Users/<user>/.ssh/id_ed25519* ~/.ssh/`)
> - Ajouter la clé publique à votre compte GitHub : `Settings > SSH and GPG keys`
> - Assurez-vous que le compte a accès en **lecture** au repo `ppaperso/heelonvault-premium`

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

## 2. Collect-DLLs — génération de `wix/dlls.wxs`

```bash
bash scripts/collect-dlls.sh \
  --binary   target/x86_64-pc-windows-gnu/release/heelonvault.exe \
  --msys2    /mingw64 \
  --staging  wix/staging \
  --out      wix/dlls.wxs
```

### Sorties attendues

| Chemin | Contenu |
| -------- | --------- |
| `wix/staging/*.dll` | DLLs mingw64 transitives |
| `wix/staging/share/glib-2.0/schemas/gschemas.compiled` | Schémas GLib compilés |
| `wix/staging/lib/gdk-pixbuf-2.0/2.10.0/loaders/*.dll` | Loaders pixbuf |
| `wix/staging/lib/gdk-pixbuf-2.0/2.10.0/loaders.cache` | Cache loaders |
| `wix/staging/share/themes/Adwaita/**` | Thème GTK4 Adwaita (packagé via `GTK4ThemeComponents`) |
| `wix/staging/share/icons/Adwaita/**` | Thème d'icônes Adwaita requis par libadwaita (packagé via `IconComponents`) |
| `wix/staging/migrations/*.sql` | Migrations SQL copiées depuis `migrations/` (packagé via `MigrationsComponents`) |
| `wix/staging/heelonvault.ico` | Icône multi-résolution |
| `wix/dlls.wxs` | Fragment WiX généré — DllComponents, PixbufLoaderComponents, GTK4ThemeComponents, IconComponents, MigrationsComponents |

### Vérifications post-script

```bash
# Nombre de DLLs stagés (doit être > 0, typiquement 30-80)
ls wix/staging/*.dll | wc -l

# gschemas.compiled présent
ls -lh wix/staging/share/glib-2.0/schemas/gschemas.compiled

# loaders.cache non vide
wc -l wix/staging/lib/gdk-pixbuf-2.0/2.10.0/loaders.cache

# Thème et icônes Adwaita staged (non vide)
find wix/staging/share/themes/Adwaita -type f | wc -l
find wix/staging/share/icons/Adwaita -type f | wc -l

# Migrations staged — doit matcher le nombre de fichiers dans migrations/
ls wix/staging/migrations/*.sql | wc -l
ls migrations/*.sql | wc -l

# Icône présente
ls -lh wix/staging/heelonvault.ico

# dlls.wxs bien formé (doit se terminer par </Wix>)
tail -3 wix/dlls.wxs
```

### Point d'attention : chemin de l'icône

`collect-dlls.sh` cherche l'icône source ici :

```text
assets/icons/hicolor/256x256/apps/heelonvault.png
```

Ce chemin est **relatif au répertoire de travail** au moment de l'appel du script.  
Lance le script **depuis la racine du repo**, pas depuis `crates/heelonvault-core/`.

Si l'icône est manquante, le script continue avec un warning — mais le build WiX échouera car `main.wxs` référence `wix\staging\heelonvault.ico`.

---

## 3. Build MSI

```bash
# Depuis la racine du repo, dans un terminal avec wix.exe dans le PATH
# (PowerShell ou MSYS2 si wix.exe est accessible)
wix build \
  wix/main.wxs \
  wix/dlls.wxs \
  -d ProductVersion=1.1.0 \
  -o heelonvault-windows-x86_64.msi
```

> `main.wxs` déclare `Version="$(var.ProductVersion)"` — une variable de préprocesseur, jamais un littéral. Le flag `-d ProductVersion=X.Y.Z` est **obligatoire** (X.Y.Z sans suffixe `-rc.N`, le format MSI ne l'accepte pas).

### Erreurs fréquentes et résolutions

| Erreur | Cause probable | Fix |
| -------- | ---------------- | ----- |
| `Undefined preprocessor variable: ProductVersion` | `-d ProductVersion=...` manquant | Ajouter le flag `-d ProductVersion=X.Y.Z` à `wix build` |
| `Cannot find source file: target\x86_64-pc-windows-gnu\release\heelonvault.exe` | `main.wxs` attend ce chemin relatif depuis la racine | Lancer `wix build` depuis la racine du repo, avec un binaire buildé via `--target x86_64-pc-windows-gnu` |
| `Cannot find source file: wix\staging\heelonvault.ico` | Chemin relatif dans `main.wxs` | Idem, ou ajuster `--basepath` |
| `Duplicate symbol 'DllComponents'` | `dlls.wxs` corrompu ou généré deux fois | Supprimer `wix/dlls.wxs` et relancer le script |
| `Unresolved reference to symbol 'GDKPIXBUF_LOADERS_DIR'` / `ADWAITAFOLDER` / `ADWAITAICONSFOLDER` / `MIGRATIONSFOLDER` | `PixbufLoaderComponents`/`GTK4ThemeComponents`/`IconComponents`/`MigrationsComponents` dans `dlls.wxs` référencent ces dirs déclarés dans `main.wxs` | Les deux `.wxs` doivent être passés à `wix build` |
| `bind.FileVersion` vide | EXE sans version resource | Normal pour un build Rust non signé, WiX utilise `0.0.0.0` |

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

# 2. Collect DLLs + thème + icônes + migrations, génère dlls.wxs
bash scripts/collect-dlls.sh \
  --binary   target/x86_64-pc-windows-gnu/release/heelonvault.exe \
  --msys2    /mingw64 \
  --staging  wix/staging \
  --out      wix/dlls.wxs

# 3. Build MSI (depuis racine repo, wix.exe dans PATH)
wix build \
  wix/main.wxs \
  wix/dlls.wxs \
  -d ProductVersion=1.2.0-rc.1 \
  -o heelonvault-windows-x86_64.msi

# 4. Checksum
sha256sum heelonvault-windows-x86_64.msi > heelonvault-windows-x86_64.msi.sha256
```

> **Pour un build Community (sans Premium)** : Remplacer l'étape 1 par :
> ```bash
> cargo build --release --locked --target x86_64-pc-windows-gnu -p heelonvault-app --no-default-features
> ```

---

## Annexe — Points de vigilance pour la transposition en GitHub Actions

Une fois ce runbook validé localement, voici les deltas à anticiper pour le workflow CI :

**Shell** : le runner `windows-latest` a Git Bash disponible. Appeler le script via `shell: bash` dans le step. MSYS2 complet nécessite l'action `msys2/setup-msys2@v2`.

**PATH de wix** : après `dotnet tool install --global wix`, ajouter `$env:USERPROFILE\.dotnet\tools` au PATH du runner. **Requiert .NET SDK 8+** (WiX v7 n'est pas compatible avec .NET 7).

**Chemin relatif de l'icône** : dans le runner, `GITHUB_WORKSPACE` est la racine du repo — s'assurer que le working directory du step est la racine, pas un sous-dossier.

**Trigger tags** : préférer un pattern robuste (`v*-rc*`) avec validation regex dans le job pour éviter les faux déclenchements.

**Cache cargo** : utiliser `actions/cache` sur `~/.cargo/registry` et `target/` pour éviter de rebuilder toutes les dépendances à chaque run.

**Signature Authenticode** : le MSI produit par ce runbook n'est pas signé — Windows 11 SmartScreen avertira les utilisateurs à l'installation. Si un certificat de signature de code est disponible, ajouter une étape `signtool sign /fd sha256 /tr <timestamp-url> /td sha256 heelonvault-windows-x86_64.msi` avant le calcul du checksum (étape 4). Sans certificat, documenter ce risque dans les notes de release.

**`loaders.cache` et chemins Windows** : `gdk-pixbuf-query-loaders` génère un cache avec des chemins absolus MSYS2. Vérifier que les chemins dans `loaders.cache` sont compatibles avec le chemin d'installation MSI (`C:\Program Files\HeelonVault\lib\...`). Si non, un post-processing sed peut être nécessaire.

--- 

### 🔐 Points spécifiques pour le build Premium en CI

**Accès au repo privé heelonvault-premium** :
- Le `GITHUB_TOKEN` par défaut généré par Actions n'a accès qu'au repo courant (`heelonvault-core`) — il ne donne **pas** accès à `heelonvault-premium`. Il faut un **Personal Access Token fine-grained** dédié, avec accès en lecture (`Contents: Read-only`) explicitement accordé sur le repo `ppaperso/heelonvault-premium`, stocké comme secret (ex: `PREMIUM_REPO_TOKEN`).
- **Ne jamais** stocker le token en clair dans les fichiers
- Exemple pour GitHub Actions — **attention à utiliser le bon secret dans `run:`**, pas le `GITHUB_TOKEN` par défaut :
  ```yaml
  - name: Configure Git for private repo
    run: |
      git config --global url."https://${{ env.GITHUB_TOKEN }}@github.com".insteadOf "ssh://git@github.com"
    env:
      GITHUB_TOKEN: ${{ secrets.PREMIUM_REPO_TOKEN }}  # PAT fine-grained, Contents:Read sur heelonvault-premium
  ```
  > La version précédente référençait `secrets.GITHUB_TOKEN` dans `run:` alors que `env:` déclarait `PREMIUM_REPO_TOKEN` — les deux noms ne correspondaient pas, et `secrets.GITHUB_TOKEN` n'aurait de toute façon pas eu accès au repo privé.

**Build avec features premium** :
```yaml
- name: Build with Premium
  run: |
    cargo build --release --locked --target x86_64-pc-windows-gnu \
      -p heelonvault-app \
      --features premium
```

**Sécurité du cache Cargo** :
- Le cache (`~/.cargo/registry/cache/`) contient le code source de `heelonvault-premium`
- **Exclure** ce cache des artefacts publics ou des logs
- Utiliser `actions/cache` avec prudence :
  ```yaml
  - name: Cache Cargo
    uses: actions/cache@v3
    with:
      path: |
        ~/.cargo/registry/index/
        ~/.cargo/registry/cache/
        target/
      key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
    # ⚠️ Le cache contient du code premium — ne pas le partager publiquement
  ```

**Nettoyage des logs** :
- Les logs de build peuvent contenir des paths vers `heelonvault-premium`
- Filtrer les logs avant publication :
  ```yaml
  - name: Sanitize logs
    run: |
      # Masquer les paths du repo privé dans les logs
      cargo build ... 2>&1 | sed 's/ppaperso\/heelonvault-premium/[REDACTED]/g'
  ```
  