#!/usr/bin/env bash
# Shared installer library for HeelonVault.
# This file is sourced by install-ubuntu.sh and install-rhel.sh.

hv_init_common_vars() {
  local script_dir="$1"

  HV_APP_NAME="HeelonVault"
  HV_APP_ID="com.heelonvault.rust"
  HV_INSTALL_DIR="/opt/heelonvault"
  # Mise à jour transactionnelle : la nouvelle version est préparée dans
  # HV_STAGE_DIR, puis échangée avec l'installation courante, conservée dans
  # HV_PREVIOUS_DIR jusqu'au succès complet (restaurée en cas d'échec).
  HV_STAGE_DIR="$HV_INSTALL_DIR.new"
  HV_PREVIOUS_DIR="$HV_INSTALL_DIR.previous"
  HV_VERSION_FILE="VERSION"
  HV_DATA_DIR="$HV_INSTALL_DIR/data"
  HV_DB_FILE="$HV_DATA_DIR/heelonvault-rust-dev.db"
  HV_BACKUP_DIR="/var/backups/heelonvault"
  HV_SYSTEM_APPS_DIR="/usr/share/applications"
  HV_DESKTOP_FILE="$HV_APP_ID.desktop"
  HV_LEGACY_DESKTOP_FILE="heelonvault.desktop"
  HV_DESKTOP_PATH="$HV_SYSTEM_APPS_DIR/$HV_DESKTOP_FILE"
  HV_LEGACY_DESKTOP_PATH="$HV_SYSTEM_APPS_DIR/$HV_LEGACY_DESKTOP_FILE"
  HV_ICON_THEME_DIR="/usr/share/icons/hicolor"
  HV_LOCAL_ICON_DIR="$HV_INSTALL_DIR/icons"
  HV_LOCAL_ICON_PATH="$HV_LOCAL_ICON_DIR/heelonvault.png"
  HV_SCRIPT_DIR="$script_dir"
  HV_ROOT_DIR="$(dirname "$HV_SCRIPT_DIR")"
  # HV_ASSETS_SOURCE / HV_PRIMARY_ICON_SOURCE are resolved in hv_verify_core_files
  # (root/script-dir/crate-local fallback, same pattern as migrations).
  HV_CHECKSUM_FILE="$HV_SCRIPT_DIR/heelonvault.sha256"

  HV_INVOKING_USER="${SUDO_USER:-root}"
  HV_INVOKING_HOME="$(getent passwd "$HV_INVOKING_USER" | cut -d: -f6 2>/dev/null || true)"
  if [[ -z "$HV_INVOKING_HOME" ]]; then
    HV_INVOKING_HOME="/root"
  fi

  HV_USER_DB_FILE="$HV_INVOKING_HOME/.local/share/heelonvault/heelonvault-rust.db"
  HV_ENTERPRISE_DATA_DIR="/var/lib/heelonvault"
  HV_ENTERPRISE_DB_FILE="$HV_ENTERPRISE_DATA_DIR/heelonvault-rust.db"
  HV_ENTERPRISE_LOG_DIR="/var/log/heelonvault"

  HV_DEPLOY_MODE="personal"
  HV_PREVIOUS_DEPLOY_MODE=""
  HV_PREVIOUS_VERSION=""
  HV_NEW_VERSION=""
  HV_SWAPPED=false
  HV_TRANSACTION_OPEN=false
  HV_ALLOW_STALE_BINARY="${HEELONVAULT_ALLOW_STALE_BINARY:-0}"
  HV_NON_INTERACTIVE="${HEELONVAULT_NON_INTERACTIVE:-0}"
  HV_BACKUP_KEEP="${HEELONVAULT_BACKUP_KEEP:-20}"
  HV_DRY_RUN="${HEELONVAULT_DRY_RUN:-0}"

  HV_FRESH_INSTALL=true
  HV_KEEP_DATA=true
  HV_HAS_LEGACY_DB=false
  HV_HAS_ACTIVE_DB=false
  HV_ACTIVE_DB_FILE="$HV_USER_DB_FILE"
  HV_ACTIVE_DB_LABEL="utilisateur ($HV_INVOKING_USER)"
}

hv_rotate_backups() {
  local pattern="$1"
  local keep="$2"
  local files=()
  local old_file

  [[ "$keep" =~ ^[0-9]+$ ]] || keep=20
  if [[ "$keep" -lt 1 ]]; then
    keep=1
  fi

  mapfile -t files < <(ls -1t $pattern 2>/dev/null || true)
  if [[ "${#files[@]}" -le "$keep" ]]; then
    return
  fi

  for old_file in "${files[@]:$keep}"; do
    rm -f "$old_file"
  done
}

hv_require_root() {
  if [[ "$(id -u)" -ne 0 ]]; then
    echo "[ERROR] Run with sudo."
    exit 1
  fi
}

