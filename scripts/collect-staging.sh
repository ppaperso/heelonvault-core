#!/usr/bin/env bash
# scripts/collect-staging.sh
# Copie les fichiers nécessaires pour le packaging MSI HeelonVault sous Windows.
# 
# Ce script REMPLACE l'ancien collect-dlls.sh et ne génère PAS de XML WiX.
# Il prépare uniquement le dossier wix/staging/ avec tous les fichiers nécessaires.
# Le fragment wix/staging.wxs utilise <Files Include="wix\staging\**\*"> pour inclure
# automatiquement tous les fichiers préparés par ce script.
#
# Usage:
#   bash scripts/collect-staging.sh \
#     --binary <chemin/vers/heelonvault.exe> \
#     --msys2 <chemin/vers/mingw64> \
#     --staging <chemin/vers/dossier/staging>
#
# Exemple:
#   bash scripts/collect-staging.sh \
#     --binary target/x86_64-pc-windows-gnu/release/heelonvault.exe \
#     --msys2 /mingw64 \
#     --staging wix/staging

set -euo pipefail

BINARY=""
MSYS2_ROOT="/mingw64"
STAGING_DIR="wix/staging"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --binary)   BINARY="$2";      shift 2 ;;
    --msys2)    MSYS2_ROOT="$2";  shift 2 ;;
    --staging)  STAGING_DIR="$2"; shift 2 ;;
    *) echo "[ERROR] Unknown arg: $1"; exit 1 ;;
  esac
done

[[ -z "$BINARY" ]] && { echo "[ERROR] --binary is required"; exit 1; }
[[ -f "$BINARY" ]] || { echo "[ERROR] Binary not found: $BINARY"; exit 1; }

echo "[staging] Binary : $BINARY"
echo "[staging] MSYS2  : $MSYS2_ROOT"
echo "[staging] Output : $STAGING_DIR"

# ── 1. Nettoyage & creation de la structure ──────────────────────────
rm -rf "$STAGING_DIR"
mkdir -p "$STAGING_DIR/lib/gdk-pixbuf-2.0/2.10.0/loaders"
mkdir -p "$STAGING_DIR/share/glib-2.0/schemas"
mkdir -p "$STAGING_DIR/share/themes/Adwaita"
mkdir -p "$STAGING_DIR/share/icons/Adwaita"
mkdir -p "$STAGING_DIR/migrations"

# ── 2. Exe principal ─────────────────────────────────────────────────
cp "$BINARY" "$STAGING_DIR/heelonvault.exe"
echo "[staging] Main executable copied"

# ── 3. DLLs transitives (via ntldd -R) ──────────────────────────────
echo "[staging] Collecting DLLs..."
ntldd -R "$BINARY" 2>/dev/null \
  | grep -Ei 'mingw64|msys64' \
  | awk -F'=>' '{print $2}' | awk '{print $1}' \
  | sort -u \
  | while IFS= read -r dll; do
      [[ -z "$dll" ]] && continue
      # Convertir les chemins Windows si nécessaire
      dll_unix=$(cygpath -u "$dll" 2>/dev/null || echo "$dll")
      [[ -f "$dll_unix" ]] && cp -u "$dll_unix" "$STAGING_DIR/"
    done

# DLLs critiques GTK4 (sécurité si ntldd rate une dépendance)
for dll in libgtk-4-1.dll libgraphene-1.0-0.dll libepoxy-0.dll; do
  [[ -f "$MSYS2_ROOT/bin/$dll" ]] && cp -u "$MSYS2_ROOT/bin/$dll" "$STAGING_DIR/"
done

echo "[staging] $(find "$STAGING_DIR" -maxdepth 1 -name '*.dll' | wc -l) DLLs staged"

# ── 4. GDK-Pixbuf loaders (SANS loaders.cache !) ────────────────────
if [[ -d "$MSYS2_ROOT/lib/gdk-pixbuf-2.0/2.10.0/loaders" ]]; then
  cp "$MSYS2_ROOT/lib/gdk-pixbuf-2.0/2.10.0/loaders"/*.dll "$STAGING_DIR/lib/gdk-pixbuf-2.0/2.10.0/loaders/"
  echo "[staging] GDK-Pixbuf loaders copied"
fi

# ── 5. GLib schemas ──────────────────────────────────────────────────
cp "$MSYS2_ROOT/share/glib-2.0/schemas/"*.xml "$STAGING_DIR/share/glib-2.0/schemas/" 2>/dev/null || true
glib-compile-schemas.exe "$STAGING_DIR/share/glib-2.0/schemas/"
echo "[staging] GLib schemas compiled"

# ── 6. Thème Adwaita (filtré : uniquement CSS, PNG, SVG, index.theme) ──
if [[ -d "$MSYS2_ROOT/share/themes/Adwaita" ]]; then
  find "$MSYS2_ROOT/share/themes/Adwaita" -type f \
    \( -name "*.css" -o -name "*.png" -o -name "*.svg" -o -name "index.theme" \) \
    -exec cp --parents {} "$STAGING_DIR/share/themes/" \;
  echo "[staging] Adwaita theme staged (filtered)"
fi

# ── 7. Icônes Adwaita (tailles standard uniquement) ───────────────────
if [[ -d "$MSYS2_ROOT/share/icons/Adwaita" ]]; then
  for size in 16x16 22x22 24x24 32x32 48x48 96x96 256x256 scalable; do
    [[ -d "$MSYS2_ROOT/share/icons/Adwaita/$size" ]] || continue
    mkdir -p "$STAGING_DIR/share/icons/Adwaita/$size"
    cp -r "$MSYS2_ROOT/share/icons/Adwaita/$size/"* "$STAGING_DIR/share/icons/Adwaita/$size/" 2>/dev/null || true
  done
  cp "$MSYS2_ROOT/share/icons/Adwaita/index.theme" "$STAGING_DIR/share/icons/Adwaita/" 2>/dev/null || true
  echo "[staging] Adwaita icons staged (standard sizes only)"
fi

# ── 8. Migrations SQL ────────────────────────────────────────────────
if [[ -d "migrations" ]]; then
  cp migrations/*.sql "$STAGING_DIR/migrations/" 2>/dev/null || true
  echo "[staging] $(ls -1 "$STAGING_DIR/migrations"/*.sql 2>/dev/null | wc -l) migration(s) staged"
fi

# ── 9. Icône applicative ─────────────────────────────────────────────
ICON_SRC="assets/icons/hicolor/256x256/apps/heelonvault.png"
if [[ -f "$ICON_SRC" ]]; then
  magick "$ICON_SRC" -define icon:auto-resize=256,128,64,48,32,16 "$STAGING_DIR/heelonvault.ico"
  echo "[staging] Application icon generated"
else
  echo "[WARNING] Icon source not found: $ICON_SRC"
fi

# ── 10. Nettoyage final ──────────────────────────────────────────────
# Suppression systématique de tous les fichiers .cache (notamment loaders.cache)
find "$STAGING_DIR" -name "*.pdb" -delete
find "$STAGING_DIR" -name "*.a" -delete
find "$STAGING_DIR" -name "*.lib" -delete
find "$STAGING_DIR" -name "*.def" -delete
find "$STAGING_DIR" -name "*.cache" -delete

TOTAL_FILES=$(find "$STAGING_DIR" -type f | wc -l)
echo "[staging] Done. Total files: $TOTAL_FILES"
echo "[staging] IMPORTANT: loaders.cache has been explicitly excluded (dynamic scanning via GDK_PIXBUF_MODULEDIR)"
