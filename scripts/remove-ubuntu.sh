#!/usr/bin/env bash
# Uninstaller for HeelonVault - Ubuntu/Debian wrapper

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CORE_LIB="$SCRIPT_DIR/remove-core.sh"

if [[ ! -f "$CORE_LIB" ]]; then
  echo "[ERROR] Bibliothèque de désinstallation introuvable: $CORE_LIB"
  exit 1
fi

# shellcheck source=remove-core.sh
source "$CORE_LIB"

hv_validate_ubuntu_os() {
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
      ubuntu|debian|linuxmint|pop|elementary|zorin|kali|raspbian|mx|lmde|devuan|pureos)
        os_ok=true
        break
        ;;
    esac
  done

  if [[ "$os_ok" != true ]]; then
    echo "[ERROR] Distribution incompatible : ${PRETTY_NAME:-$ID}"
    echo "[ERROR] remove-ubuntu.sh est réservé aux distributions Ubuntu / Debian et dérivés."
    echo "[ERROR] Pour Fedora / RHEL / Rocky Linux / AlmaLinux, utilisez : sudo ./scripts/remove-rhel.sh"
    exit 1
  fi
}

hv_main() {
  hv_remove_init_common_vars
  hv_remove_require_root
  hv_validate_ubuntu_os
  hv_remove_run_common_flow
}

hv_main "$@"
