#!/usr/bin/env bash
# Shared smoke test for packaged installer artifacts.

set -euo pipefail

INSTALL_SCRIPT=""
REMOVE_SCRIPT=""
VERIFY_LOG_MARKERS=0
LOG_FILE="$HOME/.local/state/heelonvault/app.log"
SUDO="$(command -v sudo || echo "")"

if [[ -z "$SUDO" && "$(id -u)" -ne 0 ]]; then
  echo "[ERROR] sudo est requis quand le script n'est pas exécuté en root."
  exit 1
fi

run_root() {
  if [[ "$(id -u)" -eq 0 ]]; then
    "$@"
  else
    "$SUDO" "$@"
  fi
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --install)
      INSTALL_SCRIPT="$2"
      shift 2
      ;;
    --remove)
      REMOVE_SCRIPT="$2"
      shift 2
      ;;
    --check-log-markers)
      VERIFY_LOG_MARKERS=1
      shift
      ;;
    --log-file)
      LOG_FILE="$2"
      shift 2
      ;;
    *)
      echo "Usage: $0 [--install <path/to/install.sh>] [--remove <path/to/remove.sh>] [--check-log-markers] [--log-file <path/to/app.log>]"
      exit 1
      ;;
  esac
done

if [[ -z "$INSTALL_SCRIPT" && -z "$REMOVE_SCRIPT" && "$VERIFY_LOG_MARKERS" != "1" ]]; then
  echo "[ERROR] Au moins un argument est requis: --install <path/to/install.sh>, --remove <path/to/remove.sh> ou --check-log-markers"
  exit 1
fi

if [[ -n "$INSTALL_SCRIPT" ]]; then
  if [[ ! -x "$INSTALL_SCRIPT" ]]; then
    echo "[ERROR] Script d'installation introuvable ou non exécutable: $INSTALL_SCRIPT"
    exit 1
  fi

  run_root env \
    HEELONVAULT_NON_INTERACTIVE=1 \
    HEELONVAULT_DEPLOY_MODE=personal \
    HEELONVAULT_KEEP_LEGACY_DATA=1 \
    "$INSTALL_SCRIPT"

  test -x /opt/heelonvault/heelonvault
  test -x /opt/heelonvault/run.sh
  test -f /usr/share/applications/com.heelonvault.rust.desktop
  test -f /usr/share/applications/heelonvault.desktop
  test "$(stat -c '%a' /opt/heelonvault/data)" = "700"
  test "$(stat -c '%a' /opt/heelonvault/logs)" = "700"

  desktop-file-validate /usr/share/applications/com.heelonvault.rust.desktop
  desktop-file-validate /usr/share/applications/heelonvault.desktop

  grep -q '^Exec=/opt/heelonvault/run.sh$' /usr/share/applications/com.heelonvault.rust.desktop
  grep -q '^TryExec=/opt/heelonvault/run.sh$' /usr/share/applications/com.heelonvault.rust.desktop
  grep -q '^Exec=/opt/heelonvault/run.sh$' /usr/share/applications/heelonvault.desktop
fi

if [[ -n "$REMOVE_SCRIPT" ]]; then
  if [[ ! -x "$REMOVE_SCRIPT" ]]; then
    echo "[ERROR] Script de désinstallation introuvable ou non exécutable: $REMOVE_SCRIPT"
    exit 1
  fi

  run_root "$REMOVE_SCRIPT" --non-interactive --confirm --purge --purge-scope current

  test ! -d /opt/heelonvault
fi

if [[ "$VERIFY_LOG_MARKERS" == "1" ]]; then
  if [[ ! -f "$LOG_FILE" ]]; then
    echo "[ERROR] Fichier de log introuvable: $LOG_FILE"
    exit 1
  fi

  startup_ok=0
  if grep -Eq 'tokio runtime started|all services are initialized and ready' "$LOG_FILE"; then
    startup_ok=1
  fi

  logout_ok=0
  if grep -q 'main window logout completed' "$LOG_FILE"; then
    logout_ok=1
  fi

  shutdown_ok=0
  if grep -q 'application shutdown completed' "$LOG_FILE"; then
    shutdown_ok=1
  fi

  if [[ "$startup_ok" != "1" ]]; then
    echo "[ERROR] Marqueur startup manquant dans $LOG_FILE"
    exit 1
  fi

  if [[ "$logout_ok" != "1" ]]; then
    echo "[ERROR] Marqueur logout manquant dans $LOG_FILE"
    exit 1
  fi

  if [[ "$shutdown_ok" != "1" ]]; then
    echo "[ERROR] Marqueur shutdown manquant dans $LOG_FILE"
    exit 1
  fi

  echo "[OK] Marqueur startup détecté"
  echo "[OK] Marqueur logout détecté"
  echo "[OK] Marqueur shutdown détecté"
fi