hv_verify_core_files() {
  local binary_location
  local desktop_location
  local migrations_location

  # Binaire : le plus récent parmi target/release (build cargo), la racine du
  # dépôt et le dossier des scripts (archive). Un binaire oublié à la racine ne
  # doit pas masquer un build plus récent.
  # HEELONVAULT_BINARY impose un binaire précis (build hors du dépôt, par
  # exemple avec CARGO_TARGET_DIR, que sudo ne transmet pas).
  local candidate
  binary_location=""
  if [[ -n "${HEELONVAULT_BINARY:-}" ]]; then
    if [[ ! -f "$HEELONVAULT_BINARY" ]]; then
      echo "[ERROR] HEELONVAULT_BINARY=$HEELONVAULT_BINARY : fichier introuvable"
      exit 1
    fi
    binary_location="$HEELONVAULT_BINARY"
  fi
  for candidate in "$HV_ROOT_DIR/target/release/heelonvault" "$HV_ROOT_DIR/heelonvault" "$HV_SCRIPT_DIR/heelonvault"; do
    [[ -z "${HEELONVAULT_BINARY:-}" ]] || break
    if [[ -f "$candidate" ]] && { [[ -z "$binary_location" ]] || [[ "$candidate" -nt "$binary_location" ]]; }; then
      binary_location="$candidate"
    fi
  done
  if [[ -z "$binary_location" ]]; then
    echo "[ERROR] Binaire 'heelonvault' introuvable (target/release, $HV_ROOT_DIR, $HV_SCRIPT_DIR)"
    echo "[ERROR] Construisez-le d'abord : cargo build --release -p heelonvault-app"
    exit 1
  fi

  # Chercher le desktop file: d'abord dans ROOT_DIR, puis dans SCRIPT_DIR
  if [[ -f "$HV_ROOT_DIR/heelonvault.desktop" ]]; then
    desktop_location="$HV_ROOT_DIR/heelonvault.desktop"
  elif [[ -f "$HV_SCRIPT_DIR/heelonvault.desktop" ]]; then
    desktop_location="$HV_SCRIPT_DIR/heelonvault.desktop"
  else
    echo "[ERROR] Fichier desktop 'heelonvault.desktop' introuvable dans $HV_ROOT_DIR ou $HV_SCRIPT_DIR"
    exit 1
  fi

  # Chercher migrations: d'abord dans ROOT_DIR, puis dans SCRIPT_DIR, puis dans
  # l'emplacement crate-local post-restructuration (crates/heelonvault-app/migrations)
  if [[ -d "$HV_ROOT_DIR/migrations" ]]; then
    migrations_location="$HV_ROOT_DIR/migrations"
  elif [[ -d "$HV_SCRIPT_DIR/migrations" ]]; then
    migrations_location="$HV_SCRIPT_DIR/migrations"
  elif [[ -d "$HV_ROOT_DIR/crates/heelonvault-app/migrations" ]]; then
    migrations_location="$HV_ROOT_DIR/crates/heelonvault-app/migrations"
  else
    echo "[ERROR] Dossier 'migrations' introuvable dans $HV_ROOT_DIR, $HV_SCRIPT_DIR ou $HV_ROOT_DIR/crates/heelonvault-app/migrations"
    exit 1
  fi

  # Chercher assets/icônes: d'abord dans ROOT_DIR, puis dans SCRIPT_DIR, puis dans
  # l'emplacement crate-local post-restructuration (crates/heelonvault-app/assets)
  local assets_location
  if [[ -d "$HV_ROOT_DIR/assets" ]]; then
    assets_location="$HV_ROOT_DIR/assets"
  elif [[ -d "$HV_SCRIPT_DIR/assets" ]]; then
    assets_location="$HV_SCRIPT_DIR/assets"
  elif [[ -d "$HV_ROOT_DIR/crates/heelonvault-app/assets" ]]; then
    assets_location="$HV_ROOT_DIR/crates/heelonvault-app/assets"
  else
    echo "[ERROR] Dossier 'assets' introuvable dans $HV_ROOT_DIR, $HV_SCRIPT_DIR ou $HV_ROOT_DIR/crates/heelonvault-app/assets"
    exit 1
  fi

  HV_PRIMARY_ICON_SOURCE="$assets_location/icons/hicolor/256x256/apps/heelonvault.png"
  if [[ ! -f "$HV_PRIMARY_ICON_SOURCE" ]]; then
    echo "[ERROR] Icône principale introuvable : $HV_PRIMARY_ICON_SOURCE"
    exit 1
  fi

  # Stocker les emplacements réels pour la suite
  HV_BINARY_SOURCE="$binary_location"
  HV_DESKTOP_SOURCE="$desktop_location"
  HV_MIGRATIONS_SOURCE="$migrations_location"
  HV_ASSETS_SOURCE="$assets_location"
}

# Version déclarée par les sources (crates/heelonvault-app/Cargo.toml), si présentes.
hv_source_version() {
  local manifest="$HV_ROOT_DIR/crates/heelonvault-app/Cargo.toml"
  [[ -f "$manifest" ]] || return 0
  sed -n 's/^version = "\(.*\)"/\1/p' "$manifest" | head -1
}

# Version annoncée par le binaire (`heelonvault --version`). Exécuté dans un
# bac à sable (HOME, dossier courant, base et logs temporaires, sans affichage)
# avec un délai : un binaire antérieur à `--version` démarrerait sinon
# l'application, et ne doit rien toucher sur le système.
hv_binary_version() {
  local binary="$1"
  local sandbox
  local output
  sandbox="$(mktemp -d)"
  output="$(
    cd "$sandbox" &&
      env -i PATH="/usr/bin:/bin" HOME="$sandbox" \
        HEELONVAULT_DB_PATH="$sandbox/probe.db" HEELONVAULT_LOG_DIR="$sandbox/logs" \
        timeout 10 "$binary" --version 2>/dev/null
  )" || true
  rm -rf "$sandbox"
  sed -n 's/^HeelonVault \([0-9][0-9A-Za-z.+-]*\)$/\1/p' <<< "$output" | head -1
}

