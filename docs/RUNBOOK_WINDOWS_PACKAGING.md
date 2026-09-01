# Runbook — Packaging Windows MSI HeelonVault

> Validé sur les fichiers réels : `collect-dlls.sh`, `wix/main.wxs`, `Cargo.toml`  
> Cible : MSYS2 MINGW64 shell sur Windows x86_64, target Rust `x86_64-pc-windows-gnu`  
> WiX : v7 (`wix.exe`), pas candle/light — voir `docs/msi_build_procedure.md` pour la procédure pas-à-pas complète (VM dédiée)

---

## 0. Pré-requis — Stack à installer

### 0.0 Répertoire de travail (obligatoire)

Toutes les commandes de ce runbook doivent être lancées depuis la racine du repo.

```bash
pwd
# Attendu (suffixe): .../heelonvault-core

test -f Cargo.toml && test -f wix/main.wxs && test -f scripts/collect-dlls.sh
# Attendu: aucune sortie d'erreur
```

### 0.1 Rust toolchain (target Windows natif)

```bash
# Dans MSYS2 MINGW64
# rust-toolchain.toml pin le channel (ex: 1.98.0) sans host — installer le host
# GNU explicitement pour ce repo évite d'installer un toolchain "stable" inutile.
rustup toolchain install $(grep channel ../rust-toolchain.toml | cut -d'"' -f2)-x86_64-pc-windows-gnu
rustup show   # vérifier que x86_64-pc-windows-gnu est actif pour ce repo
```

### 0.2 Paquets MSYS2 MINGW64

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

### 0.3 WiX v7

```bash
# Hors MSYS2, dans un terminal Windows standard (ou PowerShell)
dotnet tool install --global wix
wix eula accept wix7   # obligatoire (OSMF EULA v1.1) sinon le build échoue
wix --version   # doit afficher 7.x.x
```

> `wix.exe` doit être dans le PATH Windows. Vérifie avec `where wix` en PowerShell.
> Si l'organisation dépasse 10 000 $/an de revenus sur des projets utilisant WiX, un sponsoring OSMF est requis (voir `docs.firegiant.com/wix/osmf/`).

### 0.4 Vérification de la stack complète

Exécute ce bloc dans MSYS2 MINGW64 avant tout :

```bash
echo "=== Rust ===" && rustc --version && cargo --version
echo "=== ntldd ===" && ntldd --version
echo "=== ImageMagick ===" && convert --version | head -1
echo "=== glib-compile-schemas ===" && glib-compile-schemas --version
echo "=== gdk-pixbuf-query-loaders ===" && gdk-pixbuf-query-loaders --version 2>&1 | head -1
echo "=== python3 ===" && python3 --version
echo "=== wix ===" && wix --version
```

**Résultats attendus** : chaque commande retourne une version sans erreur.

---

## 1. Build du binaire

```bash
# Depuis la racine du repo, dans MSYS2 MINGW64
cargo build --release --locked --target x86_64-pc-windows-gnu -p heelonvault-app
```

> `--target x86_64-pc-windows-gnu` est explicite et obligatoire : c'est ce chemin (`target/x86_64-pc-windows-gnu/release/`) que `wix/main.wxs` attend, indépendamment du host/toolchain par défaut.

Vérification :

```bash
ls -lh target/x86_64-pc-windows-gnu/release/heelonvault.exe
file target/x86_64-pc-windows-gnu/release/heelonvault.exe
# Attendu : PE32+ executable (GUI) x86-64
```

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
- L'application doit démarrer sans crash ni dialog d'erreur GLib/GTK

### 5.3 Désinstallation

```text
msiexec /x heelonvault-windows-x86_64.msi /l*v uninstall.log
```

Vérifier que `C:\Program Files\HeelonVault\` est supprimé.

### 5.4 Checklist QA minimale

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

```bash
# 1. Build binaire
cargo build --release --locked --target x86_64-pc-windows-gnu -p heelonvault-app

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
  -d ProductVersion=1.1.0 \
  -o heelonvault-windows-x86_64.msi

# 4. Checksum
sha256sum heelonvault-windows-x86_64.msi > heelonvault-windows-x86_64.msi.sha256
```

---

## Annexe — Points de vigilance pour la transposition en GitHub Actions

Une fois ce runbook validé localement, voici les deltas à anticiper pour le workflow CI :

**Shell** : le runner `windows-latest` a Git Bash disponible. Appeler le script via `shell: bash` dans le step. MSYS2 complet nécessite l'action `msys2/setup-msys2@v2`.

**PATH de wix** : après `dotnet tool install --global wix`, ajouter `$env:USERPROFILE\.dotnet\tools` au PATH du runner.

**Chemin relatif de l'icône** : dans le runner, `GITHUB_WORKSPACE` est la racine du repo — s'assurer que le working directory du step est la racine, pas un sous-dossier.

**Trigger tags** : préférer un pattern robuste (`v*-rc*`) avec validation regex dans le job pour éviter les faux déclenchements.

**Cache cargo** : utiliser `actions/cache` sur `~/.cargo/registry` et `target/` pour éviter de rebuilder toutes les dépendances à chaque run.

**Signature Authenticode** : le MSI produit par ce runbook n'est pas signé — Windows 11 SmartScreen avertira les utilisateurs à l'installation. Si un certificat de signature de code est disponible, ajouter une étape `signtool sign /fd sha256 /tr <timestamp-url> /td sha256 heelonvault-windows-x86_64.msi` avant le calcul du checksum (étape 4). Sans certificat, documenter ce risque dans les notes de release.

**`loaders.cache` et chemins Windows** : `gdk-pixbuf-query-loaders` génère un cache avec des chemins absolus MSYS2. Vérifier que les chemins dans `loaders.cache` sont compatibles avec le chemin d'installation MSI (`C:\Program Files\HeelonVault\lib\...`). Si non, un post-processing sed peut être nécessaire.
