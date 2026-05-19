#!/usr/bin/env bash
# Shared uninstaller library for HeelonVault.
# This file is sourced by remove-ubuntu.sh and remove-rhel.sh.

hv_remove_init_common_vars() {
  HV_APP_ID="com.heelonvault.rust"
  HV_INSTALL_DIR="/opt/heelonvault"
  HV_SYSTEM_APPS_DIR="/usr/share/applications"
  HV_ICON_THEME_DIR="/usr/share/icons/hicolor"
  HV_BACKUP_DIR="/var/backups/heelonvault"
  HV_ENTERPRISE_DATA_DIR="/var/lib/heelonvault"
  HV_ENTERPRISE_LOG_DIR="/var/log/heelonvault"

  HV_INVOKING_USER="${SUDO_USER:-root}"
  HV_INVOKING_HOME="$(getent passwd "$HV_INVOKING_USER" | cut -d: -f6 2>/dev/null || true)"
  if [[ -z "$HV_INVOKING_HOME" ]]; then
    HV_INVOKING_HOME="/root"
  fi

  HV_NON_INTERACTIVE="${HEELONVAULT_NON_INTERACTIVE:-0}"
  HV_PURGE_SCOPE="current"
  HV_PURGE_DATA="n"
}

hv_remove_require_root() {
  if [[ "$(id -u)" -ne 0 ]]; then
    echo "[ERROR] Run with sudo."
    exit 1
  fi
}

hv_remove_confirm() {
  local confirm
  local purge_scope_choice
  local hard_confirm

  if [[ "$HV_NON_INTERACTIVE" == "1" ]]; then
    case "${HEELONVAULT_REMOVE_CONFIRM:-0}" in
      1|true|yes)
        confirm="o"
        ;;
      *)
        echo "[ERROR] Mode non interactif: confirmation manquante."
        echo "[ERROR] Définissez HEELONVAULT_REMOVE_CONFIRM=1 pour autoriser la désinstallation."
        exit 1
        ;;
    esac

    case "${HEELONVAULT_REMOVE_PURGE:-0}" in
      1|true|yes)
        HV_PURGE_DATA="o"
        ;;
      *)
        HV_PURGE_DATA="n"
        ;;
    esac
  fi

  echo ""
  echo "╔══════════════════════════════════════════════════════╗"
  echo "║          Désinstallation de HeelonVault             ║"
  echo "╚══════════════════════════════════════════════════════╝"
  echo ""
  echo "  Eléments système qui seront supprimés :"
  echo "  • $HV_INSTALL_DIR"
  echo "  • $HV_SYSTEM_APPS_DIR/$HV_APP_ID.desktop"
  echo "  • $HV_SYSTEM_APPS_DIR/heelonvault.desktop"
  echo "  • Icônes dans $HV_ICON_THEME_DIR/*/apps/heelonvault.png"
  echo "  • Icônes dans $HV_ICON_THEME_DIR/*/apps/$HV_APP_ID.png"
  echo ""
  echo "  Les données UTILISATEUR ($HOME/.local/share/heelonvault)"
  echo "  et les données ENTREPRISE ($HV_ENTERPRISE_DATA_DIR)"
  echo "  ainsi que les logs ENTREPRISE ($HV_ENTERPRISE_LOG_DIR)"
  echo "  et les backups ($HV_BACKUP_DIR) ne sont PAS supprimés par défaut."
  echo ""
  if [[ "$HV_NON_INTERACTIVE" == "1" ]]; then
    echo "  [auto] Suppression données + backups : ${HV_PURGE_DATA}"
    echo ""
    echo "  [auto] Confirmation désinstallation : ${confirm}"
  else
    read -rp "  Supprimer aussi les données utilisateur et backups ? [o/N] : " HV_PURGE_DATA
    echo ""
    read -rp "  Confirmer la désinstallation ? [o/N] : " confirm
  fi
  echo ""

  if [[ "${confirm,,}" != "o" ]]; then
    echo "[INFO] Désinstallation annulée."
    exit 0
  fi

  if [[ "${HV_PURGE_DATA,,}" == "o" ]]; then
    if [[ "$HV_NON_INTERACTIVE" == "1" ]]; then
      case "${HEELONVAULT_PURGE_SCOPE:-current}" in
        all)
          HV_PURGE_SCOPE="all"
          ;;
        *)
          HV_PURGE_SCOPE="current"
          ;;
      esac
      echo "[INFO] Mode non interactif: portée purge = $HV_PURGE_SCOPE"
    else
      echo ""
      echo "  Portée de purge des données utilisateur :"
      echo "  [1] Utilisateur courant uniquement (recommandé) [défaut]"
      echo "  [2] Tous les utilisateurs (UID >= 1000)"
      read -rp "  Votre choix [1/2] : " purge_scope_choice
      if [[ "${purge_scope_choice:-1}" == "2" ]]; then
        HV_PURGE_SCOPE="all"
        echo ""
        echo "  ⚠  Confirmation supplémentaire requise."
        read -rp "  Taper EXACTEMENT PURGE-ALL pour continuer : " hard_confirm
        if [[ "$hard_confirm" != "PURGE-ALL" ]]; then
          echo "[INFO] Confirmation invalide, retour à une purge utilisateur courant."
          HV_PURGE_SCOPE="current"
        fi
      fi
    fi
  fi
}

