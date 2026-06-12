#!/bin/bash
# HeelonVault macOS bundle launcher
# Configures the runtime environment before exec-ing the Rust binary.
# This script is CFBundleExecutable in Info.plist and is invoked by macOS.
set -euo pipefail

BUNDLE_DIR="$(cd "$(dirname "$0")/.." && pwd)"

# Migrations — absolute path inside the bundle Resources/.
# The Rust binary reads HEELONVAULT_MIGRATIONS_DIR in priority over
# the current_exe()-relative fallback (which would resolve to MacOS/migrations/).
export HEELONVAULT_MIGRATIONS_DIR="$BUNDLE_DIR/Resources/migrations"

# GLib schemas compiled at bundle build time.
export GSETTINGS_SCHEMA_DIR="$BUNDLE_DIR/Resources/share/glib-2.0/schemas"

# XDG data dirs: bundle Resources first, then system Homebrew fallback.
export XDG_DATA_DIRS="$BUNDLE_DIR/Resources/share:${XDG_DATA_DIRS:-/opt/homebrew/share:/usr/local/share}"

# GIO modules (Homebrew-bundled).
export GIO_MODULE_DIR="$BUNDLE_DIR/Resources/lib/gio/modules"

# gdk-pixbuf loaders cache — only export if the file was bundled.
# If absent the app still starts but PNG/SVG images may not render.
if [ -f "$BUNDLE_DIR/Resources/lib/gdk-pixbuf-2.0/2.10.0/loaders.cache" ]; then
    export GDK_PIXBUF_MODULE_FILE="$BUNDLE_DIR/Resources/lib/gdk-pixbuf-2.0/2.10.0/loaders.cache"
fi

# GSK renderer — prefer the caller's override, fall back to GL.
# Metal support in GTK4/macOS is experimental; GL is stable on Apple Silicon.
export GSK_RENDERER="${GSK_RENDERER:-gl}"

exec "$BUNDLE_DIR/MacOS/heelonvault-bin" "$@"