# Refuse un binaire plus ancien que le dernier commit du dépôt ou dont la
# version diffère des sources : sinon une « mise à jour » réinstallerait
# silencieusement un ancien build.
hv_check_binary_freshness() {
  local source_version
  local stale_reason=""

  source_version="$(hv_source_version)"
  HV_NEW_VERSION="$(hv_binary_version "$HV_BINARY_SOURCE")"

  echo "[INFO] Binaire à installer : $HV_BINARY_SOURCE"
  echo "[INFO] Version du binaire  : ${HV_NEW_VERSION:-inconnue (binaire sans --version)}"

  if [[ -n "$source_version" ]]; then
    echo "[INFO] Version des sources : $source_version"
    if [[ "$HV_NEW_VERSION" != "$source_version" ]]; then
      stale_reason="la version du binaire (${HV_NEW_VERSION:-inconnue}) diffère de celle des sources ($source_version)"
    fi
  fi

  if [[ -z "$stale_reason" ]]; then
    # Même critère que cargo : un fichier compilé dans le binaire (sources,
    # manifestes, build.rs, assets et traductions embarqués, lockfile, sources
    # premium voisines) modifié après le build. Un `git pull` met à jour ces
    # fichiers ; un simple commit ou un changement de documentation, non.
    local newer_source=""
    local candidates=()
    local path
    for path in "$HV_ROOT_DIR"/crates/*/src "$HV_ROOT_DIR"/crates/*/Cargo.toml "$HV_ROOT_DIR"/crates/*/build.rs \
      "$HV_ROOT_DIR/crates/heelonvault-app/assets" "$HV_ROOT_DIR/crates/heelonvault-core/locales" \
      "$HV_ROOT_DIR/Cargo.toml" "$HV_ROOT_DIR/Cargo.lock" \
      "$HV_ROOT_DIR/../heelonvault-premium/src" "$HV_ROOT_DIR/../heelonvault-premium/Cargo.toml"; do
      [[ -e "$path" ]] && candidates+=("$path")
    done
    if [[ "${#candidates[@]}" -gt 0 ]]; then
      newer_source="$(find "${candidates[@]}" -type f -newer "$HV_BINARY_SOURCE" -print -quit 2>/dev/null || true)"
    fi
    if [[ -n "$newer_source" ]]; then
      stale_reason="des sources ont été modifiées après le build (ex. ${newer_source#"$HV_ROOT_DIR"/})"
    fi
  fi

  if [[ -n "$stale_reason" ]]; then
    if [[ "$HV_ALLOW_STALE_BINARY" == "1" ]]; then
      echo "[WARN] Binaire potentiellement périmé : $stale_reason (HEELONVAULT_ALLOW_STALE_BINARY=1)."
      return
    fi
    echo "[ERROR] Binaire périmé : $stale_reason."
    echo "[ERROR] Reconstruisez-le (en tant qu'utilisateur, pas en root) :"
    echo "[ERROR]   cargo build --release -p heelonvault-app"
    echo "[ERROR] Si CARGO_TARGET_DIR est défini, indiquez le binaire produit :"
    echo "[ERROR]   sudo env HEELONVAULT_BINARY=\"\$CARGO_TARGET_DIR/release/heelonvault\" ./scripts/install.sh"
    echo "[ERROR] Pour forcer malgré tout : HEELONVAULT_ALLOW_STALE_BINARY=1"
    exit 1
  fi
}

# Version et profil de l'installation existante (pour la mise à jour).
hv_detect_previous_install() {
  if [[ -f "$HV_INSTALL_DIR/$HV_VERSION_FILE" ]]; then
    HV_PREVIOUS_VERSION="$(head -1 "$HV_INSTALL_DIR/$HV_VERSION_FILE")"
  elif [[ -x "$HV_INSTALL_DIR/heelonvault" ]]; then
    HV_PREVIOUS_VERSION="$(hv_binary_version "$HV_INSTALL_DIR/heelonvault")"
  fi

  if [[ -f "$HV_INSTALL_DIR/run.sh" ]]; then
    if grep -q "$HV_ENTERPRISE_DB_FILE" "$HV_INSTALL_DIR/run.sh"; then
      HV_PREVIOUS_DEPLOY_MODE="enterprise"
    else
      HV_PREVIOUS_DEPLOY_MODE="personal"
    fi
  fi

  if [[ -d "$HV_INSTALL_DIR" ]]; then
    echo "[INFO] Installation existante : version ${HV_PREVIOUS_VERSION:-inconnue}, profil ${HV_PREVIOUS_DEPLOY_MODE:-inconnu}"
  fi
}

# Une instance en cours d'exécution verrait son binaire remplacé et sa base
# copiée en pleine écriture : on demande de la fermer.
hv_ensure_app_not_running() {
  local pids
  pids="$(pgrep -x heelonvault 2>/dev/null || true)"
  if [[ -n "$pids" ]]; then
    echo "[ERROR] HeelonVault est en cours d'exécution (PID : $(tr '\n' ' ' <<< "$pids"))."
    echo "[ERROR] Fermez l'application puis relancez l'installation."
    exit 1
  fi
}

# Sauvegarde cohérente d'une base SQLite : `.backup` (verrou de lecture, copie
# page par page) puis contrôle d'intégrité de la copie. Repli sur une copie
# brute (base + journal éventuel) si le client sqlite3 est absent.
hv_backup_database() {
  local source="$1"
  local destination="$2"
  local check
  local suffix

  mkdir -p "$HV_BACKUP_DIR"
  chmod 700 "$HV_BACKUP_DIR"

  if command -v sqlite3 >/dev/null 2>&1; then
    sqlite3 "file:$source?mode=ro" ".backup '$destination'"
    check="$(sqlite3 "$destination" "PRAGMA integrity_check;" 2>&1 || true)"
    if [[ "$check" != "ok" ]]; then
      echo "[ERROR] Sauvegarde invalide ($destination) : $check"
      echo "[ERROR] Mise à jour interrompue, rien n'a été modifié."
      exit 1
    fi
  else
    echo "[WARN] Client sqlite3 absent : copie brute de la base."
    cp "$source" "$destination"
    for suffix in -wal -journal; do
      if [[ -f "$source$suffix" ]]; then
        cp "$source$suffix" "$destination$suffix"
      fi
    done
  fi
  chmod 600 "$destination"*
}

hv_verify_checksum() {
  if [[ -f "$HV_CHECKSUM_FILE" ]]; then
    if ! command -v sha256sum >/dev/null 2>&1; then
      echo "[ERROR] sha256sum introuvable, impossible de vérifier l'intégrité de l'archive."
      exit 1
    fi
    echo "[INFO] Vérification d'intégrité du binaire..."
    (
      cd "$HV_SCRIPT_DIR" || exit 1
      sha256sum -c "$(basename "$HV_CHECKSUM_FILE")"
    )
  else
    echo "[WARN] Fichier de checksum absent ($HV_CHECKSUM_FILE). Vérification d'intégrité ignorée."
  fi
}

hv_select_deploy_mode() {
  local deploy_choice

  echo ""
  if [[ "$HV_NON_INTERACTIVE" == "1" ]]; then
    case "${HEELONVAULT_DEPLOY_MODE:-${HV_PREVIOUS_DEPLOY_MODE:-personal}}" in
      enterprise)
        HV_DEPLOY_MODE="enterprise"
        ;;
      *)
        HV_DEPLOY_MODE="personal"
        ;;
    esac
    echo "[INFO] Mode non interactif actif"
    echo "[INFO] Profil de déploiement : $HV_DEPLOY_MODE"
  else
    echo "╔══════════════════════════════════════════════════════╗"
    echo "║           Profil de déploiement HeelonVault         ║"
    echo "╚══════════════════════════════════════════════════════╝"
    local default_choice=1
    if [[ "$HV_PREVIOUS_DEPLOY_MODE" == "enterprise" ]]; then
      default_choice=2
    fi
    if [[ "$default_choice" == 1 ]]; then
      echo "  [1] Personnel (poste local) [défaut${HV_PREVIOUS_DEPLOY_MODE:+, profil actuel}]"
      echo "  [2] Entreprise (serveur / multi-utilisateur)"
    else
      echo "  [1] Personnel (poste local)"
      echo "  [2] Entreprise (serveur / multi-utilisateur) [défaut, profil actuel]"
    fi
    echo ""
    read -rp "  Votre choix [1/2] : " deploy_choice

    case "${deploy_choice:-$default_choice}" in
      2)
        HV_DEPLOY_MODE="enterprise"
        echo "[INFO] Mode sélectionné : Entreprise"
        echo "[INFO] Aucun outil de publication réseau ne sera installé automatiquement."
        ;;
      *)
        HV_DEPLOY_MODE="personal"
        echo "[INFO] Mode sélectionné : Personnel"
        ;;
    esac
  fi

  if [[ -n "$HV_PREVIOUS_DEPLOY_MODE" && "$HV_DEPLOY_MODE" != "$HV_PREVIOUS_DEPLOY_MODE" ]]; then
    echo "[WARN] Changement de profil : $HV_PREVIOUS_DEPLOY_MODE → $HV_DEPLOY_MODE."
    echo "[WARN] L'application utilisera une autre base ; l'ancienne n'est ni déplacée ni supprimée."
  fi

  if [[ "$HV_DEPLOY_MODE" == "enterprise" ]]; then
    HV_ACTIVE_DB_FILE="$HV_ENTERPRISE_DB_FILE"
    HV_ACTIVE_DB_LABEL="entreprise"
  else
    HV_ACTIVE_DB_FILE="$HV_USER_DB_FILE"
    HV_ACTIVE_DB_LABEL="utilisateur ($HV_INVOKING_USER)"
  fi
}

hv_detect_existing_installation() {
  local choice

  if [[ -f "$HV_DB_FILE" ]]; then
    HV_HAS_LEGACY_DB=true
  fi

  if [[ -f "$HV_ACTIVE_DB_FILE" ]]; then
    HV_HAS_ACTIVE_DB=true
  fi

  if [[ -d "$HV_INSTALL_DIR" ]]; then
    HV_FRESH_INSTALL=false
    echo ""
    echo "╔══════════════════════════════════════════════════════╗"
    echo "║       HeelonVault est déjà installé                 ║"
    echo "╚══════════════════════════════════════════════════════╝"

    if [[ "$HV_HAS_ACTIVE_DB" == true ]]; then
      echo ""
      echo "  Base $HV_ACTIVE_DB_LABEL détectée :"
      echo "  $HV_ACTIVE_DB_FILE"
      echo "  (conservée automatiquement lors d'une réinstallation)"
    fi

    if [[ "$HV_HAS_LEGACY_DB" == true ]]; then
      echo ""
      echo "  Une base legacy existante a été détectée :"
      echo "  $HV_DB_FILE"
      echo ""
      echo "  Que souhaitez-vous faire ?"
      echo "  [1] Conserver data/ (mise à jour, backup automatique) [défaut]"
      echo "  [2] Repartir de zéro côté /opt (suppression complète, backup automatique)"
      echo ""
      if [[ "$HV_NON_INTERACTIVE" == "1" ]]; then
        choice="${HEELONVAULT_KEEP_LEGACY_DATA:-1}"
        echo "  [auto] choix non interactif: $choice"
      else
        read -rp "  Votre choix [1/2] : " choice
      fi

      case "${choice:-1}" in
        2)
          HV_KEEP_DATA=false
          echo ""
          echo "  ⚠  Le dossier /opt/heelonvault/data sera supprimé après backup."
          ;;
        *)
          HV_KEEP_DATA=true
          echo ""
          echo "  ✓  Le dossier /opt/heelonvault/data sera conservé."
          ;;
      esac
    else
      if [[ "$HV_HAS_ACTIVE_DB" == true ]]; then
        echo ""
        echo "  Aucune base legacy dans /opt détectée, mise à jour simple."
        echo "  La base $HV_ACTIVE_DB_LABEL existante reste inchangée."
      else
        echo "  Aucune base détectée, mise à jour simple."
      fi
      HV_KEEP_DATA=false
    fi
    echo ""
  fi
}

hv_display_target_paths() {
  local confirm

  echo ""
  echo "╔══════════════════════════════════════════════════════╗"
  echo "║       Validation des chemins d'installation         ║"
  echo "╚══════════════════════════════════════════════════════╝"
  echo ""
  echo "  Dossiers SYSTÈME qui seront créés/modifiés :"
  echo "  • Installation binaire  : $HV_INSTALL_DIR"
  echo "  • Lanceurs bureau       : $HV_SYSTEM_APPS_DIR"
  echo "  • Icônes système        : $HV_ICON_THEME_DIR/*/apps/"
  echo "  • Backups données       : $HV_BACKUP_DIR"
  echo ""
  
  if [[ "$HV_DEPLOY_MODE" == "enterprise" ]]; then
    echo "  Dossiers ENTREPRISE (multi-utilisateur) :"
    echo "  • Base de données      : $HV_ENTERPRISE_DB_FILE"
    echo "  • Logs                 : $HV_ENTERPRISE_LOG_DIR"
  else
    echo "  Dossier UTILISATEUR ($HV_INVOKING_USER) :"
    echo "  • Base de données      : $HV_USER_DB_FILE"
  fi
  echo ""

  if [[ "$HV_FRESH_INSTALL" == false ]]; then
    echo "  ⚠  MISE À JOUR DÉTECTÉE : ${HV_PREVIOUS_VERSION:-version inconnue} → ${HV_NEW_VERSION:-version inconnue}"
    echo "  • En cas d'échec, la version actuelle est restaurée automatiquement"
    if [[ "$HV_HAS_ACTIVE_DB" == true ]]; then
      echo "  • Base existante sera CONSERVÉE"
    fi
    if [[ "$HV_HAS_LEGACY_DB" == true ]]; then
      echo "  • Base legacy sera BACKUPÉE avant modification"
    fi
  fi
  echo ""

  if [[ "$HV_NON_INTERACTIVE" == "1" ]]; then
    echo "  [auto] Mode non interactif : poursuite sans confirmation"
    return
  fi

  read -rp "  Confirmer l'installation à ces chemins ? [o/N] : " confirm
  if [[ "${confirm,,}" != "o" ]]; then
    echo ""
    echo "[INFO] Installation annulée."
    exit 0
  fi
}

