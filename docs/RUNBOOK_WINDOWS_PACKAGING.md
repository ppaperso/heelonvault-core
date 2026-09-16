# Runbook — Packaging Windows MSI HeelonVault

> **Cible :** Windows x86_64, target Rust `x86_64-pc-windows-gnu` (ABI MinGW, imposée par les DLL GTK4 de MSYS2)
> **WiX :** v3.14 (`candle.exe` / `light.exe` / `heat.exe`)
> **Source de vérité :** `.github/workflows/windows-msi-rc.yml` + `scripts/build-msi.ps1`

La référence fiable est le workflow CI : il est exécuté à chaque build et donc toujours à jour.
Ce document le décrit et complète ce qui ne s'y lit pas directement. En cas de divergence,
**le workflow a raison**.

---

## 1. Ce que produit le build

`scripts/build-msi.ps1` enchaîne quatre étapes :

1. `cargo build --release -p heelonvault-app`
2. Staging : l'exe, la fermeture transitive de ses DLL (via `objdump -p`), les loaders
   gdk-pixbuf, `share\{glib-2.0,gtk-4.0,icons,fonts}`, les migrations SQL et les ressources
   applicatives.
3. `heat.exe` → `staging.wxs`
4. `candle.exe` + `light.exe` → `crates\heelonvault-app\wix\output\HeelonVault-<version>.msi`

Le MSI installe sous `C:\Program Files\HeelonVault\` :

```
bin\      heelonvault.exe, toutes les DLL, migrations\
share\    glib-2.0\schemas\ (dont gschemas.compiled), gtk-4.0\, icons\, fonts\
lib\      gdk-pixbuf-2.0\
```

> **Attention :** `share\` et `lib\` sont **frères** de `bin\`, pas ses enfants. Les migrations,
> elles, sont **dans** `bin\`. `setup_windows_resources()` (`crates/heelonvault-app/src/main.rs`)
> tient compte de cette asymétrie — voir §5.

Versionnage : `-Version` est déduit de `crates/heelonvault-app/Cargo.toml`, sauf si passé
explicitement (le CI passe le tag). Un suffixe `-rc.N` devient le 4e segment de version WiX
(`1.2.0-rc.1` → `1.2.0.1`). `Product Id="*"` : **le ProductCode change à chaque build**, donc
ne jamais réutiliser un GUID de désinstallation mémorisé (cf. §4).

---

## 2. Environnement de build

Reproduit l'environnement CI (`windows-msi-rc.yml`) :

| Composant | Détail |
| --- | --- |
| MSYS2 | installé, puis `pacman -S mingw-w64-x86_64-{toolchain,gtk4,libadwaita,pkgconf}` |
| pkg-config | copier `pkgconf.exe` → `pkg-config.exe` dans `mingw64\bin` — la crate Rust `pkg-config` ne cherche que ce nom |
| Rust | `rustup set default-host x86_64-pc-windows-gnu` ; la version est épinglée à 1.98.0 par `rust-toolchain.toml` |
| WiX | v3.14 — zip `wix314-binaries.zip` de la release `wix3141rtm` du dépôt `wixtoolset/wix3`, extrait puis passé via `-WixBin` |
| heelonvault-premium | checkout **en dossier frère** du repo (le workspace le résout en `../heelonvault-premium`), sinon builder avec `--no-default-features` |

Build local :

```powershell
# mingw64\bin doit être dans le PATH de ce shell
.\scripts\build-msi.ps1 -WixBin "C:\chemin\vers\wix314" -Version "1.2.0-rc.1"
```

`-GtkRoot` et `-WixBin` sont auto-détectés (`C:\msys64\mingw64`, `C:\Program Files (x86)\WiX
Toolset v3.14\bin`) ; les passer explicitement si l'installation est ailleurs.

---

## 3. Build via CI (recommandé)

```bash
# build seul, sans publier
gh workflow run windows-msi-rc.yml -f publish_release=false

# build + publication sur une release existante
gh workflow run windows-msi-rc.yml -f rc_tag=v1.2.0-rc.1 -f publish_release=true
```

Un push de tag `v*.*.*` déclenche automatiquement build + publication (prerelease si le tag
porte un suffixe `-rc.N`).

Récupérer l'artefact d'un run :

```bash
gh run download <run-id> -n heelonvault-windows-msi -D .
```

---

## 4. Installer / désinstaller pour tester

```powershell
# Le ProductCode change à chaque build : toujours le relire, jamais le coder en dur
$code = (Get-ItemProperty HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\* |
         Where-Object { $_.DisplayName -like '*HeelonVault*' }).PSChildName
Start-Process msiexec.exe -ArgumentList "/x $code /qn" -Wait

