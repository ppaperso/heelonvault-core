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

# User data and logs — same XDG layout as the native install (run.sh generated
# by install-core.sh), so both share one database. Without these exports,
# main.rs has no Linux platform default and falls back to <cwd>/data and
# <cwd>/logs, i.e. a location that depends on where the AppImage was launched.
# An explicit HEELONVAULT_DB_PATH / HEELONVAULT_LOG_DIR still wins.
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
APP_DATA_DIR="$DATA_HOME/heelonvault"
DB_PATH="${HEELONVAULT_DB_PATH:-$APP_DATA_DIR/heelonvault-rust.db}"
LOG_DIR="${HEELONVAULT_LOG_DIR:-$STATE_HOME/heelonvault/logs}"

umask 077
mkdir -p "$(dirname "$DB_PATH")" "$LOG_DIR"

# One-time recovery of a database created by AppImage 2.0.0, which wrote to
# <launch dir>/data/heelonvault-rust-dev.db. $OWD is the launch directory, set
# by the AppImage runtime. Copied, never moved: the legacy file may be a
# developer's dev database. The -wal file holds not-yet-checkpointed commits.
if [ -z "${HEELONVAULT_DB_PATH:-}" ] && [ ! -e "$DB_PATH" ]; then
    for legacy_dir in "${OWD:-}" "$HOME"; do
        LEGACY_DB="$legacy_dir/data/heelonvault-rust-dev.db"
        if [ -n "$legacy_dir" ] && [ -r "$LEGACY_DB" ]; then
            cp -n "$LEGACY_DB" "$DB_PATH" \
                && { [ ! -r "$LEGACY_DB-wal" ] || cp -n "$LEGACY_DB-wal" "$DB_PATH-wal"; } \
                && echo "[HeelonVault] Database copied from $LEGACY_DB to $DB_PATH" >&2
            break
        fi
    done
fi

export HEELONVAULT_DB_PATH="$DB_PATH"
export HEELONVAULT_LOG_DIR="$LOG_DIR"

exec "$APPDIR/usr/bin/heelonvault" "$@"