hv_manage_backups() {
  local timestamp
  local backup_file

  if [[ "$HV_HAS_LEGACY_DB" == true ]]; then
    timestamp="$(date +%Y%m%d_%H%M%S)"
    backup_file="$HV_BACKUP_DIR/heelonvault_legacy_backup_${timestamp}.db"
    hv_backup_database "$HV_DB_FILE" "$backup_file"
    echo "[INFO] Backup base legacy → $backup_file"
    hv_rotate_backups "$HV_BACKUP_DIR/heelonvault_legacy_backup_*.db" "$HV_BACKUP_KEEP"
  fi

  if [[ "$HV_HAS_ACTIVE_DB" == true ]]; then
    timestamp="$(date +%Y%m%d_%H%M%S)"
    if [[ "$HV_DEPLOY_MODE" == "enterprise" ]]; then
      backup_file="$HV_BACKUP_DIR/heelonvault_enterprise_backup_${timestamp}.db"
    else
      backup_file="$HV_BACKUP_DIR/heelonvault_user_${HV_INVOKING_USER}_backup_${timestamp}.db"
    fi
    hv_backup_database "$HV_ACTIVE_DB_FILE" "$backup_file"
    echo "[INFO] Backup base $HV_ACTIVE_DB_LABEL → $backup_file (intégrité vérifiée)"

    if [[ "$HV_DEPLOY_MODE" == "enterprise" ]]; then
      hv_rotate_backups "$HV_BACKUP_DIR/heelonvault_enterprise_backup_*.db" "$HV_BACKUP_KEEP"
    else
      hv_rotate_backups "$HV_BACKUP_DIR/heelonvault_user_${HV_INVOKING_USER}_backup_*.db" "$HV_BACKUP_KEEP"
    fi
  fi
}