Start-Process msiexec.exe -ArgumentList '/i C:\chemin\heelonvault.msi /qn' -Wait
```

Ajouter `/l*v install.log` pour un journal d'installation détaillé.

---

## 5. Ce que l'application résout au runtime

`setup_windows_resources()` (`crates/heelonvault-app/src/main.rs`) est appelée **avant toute
initialisation GTK** et fixe :

- `GTK_DATA_PREFIX`, `GTK_EXE_PREFIX`, `XDG_DATA_DIRS`, `GSETTINGS_SCHEMA_DIR`,
  `GDK_PIXBUF_MODULEDIR` → depuis la **racine d'installation** (on remonte d'un cran si
  l'exécutable est dans `bin\`), donc `INSTALLFOLDER\share` et `INSTALLFOLDER\lib` ;
- `HEELONVAULT_MIGRATIONS_DIR` → depuis le **dossier de l'exécutable** (`bin\migrations`).

Ces deux racines diffèrent volontairement : ne pas les réunifier sans changer aussi le staging.

> Renseigner `XDG_DATA_DIRS` **désactive** le repli natif de GLib sous Windows
> (`g_win32_get_system_data_dirs`, qui déduit le préfixe du dossier de la DLL chargée). Une
> valeur erronée est donc pire que pas de valeur : elle a provoqué un `g_error()` → `abort()`
> à l'ouverture de tout sélecteur de fichiers, et rendu le thème d'icônes introuvable.

Assets et traductions sont embarqués dans le binaire (GResource, via `build.rs`). Base de
données et journaux vont dans `%LOCALAPPDATA%\Heelonys\HeelonVault\data\heelonvault\{data,logs}`.

### Lancer le binaire de dev sans MSI

`target\release\heelonvault.exe` n'est pas dans un dossier `bin\`, donc les ressources sont
cherchées **à côté de lui**. Il faut y recopier `share\`, `lib\` et `migrations\` (et avoir
`mingw64\bin` dans le PATH pour les DLL), sinon l'application s'arrête sur une recherche de
schéma GSettings.

---

## 6. Diagnostiquer un crash Windows

Le binaire est en sous-système GUI : **aucune console n'est attachée**, stdout/stderr sont
invisibles au double-clic. Trois sources, dans cet ordre :

1. **Rediriger stderr** — fonctionne malgré le sous-système GUI si la redirection vient du
   shell appelant. Les messages GLib (`g_error`, assertions) et les panics Rust y passent :

   ```powershell
   & "C:\Program Files\HeelonVault\bin\heelonvault.exe" 2> "$env:USERPROFILE\Desktop\hv-stderr.log"
   ```

   Ajouter `$env:G_MESSAGES_DEBUG='all'` pour plus de détail, ou `$env:G_DEBUG='fatal-warnings'`
   pour faire échouer au **premier** avertissement GLib plutôt qu'au symptôme final.

2. **Observateur d'événements** — donne module fautif, code d'exception et offset :

   ```powershell
   Get-WinEvent -FilterHashtable @{LogName='Application'; Id=1000} -MaxEvents 5 |
     Where-Object { $_.Message -match 'heelonvault' } | Format-List TimeCreated, Message
   ```

   Lecture des codes : `0xc0000005` = access violation (faute mémoire native) ;
   `0x40000015` = `STATUS_FATAL_APP_EXIT`, c'est-à-dire `abort()` — typiquement un
   `g_error()`/assertion GLib, ou un panic Rust traversant une frontière FFI.

3. **Journaux applicatifs** — `%LOCALAPPDATA%\Heelonys\HeelonVault\data\heelonvault\logs\`.
   Le layer fichier est bufferisé (`tracing_appender::non_blocking`) : en cas d'`abort()` les
   dernières lignes peuvent manquer. **L'absence de trace n'est donc pas une preuve d'absence
   de panic** — d'où la redirection stderr du point 1, écrite synchronement par le hook de panic.

---

## 7. Limites connues

- **Backends d'impression GTK non embarqués.** Ils sont chargés dynamiquement
  (`g_module_open`) et échappent donc au scan des imports PE de `build-msi.ps1`. Le bouton
  « Imprimer » de l'export de clé de récupération est retiré sous Windows pour cette raison.
- **`GIO_MODULE_DIR` non renseigné** et `lib\gio\modules` non stagé — même classe de trou,
  sans impact constaté à ce jour.
- **Pas de signature de code.** Au premier lancement, Windows Defender / SmartScreen analyse
  le binaire non signé : cela explique quelques secondes de latence au tout premier démarrage.
- **Validation MSI minimale en CI** : le workflow vérifie la signature OLE et la taille du
  fichier, pas le contenu du staging.
- **Ne pas installer MSYS2 sur une VM de test.** Avec `mingw64\bin` dans le PATH, une DLL
  manquante du paquet serait silencieusement chargée depuis MSYS2 : le test validerait un
  packaging incomplet. Garder une VM de test vierge (ou un snapshot propre) pour la
  validation finale.
