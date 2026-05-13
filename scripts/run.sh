#!/usr/bin/env bash
# Production launcher (Rust-only)

set -euo pipefail

APP_DIR="/opt/heelonvault"
BIN_PATH="${APP_DIR}/target/release/heelonvault"
PROD_DB_DIR="/var/lib/heelonvault-rust-shared"
PROD_DB_PATH="${PROD_DB_DIR}/heelonvault.db"
PROD_LOG_DIR="${PROD_DB_DIR}/logs"

if [[ ! -x "$BIN_PATH" ]]; then
  echo "[ERROR] Rust binary not found: $BIN_PATH"
  echo "Build it with: cd ${APP_DIR} && cargo build --release"
  exit 1
fi

mkdir -p "$PROD_DB_DIR"
export HEELONVAULT_DB_PATH="$PROD_DB_PATH"
export HEELONVAULT_LOG_DIR="$PROD_LOG_DIR"
export HEELONVAULT_LOG_LEVEL="info"

exec "$BIN_PATH"