# Prépare la nouvelle version dans HV_STAGE_DIR, sans toucher à l'installation
# courante. La base legacy /opt/heelonvault/data est recopiée si conservée.
hv_stage_files() {
  local stage="$HV_STAGE_DIR"
  echo "[INFO] Préparation de la nouvelle version dans $stage"
  rm -rf "$stage"
  mkdir -p "$stage/icons"

  if [[ "$HV_KEEP_DATA" == true && -d "$HV_DATA_DIR" ]]; then
    cp -a "$HV_DATA_DIR" "$stage/data"
  else
    mkdir -p "$stage/data"
  fi
  mkdir -p "$stage/logs"

  cp "$HV_BINARY_SOURCE" "$stage/heelonvault"
  cp "$HV_DESKTOP_SOURCE" "$stage/"
  cp -r "$HV_MIGRATIONS_SOURCE" "$stage/migrations"
  if [[ -n "$HV_NEW_VERSION" ]]; then
    echo "$HV_NEW_VERSION" > "$stage/$HV_VERSION_FILE"
  fi
  if [[ -f "$HV_ROOT_DIR/README.md" ]]; then
    cp "$HV_ROOT_DIR/README.md" "$stage/"
  fi
  if [[ -f "$HV_PRIMARY_ICON_SOURCE" ]]; then
    install -m 644 "$HV_PRIMARY_ICON_SOURCE" "$stage/icons/heelonvault.png"
  fi

  chown -R root:root "$stage"
  chmod 755 "$stage"
  chmod 700 "$stage/data" "$stage/logs"
}

