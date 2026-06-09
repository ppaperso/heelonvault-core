# Runbook — Packaging Windows MSI HeelonVault

> Validé sur les fichiers réels : `collect-dlls.sh`, `wix/main.wxs`, `Cargo.toml`  
> Cible : MSYS2 MINGW64 shell sur Windows x86_64  
> WiX : v4 (`wix.exe`), pas candle/light

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
rustup target add x86_64-pc-windows-gnu
rustup show   # vérifier que x86_64-pc-windows-gnu est actif
```

### 0.2 Paquets MSYS2 MINGW64

```bash
pacman -S --needed \
  mingw-w64-x86_64-rust \
  mingw-w64-x86_64-ntldd \
  mingw-w64-x86_64-imagemagick \
  mingw-w64-x86_64-glib2 \
  mingw-w64-x86_64-gdk-pixbuf2 \
  mingw-w64-x86_64-gtk3 \
  python3
```

### 0.3 WiX v4

```bash
# Hors MSYS2, dans un terminal Windows standard (ou PowerShell)
dotnet tool install --global wix
wix --version   # doit afficher 4.x.x
```

> `wix.exe` doit être dans le PATH Windows. Vérifie avec `where wix` en PowerShell.

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
cargo build --release --locked -p heelonvault-app
```

> `-p heelonvault-app` cible le package binaire du workspace.  
> Le binaire produit est `target/release/heelonvault.exe`.

Vérification :

```bash
ls -lh target/release/heelonvault.exe
file target/release/heelonvault.exe
# Attendu : PE32+ executable (GUI) x86-64
```

---

## 2. Collect-DLLs — génération de `wix/dlls.wxs`

```bash
bash scripts/collect-dlls.sh \
  --binary   target/release/heelonvault.exe \
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
| `wix/staging/heelonvault.ico` | Icône multi-résolution |
| `wix/dlls.wxs` | Fragment WiX généré |

### Vérifications post-script

```bash
# Nombre de DLLs stagés (doit être > 0, typiquement 30-80)
ls wix/staging/*.dll | wc -l

# gschemas.compiled présent
ls -lh wix/staging/share/glib-2.0/schemas/gschemas.compiled

# loaders.cache non vide
wc -l wix/staging/lib/gdk-pixbuf-2.0/2.10.0/loaders.cache

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
  -o heelonvault-windows-x86_64.msi
```

### Erreurs fréquentes et résolutions

| Erreur | Cause probable | Fix |
| -------- | ---------------- | ----- |
| `Cannot find source file: target\release\heelonvault.exe` | `main.wxs` attend ce chemin relatif depuis la racine | Lancer `wix build` depuis la racine du repo |
| `Cannot find source file: wix\staging\heelonvault.ico` | Chemin relatif dans `main.wxs` | Idem, ou ajuster `--basepath` |
| `Duplicate symbol 'DllComponents'` | `dlls.wxs` corrompu ou généré deux fois | Supprimer `wix/dlls.wxs` et relancer le script |
| `Unresolved reference to symbol 'GDKPIXBUF_LOADERS_DIR'` | `PixbufLoaderComponents` dans `dlls.wxs` référence ce dir déclaré dans `main.wxs` | Les deux `.wxs` doivent être passés à `wix build` |
| `bind.FileVersion` vide | EXE sans version resource | Normal pour un build Rust non signé, WiX utilise `0.0.0.0` |

Vérification :

```bash
ls -lh heelonvault-windows-x86_64.msi
# Attendu : fichier > 10 MB (binaire + DLLs embarqués)
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
cargo build --release --locked -p heelonvault-app

# 2. Collect DLLs + génère dlls.wxs
bash scripts/collect-dlls.sh \
  --binary   target/release/heelonvault.exe \
  --msys2    /mingw64 \
  --staging  wix/staging \
  --out      wix/dlls.wxs

# 3. Build MSI (depuis racine repo, wix.exe dans PATH)
wix build \
  wix/main.wxs \
  wix/dlls.wxs \
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

**`loaders.cache` et chemins Windows** : `gdk-pixbuf-query-loaders` génère un cache avec des chemins absolus MSYS2. Vérifier que les chemins dans `loaders.cache` sont compatibles avec le chemin d'installation MSI (`C:\Program Files\HeelonVault\lib\...`). Si non, un post-processing sed peut être nécessaire.