hv_remove_stop_running_app() {
  if pgrep -x heelonvault >/dev/null 2>&1; then
    echo "[INFO] Arrêt du processus heelonvault en cours..."
    pkill -x heelonvault 2>/dev/null || true
    sleep 1
  fi
}

hv_remove_desktop_integration() {
  local size

  echo "[INFO] Suppression des lanceurs bureau..."
  rm -f "$HV_SYSTEM_APPS_DIR/$HV_APP_ID.desktop"
  rm -f "$HV_SYSTEM_APPS_DIR/heelonvault.desktop"

  echo "[INFO] Suppression des icônes..."
  for size in 48x48 128x128 256x256; do
    rm -f "$HV_ICON_THEME_DIR/$size/apps/heelonvault.png"
    rm -f "$HV_ICON_THEME_DIR/$size/apps/$HV_APP_ID.png"
  done

  if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "$HV_ICON_THEME_DIR" 2>/dev/null || true
  fi

  if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$HV_SYSTEM_APPS_DIR" 2>/dev/null || true
  fi
}

hv_remove_install_dir() {
  echo "[INFO] Suppression de $HV_INSTALL_DIR..."
  rm -rf "$HV_INSTALL_DIR"
}

hv_remove_purge_data() {
  local username uid home_dir
  local app_data
  local app_state

  if [[ "${HV_PURGE_DATA,,}" != "o" ]]; then
    return
  fi

  if [[ "$HV_PURGE_SCOPE" == "all" ]]; then
    while IFS=: read -r username _ uid _ _ home_dir _; do
      if [[ "$uid" -ge 1000 && -d "$home_dir" ]]; then
        app_data="$home_dir/.local/share/heelonvault"
        app_state="$home_dir/.local/state/heelonvault"
        if [[ -d "$app_data" ]]; then
          echo "[INFO] Suppression données utilisateur : $app_data"
          rm -rf "$app_data"
        fi
        if [[ -d "$app_state" ]]; then
          echo "[INFO] Suppression état utilisateur : $app_state"
          rm -rf "$app_state"
        fi
      fi
    done < /etc/passwd
  else
    app_data="$HV_INVOKING_HOME/.local/share/heelonvault"
    app_state="$HV_INVOKING_HOME/.local/state/heelonvault"
    if [[ -d "$app_data" ]]; then
      echo "[INFO] Suppression données utilisateur courant : $app_data"
      rm -rf "$app_data"
    fi
    if [[ -d "$app_state" ]]; then
      echo "[INFO] Suppression état utilisateur courant : $app_state"
      rm -rf "$app_state"
    fi
  fi

  if [[ -d "$HV_BACKUP_DIR" ]]; then
    echo "[INFO] Suppression des backups : $HV_BACKUP_DIR"
    rm -rf "$HV_BACKUP_DIR"
  fi

  if [[ -d "$HV_ENTERPRISE_DATA_DIR" ]]; then
    echo "[INFO] Suppression données entreprise : $HV_ENTERPRISE_DATA_DIR"
    rm -rf "$HV_ENTERPRISE_DATA_DIR"
  fi

  if [[ -d "$HV_ENTERPRISE_LOG_DIR" ]]; then
    echo "[INFO] Suppression logs entreprise : $HV_ENTERPRISE_LOG_DIR"
    rm -rf "$HV_ENTERPRISE_LOG_DIR"
  fi
}

hv_remove_print_summary() {
  echo ""
  echo "╔══════════════════════════════════════════════════════╗"
  echo "║          Désinstallation terminée                   ║"
  echo "╚══════════════════════════════════════════════════════╝"
  echo "[OK] Binaire et fichiers système supprimés"
  echo "[OK] Lanceurs bureau supprimés"
  echo "[OK] Icônes système supprimées"
  if [[ "${HV_PURGE_DATA,,}" == "o" ]]; then
    echo "[OK] Données utilisateur/entreprise et backups supprimés"
  else
    echo "[INFO] Données utilisateur conservées : ~/.local/share/heelonvault"
    echo "[INFO] Données entreprise conservées : $HV_ENTERPRISE_DATA_DIR"
    echo "[INFO] Logs entreprise conservés     : $HV_ENTERPRISE_LOG_DIR"
    echo "[INFO] Backups conservés               : $HV_BACKUP_DIR"
  fi
  echo ""
  echo "  HeelonVault a été désinstallé proprement."
  echo ""
}

hv_remove_run_common_flow() {
  hv_remove_confirm
  hv_remove_stop_running_app
  hv_remove_desktop_integration
  hv_remove_install_dir
  hv_remove_purge_data
  hv_remove_print_summary
}