# Échange atomique (rename sur le même système de fichiers) : l'installation
# courante devient HV_PREVIOUS_DIR, la version préparée prend sa place.
hv_swap_install() {
  rm -rf "$HV_PREVIOUS_DIR"
  if [[ -d "$HV_INSTALL_DIR" ]]; then
    mv "$HV_INSTALL_DIR" "$HV_PREVIOUS_DIR"
  fi
  HV_SWAPPED=true
  mv "$HV_STAGE_DIR" "$HV_INSTALL_DIR"
  echo "[INFO] Nouvelle version en place : $HV_INSTALL_DIR"
}

# Piège EXIT : tant que la transaction est ouverte, toute sortie (erreur sous
# `set -e`, y compris dans une fonction, ou interruption) restaure l'état
# précédent. Un piège ERR ne suffirait pas : sans `set -E`, il n'est pas
# hérité par les fonctions.
hv_on_exit() {
  local status=$?
  if [[ "$HV_TRANSACTION_OPEN" == true ]]; then
    hv_rollback "$status"
  fi
}

hv_rollback() {
  local status="$1"
  HV_TRANSACTION_OPEN=false
  [[ "$status" -ne 0 ]] || status=1
  echo ""
  echo "[ERROR] Échec de l'installation (code $status) : restauration de l'état précédent."
  rm -rf "$HV_STAGE_DIR"
  if [[ "$HV_SWAPPED" == true ]]; then
    rm -rf "$HV_INSTALL_DIR"
    if [[ -d "$HV_PREVIOUS_DIR" ]]; then
      mv "$HV_PREVIOUS_DIR" "$HV_INSTALL_DIR"
      echo "[INFO] Version précédente restaurée dans $HV_INSTALL_DIR"
    fi
  fi
  echo "[INFO] Les bases de données n'ont pas été modifiées ; sauvegardes dans $HV_BACKUP_DIR."
  exit "$status"
}

hv_fix_user_data_owner() {
  # Corriger le propriétaire du répertoire de données utilisateur si hérité d'un autre UID
  local user_data_dir="$HV_INVOKING_HOME/.local/share/heelonvault"
  if [[ -d "$user_data_dir" ]] && [[ "$(stat -c '%u' "$user_data_dir")" != "$(id -u "$HV_INVOKING_USER" 2>/dev/null || echo 0)" ]]; then
    echo "[WARN] Répertoire données utilisateur ($user_data_dir) appartient à un autre UID — chown appliqué"
    chown "$HV_INVOKING_USER":"$HV_INVOKING_USER" "$user_data_dir"
  fi

  if [[ "$HV_DEPLOY_MODE" == "enterprise" ]]; then
    mkdir -p "$HV_ENTERPRISE_DATA_DIR" "$HV_ENTERPRISE_LOG_DIR"
    chown "$HV_INVOKING_USER":"$HV_INVOKING_USER" "$HV_ENTERPRISE_DATA_DIR" "$HV_ENTERPRISE_LOG_DIR"
    chmod 750 "$HV_ENTERPRISE_DATA_DIR" "$HV_ENTERPRISE_LOG_DIR"
  fi
}

hv_validate_migrations_payload() {
  local src_dir="$HV_MIGRATIONS_SOURCE"
  local dst_dir="$HV_STAGE_DIR/migrations"
  local src_list
  local dst_list
  local sql_file

  if [[ ! -d "$dst_dir" ]]; then
    echo "[ERROR] Dossier migrations manquant après déploiement : $dst_dir"
    exit 1
  fi

  src_list="$(find "$src_dir" -maxdepth 1 -type f -name '*.sql' -printf '%f\n' | LC_ALL=C sort)"
  dst_list="$(find "$dst_dir" -maxdepth 1 -type f -name '*.sql' -printf '%f\n' | LC_ALL=C sort)"

  if [[ -z "$src_list" ]]; then
    echo "[ERROR] Aucun fichier migration .sql dans la source : $src_dir"
    exit 1
  fi

  if [[ "$src_list" != "$dst_list" ]]; then
    echo "[ERROR] Liste des migrations copiées incohérente entre source et destination"
    echo "[ERROR] Source      : $src_dir"
    echo "[ERROR] Destination : $dst_dir"
    exit 1
  fi

  while IFS= read -r sql_file; do
    [[ -n "$sql_file" ]] || continue
    if ! cmp -s "$src_dir/$sql_file" "$dst_dir/$sql_file"; then
      echo "[ERROR] Migration copiée avec un contenu différent : $sql_file"
      exit 1
    fi
  done <<< "$src_list"

  echo "[INFO] Migrations validées : source et destination sont identiques"
}

