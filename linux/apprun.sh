#!/bin/bash
# HeelonVault Linux AppImage launcher (AppRun)
# Invoked by the AppImage runtime — $APPDIR is set by the runtime.
# Configures the GTK4/GLib runtime environment then exec-s the Rust binary.

# APPDIR is set by the AppImage runtime; fall back to script location for testing.
SELF="$(readlink -f "$0")"
HERE="${SELF%/*}"
export APPDIR="${APPDIR:-$HERE}"

# Migrations — absolute path inside the AppImage.
# main.rs reads HEELONVAULT_MIGRATIONS_DIR in priority over the exe-sibling fallback.
export HEELONVAULT_MIGRATIONS_DIR="$APPDIR/usr/share/heelonvault/migrations"

# GLib schemas compiled at AppImage build time.
export GSETTINGS_SCHEMA_DIR="$APPDIR/usr/share/glib-2.0/schemas"

# Bundled libraries — prepend to LD_LIBRARY_PATH.
export LD_LIBRARY_PATH="$APPDIR/usr/lib:${LD_LIBRARY_PATH:-}"

# XDG data dirs: AppImage-internal data first, then system fallback.
export XDG_DATA_DIRS="$APPDIR/usr/share:${XDG_DATA_DIRS:-/usr/local/share:/usr/share}"

# GIO modules (Adwaita / GSettings backend).
if [ -d "$APPDIR/usr/lib/gio/modules" ]; then
    export GIO_MODULE_DIR="$APPDIR/usr/lib/gio/modules"
fi

# gdk-pixbuf loaders cache — only export if bundled.
if [ -f "$APPDIR/usr/lib/gdk-pixbuf-2.0/2.10.0/loaders.cache" ]; then
    export GDK_PIXBUF_MODULE_FILE="$APPDIR/usr/lib/gdk-pixbuf-2.0/2.10.0/loaders.cache"
fi

exec "$APPDIR/usr/bin/heelonvault" "$@"
