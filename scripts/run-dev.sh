#!/usr/bin/env bash
# Development launcher (Rust-only)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
DEV_DB_DIR="${REPO_ROOT}/data"
DEV_DB_PATH="${DEV_DB_DIR}/heelonvault-rust-dev.db"
DEV_LOG_DIR="${REPO_ROOT}/logs"

if [[ ! -f "${REPO_ROOT}/Cargo.toml" ]]; then
  echo "[ERROR] Cargo.toml not found at repository root: ${REPO_ROOT}/Cargo.toml"
  exit 1
fi

mkdir -p "$DEV_DB_DIR"
export HEELONVAULT_DB_PATH="$DEV_DB_PATH"
export HEELONVAULT_LOG_DIR="$DEV_LOG_DIR"
export HEELONVAULT_LOG_LEVEL="debug"

cd "$REPO_ROOT"
exec cargo run