hv_generate_run_script() {
  if [[ "$HV_DEPLOY_MODE" == "enterprise" ]]; then
cat > "$HV_STAGE_DIR/run.sh" <<EOF
#!/usr/bin/env bash
set -euo pipefail

APP_DIR="/opt/heelonvault"
DB_PATH="$HV_ENTERPRISE_DB_FILE"
LOG_DIR="$HV_ENTERPRISE_LOG_DIR"

umask 077
mkdir -p "$HV_ENTERPRISE_DATA_DIR" "$HV_ENTERPRISE_LOG_DIR"

cd /opt/heelonvault
export HEELONVAULT_DB_PATH="\$DB_PATH"
export HEELONVAULT_LOG_DIR="\$LOG_DIR"
export HEELONVAULT_MIGRATIONS_DIR="/opt/heelonvault/migrations"
if [[ ! -d "\$HEELONVAULT_MIGRATIONS_DIR" ]]; then
  echo "[ERROR] Dossier migrations introuvable: \$HEELONVAULT_MIGRATIONS_DIR" >&2
  exit 1
fi
exec /opt/heelonvault/heelonvault "\$@"
EOF
  else
cat > "$HV_STAGE_DIR/run.sh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

APP_DIR="/opt/heelonvault"
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
STATE_HOME="${XDG_STATE_HOME:-$HOME/.local/state}"
APP_DATA_DIR="$DATA_HOME/heelonvault"
APP_STATE_DIR="$STATE_HOME/heelonvault"
DB_PATH="$APP_DATA_DIR/heelonvault-rust.db"
LOG_DIR="$APP_STATE_DIR/logs"
LEGACY_DB_PATH="/opt/heelonvault/data/heelonvault-rust-dev.db"

umask 077
mkdir -p "$APP_DATA_DIR" "$LOG_DIR"

if [[ ! -f "$DB_PATH" && -r "$LEGACY_DB_PATH" ]]; then
  cp -n "$LEGACY_DB_PATH" "$DB_PATH" 2>/dev/null || true
fi

cd /opt/heelonvault
export HEELONVAULT_DB_PATH="$DB_PATH"
export HEELONVAULT_LOG_DIR="$LOG_DIR"
export HEELONVAULT_MIGRATIONS_DIR="/opt/heelonvault/migrations"
if [[ ! -d "$HEELONVAULT_MIGRATIONS_DIR" ]]; then
  echo "[ERROR] Dossier migrations introuvable: $HEELONVAULT_MIGRATIONS_DIR" >&2
  exit 1
fi
exec /opt/heelonvault/heelonvault "$@"
EOF
  fi

  chmod 755 "$HV_STAGE_DIR/heelonvault"
  # run.sh doit rester exécutable par l'utilisateur final (lancement desktop).
  chmod 755 "$HV_STAGE_DIR/run.sh"
}

hv_install_icons() {
  local size
  local src
  local dst
  # HV_ASSETS_SOURCE is resolved once in hv_verify_core_files (root/script-dir/
  # crate-local fallback) — reuse it here instead of re-deriving it.
  local assets_dir="$HV_ASSETS_SOURCE"

  if [[ -z "$assets_dir" || ! -d "$assets_dir" ]]; then
    echo "[WARN] Dossier assets introuvable, installation des icônes ignorée"
    return
  fi

  echo "[INFO] Installation des icônes..."
  for size in 48x48 128x128 256x256; do
    src="$assets_dir/icons/hicolor/$size/apps/heelonvault.png"
    dst="$HV_ICON_THEME_DIR/$size/apps"
    if [[ -f "$src" ]]; then
      mkdir -p "$dst"
      install -m 644 "$src" "$dst/heelonvault.png"
      install -m 644 "$src" "$dst/$HV_APP_ID.png"
    fi
  done

  if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "$HV_ICON_THEME_DIR" 2>/dev/null || true
  fi
}

hv_install_desktop_integration() {
  echo "[INFO] Installation du raccourci bureau..."
  cat > "$HV_DESKTOP_PATH" <<EOF
[Desktop Entry]
Version=1.0
Type=Application
Name=$HV_APP_NAME
Comment=Gestionnaire de mots de passe
Exec=/opt/heelonvault/run.sh
TryExec=/opt/heelonvault/run.sh
Icon=$HV_LOCAL_ICON_PATH
Terminal=false
Categories=System;Security;
Keywords=security;secret;encryption;vault;password;
StartupNotify=true
StartupWMClass=$HV_APP_ID
EOF

  chmod 644 "$HV_DESKTOP_PATH"

  cat > "$HV_LEGACY_DESKTOP_PATH" <<EOF
[Desktop Entry]
Version=1.0
Type=Application
Name=$HV_APP_NAME
Comment=Gestionnaire de mots de passe
Exec=/opt/heelonvault/run.sh
TryExec=/opt/heelonvault/run.sh
Icon=$HV_LOCAL_ICON_PATH
Terminal=false
Categories=System;Security;
Keywords=security;secret;encryption;vault;password;
StartupNotify=true
StartupWMClass=$HV_APP_ID
NoDisplay=true
EOF
  chmod 644 "$HV_LEGACY_DESKTOP_PATH"

  if command -v desktop-file-validate >/dev/null 2>&1; then
    desktop-file-validate "$HV_DESKTOP_PATH"
    desktop-file-validate "$HV_LEGACY_DESKTOP_PATH"
  fi

  if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$HV_SYSTEM_APPS_DIR" 2>/dev/null || true
  fi
}

hv_validate_artifacts() {
  if [[ ! -x "$HV_INSTALL_DIR/run.sh" ]]; then
    echo "[ERROR] Lanceur terminal manquant ou non executable: $HV_INSTALL_DIR/run.sh"
    exit 1
  fi

  if [[ ! -f "$HV_DESKTOP_PATH" ]]; then
    echo "[ERROR] Lanceur desktop non installe: $HV_DESKTOP_PATH"
    exit 1
  fi

  if [[ ! -f "$HV_LEGACY_DESKTOP_PATH" ]]; then
    echo "[ERROR] Lanceur desktop legacy non installe: $HV_LEGACY_DESKTOP_PATH"
    exit 1
  fi

  if [[ ! -d "$HV_INSTALL_DIR/migrations" ]]; then
    echo "[ERROR] Dossier migrations non installe: $HV_INSTALL_DIR/migrations"
    exit 1
  fi

  if ! find "$HV_INSTALL_DIR/migrations" -maxdepth 1 -type f -name '*.sql' | grep -q .; then
    echo "[ERROR] Aucun fichier migration .sql trouvé dans: $HV_INSTALL_DIR/migrations"
    exit 1
  fi
}

