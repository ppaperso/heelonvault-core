#!/usr/bin/env bash
# Unified installer entrypoint for HeelonVault.
# Detects the Linux family and dispatches to install-ubuntu.sh or install-rhel.sh.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
UBUNTU_SCRIPT="$SCRIPT_DIR/install-ubuntu.sh"
RHEL_SCRIPT="$SCRIPT_DIR/install-rhel.sh"

if [[ ! -f /etc/os-release ]]; then
  echo "[ERROR] /etc/os-release introuvable. Distribution non identifiable."
  exit 1
fi

. /etc/os-release

echo "[INFO] Distribution détectée : ${PRETTY_NAME:-$ID}"

is_debian_family=false
is_rhel_family=false

for tok in ${ID:-} ${ID_LIKE:-}; do
  case "$tok" in
    ubuntu|debian|linuxmint|pop|elementary|zorin|kali|raspbian|mx|lmde|devuan|pureos)
      is_debian_family=true
      ;;
    fedora|rhel|centos|rocky|almalinux|ol|scientific|eurolinux|nobara|ultramarine)
      is_rhel_family=true
      ;;
  esac
done

# Safety net: package-manager heuristic when os-release metadata is incomplete.
if [[ "$is_debian_family" == false && "$is_rhel_family" == false ]]; then
  if command -v apt-get >/dev/null 2>&1; then
    is_debian_family=true
  elif command -v dnf >/dev/null 2>&1 || command -v yum >/dev/null 2>&1; then
    is_rhel_family=true
  fi
fi

if [[ "$is_debian_family" == true && "$is_rhel_family" == false ]]; then
  if [[ ! -x "$UBUNTU_SCRIPT" ]]; then
    echo "[ERROR] Script manquant ou non exécutable: $UBUNTU_SCRIPT"
    exit 1
  fi
  echo "[INFO] Famille Debian/Ubuntu détectée -> exécution de install-ubuntu.sh"
  export HEELONVAULT_SUPPRESS_OS_LOG=1
  exec "$UBUNTU_SCRIPT" "$@"
fi

if [[ "$is_rhel_family" == true && "$is_debian_family" == false ]]; then
  if [[ ! -x "$RHEL_SCRIPT" ]]; then
    echo "[ERROR] Script manquant ou non exécutable: $RHEL_SCRIPT"
    exit 1
  fi
  echo "[INFO] Famille Fedora/RHEL détectée -> exécution de install-rhel.sh"
  export HEELONVAULT_SUPPRESS_OS_LOG=1
  exec "$RHEL_SCRIPT" "$@"
fi

if [[ "$is_debian_family" == true && "$is_rhel_family" == true ]]; then
  echo "[ERROR] Détection OS ambiguë (Debian et RHEL en même temps)."
  echo "[ERROR] Lancez explicitement l'un des scripts suivants :"
  echo "        sudo ./scripts/install-ubuntu.sh"
  echo "        sudo ./scripts/install-rhel.sh"
  exit 1
fi

echo "[ERROR] Distribution non supportée: ${PRETTY_NAME:-$ID}"
echo "[ERROR] Systèmes supportés : Ubuntu/Debian et Fedora/RHEL (Rocky/AlmaLinux/CentOS Stream)."
exit 1
