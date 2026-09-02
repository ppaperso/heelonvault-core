#!/usr/bin/env bash
set -euo pipefail

# --- Couleurs ---
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

info() { echo -e "${CYAN}[INFO]${NC} $*"; }
ok()   { echo -e "${GREEN}[ OK ]${NC} $*"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
err()  { echo -e "${RED}[ERR ]${NC} $*"; }

# --- Vérification de l'environnement ---
if [[ "${MSYSTEM:-}" != "MINGW64" ]]; then
    err "Ce script doit être exécuté dans l'environnement MSYS2 MINGW64."
    exit 1
fi

# Sécurité : s'assurer que les binaires de base sont accessibles
export PATH="/usr/local/bin:/usr/bin:/bin:/mingw64/bin:$PATH"

# --- 1. Mise à jour MSYS2 ---
info "Synchronisation et mise à jour des paquets de base..."
pacman -Syu --noconfirm

# --- 2. Installation des paquets ---
info "Installation des paquets de compilation et dépendances..."
packages=(
    git
    curl
    mingw-w64-x86_64-pkgconf
    mingw-w64-x86_64-ntldd
    mingw-w64-x86_64-imagemagick
    mingw-w64-x86_64-glib2
    mingw-w64-x86_64-gdk-pixbuf2
    mingw-w64-x86_64-gtk4
    mingw-w64-x86_64-graphene
    mingw-w64-x86_64-libepoxy
    mingw-w64-x86_64-libadwaita
    python3
)
pacman -S --needed --noconfirm "${packages[@]}"

# --- 3. Rustup ---
if command -v cargo &>/dev/null; then
    ok "Rustup déjà présent ($('cargo --version'))"
else
    info "Installation de rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile minimal
fi

# --- 4. Configuration permanente du PATH ---
# .bashrc (shells interactifs)
if ! grep -q '\.cargo/bin' ~/.bashrc 2>/dev/null; then
    echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.bashrc
    ok "PATH cargo ajouté à ~/.bashrc"
fi

# /etc/profile.d (shells de login — propre et résistant aux mises à jour MSYS2)
PROFILE_D="/etc/profile.d"
mkdir -p "$PROFILE_D"

if [[ ! -f "$PROFILE_D/dotnet_tools.sh" ]]; then
    # $USER est l'utilisateur Windows dans MSYS2
    cat > "$PROFILE_D/dotnet_tools.sh" << EOF
export PATH="/c/Program Files/dotnet:\$PATH"
export PATH="/c/Users/$USER/.dotnet/tools:\$PATH"
EOF
    ok "PATH dotnet configuré dans /etc/profile.d/"
fi

# --- 5. Vérifications finales ---
info "Vérifications des outils MSYS2..."

declare -A check_cmd=(
    [git]="git --version"
    [rustc]="rustc --version"
    [cargo]="cargo --version"
    [ntldd]="ntldd --version 2>/dev/null || echo présent"
    [imagemagick]="convert --version | head -1"
    [glib-compile-schemas]="glib-compile-schemas --version"
    [python3]="python3 --version"
    [wix]="wix --version"
    [gdk-pixbuf-query-loaders]="command -v gdk-pixbuf-query-loaders"
)

for name in "${!check_cmd[@]}"; do
    cmd="${check_cmd[$name]}"
    if eval "$cmd" &>/dev/null; then
        version=$(eval "$cmd" 2>/dev/null | head -1 || true)
        ok "$name : ${version:-OK}"
    else
        err "$name : ABSENT"
    fi
done

info "Configuration MSYS2 terminée avec succès."