hv_print_summary() {
  echo ""
  echo "╔══════════════════════════════════════════════════════╗"
  echo "║            Installation terminée                    ║"
  echo "╚══════════════════════════════════════════════════════╝"
  echo "[OK] Binaire installé : $HV_INSTALL_DIR/heelonvault"
  echo "[OK] Lanceur installé : $HV_DESKTOP_PATH"
  echo "[OK] Lanceur compat installé : $HV_LEGACY_DESKTOP_PATH"
  echo "[OK] Lancement terminal : $HV_INSTALL_DIR/run.sh"
  if [[ "$HV_DEPLOY_MODE" == "enterprise" ]]; then
    echo "[OK] Profil déployé : Entreprise"
    echo "[OK] Base entreprise : $HV_ENTERPRISE_DB_FILE"
    echo "[OK] Logs entreprise : $HV_ENTERPRISE_LOG_DIR"
  else
    echo "[OK] Profil déployé : Personnel"
    echo "[OK] Base utilisateur : ~/.local/share/heelonvault/heelonvault-rust.db"
    echo "[OK] Logs utilisateur : ~/.local/state/heelonvault/logs"
  fi
  echo "[OK] L'application est disponible dans le menu applicatif GNOME"
  echo "[OK] Test menu: gtk-launch $HV_APP_ID"
}

hv_print_enterprise_tips() {
  if [[ "$HV_DEPLOY_MODE" == "enterprise" ]]; then
    echo ""
    echo "[INFO] Conseils publication réseau (non automatisée) :"
    echo "[INFO] 1) Prévoir un compte système dédié à l'exécution de HeelonVault."
    echo "[INFO] 2) Publier via une solution d'accès distant d'entreprise (RDS/VDI/RemoteApp)."
    echo "[INFO] 3) Mettre en place backup et rotation des logs pour $HV_ENTERPRISE_DATA_DIR et $HV_ENTERPRISE_LOG_DIR."
    echo "[INFO] 4) Restreindre les accès OS/réseau au strict nécessaire (pare-feu, comptes, audit)."
  fi
}

hv_print_dry_run_plan() {
  echo ""
  echo "[DRY-RUN] Aucun changement système n'a été appliqué."
  echo "[DRY-RUN] Profil visé : $HV_DEPLOY_MODE"
  echo "[DRY-RUN] Déploiement prévu dans : $HV_INSTALL_DIR"
  echo "[DRY-RUN] Binaire source : $HV_BINARY_SOURCE"
  echo "[DRY-RUN] run.sh cible : $HV_INSTALL_DIR/run.sh"
  echo "[DRY-RUN] Desktop entries : $HV_DESKTOP_PATH et $HV_LEGACY_DESKTOP_PATH"
  echo "[DRY-RUN] Icônes thème : $HV_ICON_THEME_DIR/{48x48,128x128,256x256}/apps"
  if [[ "$HV_DEPLOY_MODE" == "enterprise" ]]; then
    echo "[DRY-RUN] Base entreprise : $HV_ENTERPRISE_DB_FILE"
    echo "[DRY-RUN] Logs entreprise : $HV_ENTERPRISE_LOG_DIR"
  else
    echo "[DRY-RUN] Base utilisateur : $HV_USER_DB_FILE"
    echo "[DRY-RUN] Logs utilisateur : ~/.local/state/heelonvault/logs"
  fi

  if [[ "$HV_HAS_LEGACY_DB" == true ]]; then
    echo "[DRY-RUN] Backup legacy prévu dans : $HV_BACKUP_DIR"
  fi
  if [[ "$HV_HAS_ACTIVE_DB" == true ]]; then
    echo "[DRY-RUN] Backup base active prévu dans : $HV_BACKUP_DIR"
  fi
  echo "[DRY-RUN] Dépendances runtime seraient vérifiées/installées."
  echo ""
}

hv_run_common_install_flow() {
  hv_verify_core_files
  hv_verify_checksum
  hv_check_binary_freshness
  hv_detect_previous_install
  hv_select_deploy_mode
  hv_detect_existing_installation
  hv_display_target_paths

  if [[ "$HV_DRY_RUN" == "1" ]]; then
    hv_print_dry_run_plan
    return
  fi

  if ! declare -F hv_install_runtime_dependencies >/dev/null 2>&1; then
    echo "[ERROR] Fonction hv_install_runtime_dependencies manquante dans le wrapper OS."
    exit 1
  fi

  # 1. Contrôles et préparatifs sans effet sur l'installation courante : un
  #    échec ici laisse le système tel quel.
  hv_ensure_app_not_running
  hv_install_runtime_dependencies
  hv_manage_backups
  hv_stage_files
  hv_validate_migrations_payload
  hv_generate_run_script

  # 2. Bascule puis intégration système : toute erreur restaure la version
  #    précédente.
  trap hv_on_exit EXIT
  trap 'exit 130' INT TERM
  HV_TRANSACTION_OPEN=true
  hv_swap_install
  hv_fix_user_data_owner
  hv_install_icons
  hv_install_desktop_integration
  hv_validate_artifacts
  if declare -F hv_post_install_os_specific >/dev/null 2>&1; then
    hv_post_install_os_specific
  fi
  HV_TRANSACTION_OPEN=false
  trap - EXIT INT TERM

  rm -rf "$HV_PREVIOUS_DIR"
  hv_print_summary
  hv_print_enterprise_tips
}
