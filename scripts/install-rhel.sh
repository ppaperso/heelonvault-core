#!/usr/bin/env bash
# Installer for HeelonVault - Fedora/RHEL wrapper

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CORE_LIB="$SCRIPT_DIR/install-core.sh"

if [[ ! -f "$CORE_LIB" ]]; then
  echo "[ERROR] Bibliothèque d'installation introuvable: $CORE_LIB"
  exit 1
fi

# shellcheck source=install-core.sh
source "$CORE_LIB"

hv_install_runtime_dependencies() {
  echo "[INFO] Vérification des dépendances runtime..."
  # Correspondances des paquets RHEL/Fedora :
  #   desktop-file-utils   -> desktop-file-utils
  #   libgtk-4-1           -> gtk4
  #   libadwaita-1-0       -> libadwaita
  #   libsqlite3-0         -> sqlite
  #   libglib2.0-0         -> glib2
  "$HV_PKG_MGR" install -y \
    desktop-file-utils \
    gtk4 \
    libadwaita \
    sqlite \
    glib2
}

hv_post_install_os_specific() {
  # Sur RHEL/Fedora, SELinux peut bloquer l'exécution depuis /opt.
  if command -v chcon >/dev/null 2>&1 && command -v getenforce >/dev/null 2>&1; then
    if [[ "$(getenforce 2>/dev/null)" != "Disabled" ]]; then
      echo "[INFO] SELinux actif : application du contexte bin_t sur les exécutables..."
      chcon -t bin_t "$HV_INSTALL_DIR/heelonvault" 2>/dev/null || true
      chcon -t bin_t "$HV_INSTALL_DIR/run.sh" 2>/dev/null || true
    fi
  fi
}

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
    echo "[ERROR] install-rhel.sh est réservé aux distributions Fedora / RHEL / Rocky Linux / AlmaLinux."
    echo "[ERROR] Pour Ubuntu / Debian et dérivés, utilisez : sudo ./scripts/install-ubuntu.sh"
    exit 1
  fi

  if command -v dnf >/dev/null 2>&1; then
    HV_PKG_MGR="dnf"
  elif command -v yum >/dev/null 2>&1; then
    HV_PKG_MGR="yum"
  else
    echo "[ERROR] Ni dnf ni yum trouvé sur ce système ${PRETTY_NAME:-$ID}."
    exit 1
  fi

  case "${ID:-}" in
    rhel|centos|rocky|almalinux|ol)
      if [[ "${VERSION_ID%%.*}" -lt 9 ]] 2>/dev/null; then
        echo "[ERROR] Version ${VERSION_ID} détectée. GTK4 et libadwaita nécessitent la version 9+."
        echo "[ERROR] Installation interrompue pour éviter un déploiement cassé."
        exit 1
      fi
      ;;
  esac
}

hv_main() {
  hv_init_common_vars "$SCRIPT_DIR"
  hv_require_root
  hv_validate_rhel_os
  hv_run_common_install_flow
}

hv_main "$@"
