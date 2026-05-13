#!/usr/bin/env bash
# Uninstaller for HeelonVault - Fedora/RHEL wrapper

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CORE_LIB="$SCRIPT_DIR/remove-core.sh"

if [[ ! -f "$CORE_LIB" ]]; then
  echo "[ERROR] Bibliothèque de désinstallation introuvable: $CORE_LIB"
  exit 1
fi

# shellcheck source=remove-core.sh
source "$CORE_LIB"

hv_validate_rhel_os() {
  if [[ ! -f /etc/os-release ]]; then
    echo "[ERROR] /etc/os-release introuvable. Distribution non identifiable."
    exit 1
  fi

  # shellcheck source=/etc/os-release
  . /etc/os-release
  if [[ "${HEELONVAULT_SUPPRESS_OS_LOG:-0}" != "1" ]]; then
    echo "[INFO] Distribution détectée : ${PRETTY_NAME:-$ID}"
  fi

  local os_ok=false
  for tok in ${ID:-} ${ID_LIKE:-}; do
    case "$tok" in
      fedora|rhel|centos|rocky|almalinux|ol|scientific|eurolinux|nobara|ultramarine)
        os_ok=true
        break
        ;;
    esac
  done

  if [[ "$os_ok" != true ]]; then
    echo "[ERROR] Distribution incompatible : ${PRETTY_NAME:-$ID}"
    echo "[ERROR] remove-rhel.sh est réservé aux distributions Fedora / RHEL / Rocky Linux / AlmaLinux."
    echo "[ERROR] Pour Ubuntu / Debian et dérivés, utilisez : sudo ./scripts/remove-ubuntu.sh"
    exit 1
  fi
}

hv_main() {
  hv_remove_init_common_vars
  hv_remove_require_root
  hv_validate_rhel_os
  hv_remove_run_common_flow
}

hv_main "$@"
