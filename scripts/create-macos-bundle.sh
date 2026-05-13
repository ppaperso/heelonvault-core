#!/usr/bin/env bash
# create-macos-bundle.sh — Build HeelonVault.app and package it into a .dmg
#
# Usage:
#   bash scripts/create-macos-bundle.sh \
#       --binary  target/release/heelonvault \
#       --version 1.1.0 \
#       --out     heelonvault-macos-arm64.dmg
#
# Required tools (install via Homebrew):
#   dylibbundler  — harvests and rewrites dylib references
#   create-dmg    — builds a polished, drag-to-Applications DMG
#
# sips and iconutil are macOS built-ins and need no installation.

set -euo pipefail

# ---------------------------------------------------------------------------
# Argument parsing
# ---------------------------------------------------------------------------
BINARY=""
VERSION=""
OUT_DMG=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --binary)  BINARY="$2";  shift 2 ;;
        --version) VERSION="$2"; shift 2 ;;
        --out)     OUT_DMG="$2"; shift 2 ;;
        *) echo "[ERROR] Unknown argument: $1" >&2; exit 1 ;;
    esac
done

if [[ -z "$BINARY" || -z "$OUT_DMG" ]]; then
    echo "[ERROR] --binary and --out are required" >&2
    exit 1
fi

if [[ ! -x "$BINARY" ]]; then
    echo "[ERROR] Binary not found or not executable: $BINARY" >&2
    exit 1
fi

# Derive version from the binary itself if not supplied
if [[ -z "$VERSION" ]]; then
    VERSION="$("$BINARY" --version 2>/dev/null || true)"
    VERSION="${VERSION:-0.0.0}"
fi

echo "[INFO] Building HeelonVault.app  version=${VERSION}"

# ---------------------------------------------------------------------------
# Paths
# ---------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

APP_DIR="HeelonVault.app"
CONTENTS="${APP_DIR}/Contents"
MACOS_DIR="${CONTENTS}/MacOS"
FRAMEWORKS_DIR="${CONTENTS}/Frameworks"
RESOURCES_DIR="${CONTENTS}/Resources"

ICON_SRC="${REPO_ROOT}/assets/icons/hicolor/256x256/apps/heelonvault.png"
ICONSET_DIR="${RESOURCES_DIR}/heelonvault.iconset"
ICNS_OUT="${RESOURCES_DIR}/heelonvault.icns"

# GLib/GDK-Pixbuf paths as installed by Homebrew on arm64 macOS
HOMEBREW_PREFIX="${HOMEBREW_PREFIX:-/opt/homebrew}"
GLIB_SCHEMA_SRC="${HOMEBREW_PREFIX}/share/glib-2.0/schemas"
PIXBUF_LOADERS_DIR="${HOMEBREW_PREFIX}/lib/gdk-pixbuf-2.0"

# ---------------------------------------------------------------------------
# 1. Clean previous bundle
# ---------------------------------------------------------------------------
echo "[1/11] Cleaning previous bundle..."
rm -rf "${APP_DIR}"

# ---------------------------------------------------------------------------
# 2. Create bundle skeleton
# ---------------------------------------------------------------------------
echo "[2/11] Creating .app skeleton..."
mkdir -p "${MACOS_DIR}" "${FRAMEWORKS_DIR}" "${RESOURCES_DIR}"

# ---------------------------------------------------------------------------
# 3. Copy binary as heelonvault-bin (the real executable)
#    dylibbundler MUST run on this copy — never on target/release/heelonvault
# ---------------------------------------------------------------------------
echo "[3/11] Copying binary..."
cp "${BINARY}" "${MACOS_DIR}/heelonvault-bin"
chmod +x "${MACOS_DIR}/heelonvault-bin"

# ---------------------------------------------------------------------------
# 4. Create launcher wrapper script
#    Sets GTK4 runtime env vars before exec-ing the real binary.
# ---------------------------------------------------------------------------
echo "[4/11] Writing launcher wrapper..."
cat > "${MACOS_DIR}/heelonvault" <<'WRAPPER'
#!/usr/bin/env bash
# HeelonVault macOS launcher — sets GTK4 runtime paths, then exec real binary
BUNDLE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RESOURCES="${BUNDLE_DIR}/../Resources"
FRAMEWORKS="${BUNDLE_DIR}/../Frameworks"

export GSETTINGS_SCHEMA_DIR="${RESOURCES}/share/glib-2.0/schemas"
export GDK_PIXBUF_MODULE_FILE="${RESOURCES}/lib/gdk-pixbuf-2.0/2.10.0/loaders.cache"
export XDG_DATA_DIRS="${RESOURCES}/share:${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"

exec "${BUNDLE_DIR}/heelonvault-bin" "$@"
WRAPPER
chmod +x "${MACOS_DIR}/heelonvault"

# ---------------------------------------------------------------------------
# 5. Patch and bundle dylibs (on the COPY, not the source binary)
# ---------------------------------------------------------------------------
echo "[5/11] Bundling dylibs with dylibbundler..."
dylibbundler \
    --bundle-deps \
    --fix-file "${MACOS_DIR}/heelonvault-bin" \
    --dest-dir "${FRAMEWORKS_DIR}" \
    --install-path "@executable_path/../Frameworks/" \
    --overwrite-dir \
    --no-codesign

# ---------------------------------------------------------------------------
# 6. GLib compiled schemas
# ---------------------------------------------------------------------------
echo "[6/11] Staging GLib schemas..."
SCHEMA_DST="${RESOURCES_DIR}/share/glib-2.0/schemas"
mkdir -p "${SCHEMA_DST}"

if [[ -d "${GLIB_SCHEMA_SRC}" ]]; then
    cp "${GLIB_SCHEMA_SRC}/"*.gschema.xml "${SCHEMA_DST}/" 2>/dev/null || true
    glib-compile-schemas "${SCHEMA_DST}"
else
    echo "[WARN] GLib schema source not found at ${GLIB_SCHEMA_SRC}, skipping"
fi

# ---------------------------------------------------------------------------
# 7. GDK-Pixbuf loaders
# ---------------------------------------------------------------------------
echo "[7/11] Staging GDK-Pixbuf loaders..."
PIXBUF_DST_ROOT="${RESOURCES_DIR}/lib/gdk-pixbuf-2.0"

if [[ -d "${PIXBUF_LOADERS_DIR}" ]]; then
    mkdir -p "${PIXBUF_DST_ROOT}"
    # Copy the tree contents and resolve Homebrew symlinks so bundle paths are self-contained.
    cp -RL "${PIXBUF_LOADERS_DIR}/." "${PIXBUF_DST_ROOT}/"
    # Regenerate loaders.cache with bundle-relative paths
    GDK_PIXBUF_MODULEDIR="${PIXBUF_DST_ROOT}/2.10.0/loaders" \
        gdk-pixbuf-query-loaders > "${PIXBUF_DST_ROOT}/2.10.0/loaders.cache"
else
    echo "[WARN] GDK-Pixbuf loaders not found at ${PIXBUF_LOADERS_DIR}, skipping"
fi

# ---------------------------------------------------------------------------
# 8. App icon: PNG → .iconset → .icns
# ---------------------------------------------------------------------------
echo "[8/11] Generating .icns icon..."
if [[ -f "${ICON_SRC}" ]]; then
    mkdir -p "${ICONSET_DIR}"
    for SIZE in 16 32 64 128 256 512; do
        sips -z "${SIZE}" "${SIZE}" "${ICON_SRC}" \
            --out "${ICONSET_DIR}/icon_${SIZE}x${SIZE}.png" > /dev/null
        DOUBLE=$((SIZE * 2))
        sips -z "${DOUBLE}" "${DOUBLE}" "${ICON_SRC}" \
            --out "${ICONSET_DIR}/icon_${SIZE}x${SIZE}@2x.png" > /dev/null
    done
    iconutil --convert icns "${ICONSET_DIR}" --output "${ICNS_OUT}"
    rm -rf "${ICONSET_DIR}"
else
    echo "[WARN] Icon source not found at ${ICON_SRC}, skipping"
fi

# ---------------------------------------------------------------------------
# 9. Inject Info.plist with real version
# ---------------------------------------------------------------------------
echo "[9/11] Writing Info.plist (version=${VERSION})..."
sed "s/@@VERSION@@/${VERSION}/g" \
    "${REPO_ROOT}/macos/Info.plist" > "${CONTENTS}/Info.plist"

# ---------------------------------------------------------------------------
# 10. Ad-hoc code signature (no Apple Developer ID required)
# ---------------------------------------------------------------------------
echo "[10/11] Ad-hoc signing..."
codesign --force --deep --sign - "${APP_DIR}"

# ---------------------------------------------------------------------------
# 11. Build DMG
# ---------------------------------------------------------------------------
echo "[11/11] Creating ${OUT_DMG}..."
if command -v create-dmg > /dev/null 2>&1; then
    create-dmg \
        --volname "HeelonVault ${VERSION}" \
        --window-size 600 400 \
        --icon-size 100 \
        --icon "HeelonVault.app" 150 200 \
        --app-drop-link 450 200 \
        --no-internet-enable \
        "${OUT_DMG}" \
        "${APP_DIR}"
else
    echo "[WARN] create-dmg not found, falling back to hdiutil"
    STAGING_DIR="$(mktemp -d)"
    cp -R "${APP_DIR}" "${STAGING_DIR}/"
    hdiutil create \
        -volname "HeelonVault ${VERSION}" \
        -srcfolder "${STAGING_DIR}" \
        -ov -format UDZO \
        "${OUT_DMG}"
    rm -rf "${STAGING_DIR}"
fi

echo "[OK] Bundle created: ${OUT_DMG}"
