# ============================================================
# HeelonVault — Remote Windows MSI Build (VERSION ULTIME)
# ============================================================

# ------------------------------------------------------------
# Configuration
# ------------------------------------------------------------
VM_IP := "192.168.122.47"
VM_USER := "builduser"
SSH_KEY := "~/.ssh/id_ed25519"

PROJECT_ROOT := ".."
# ⚠️ Utiliser / pour PowerShell (compatible avec les 2 OS)
REMOTE_ROOT := "C:/Users/" + VM_USER + "/build/heelonvault"
REMOTE_CORE := REMOTE_ROOT + "/heelonvault-core"
CRATE_DIR := "crates/heelonvault-app"
LOCAL_DIST := "./dist"

# Chemin vers dist.exe (avec / pour PowerShell)
DIST_EXE_PATH := "C:/Users/" + VM_USER + "/.cargo/bin/dist.exe"

# Triple cible pour dist build (requis car --artifacts désactive le mode host)
WINDOWS_TARGET := "x86_64-pc-windows-msvc"

# ------------------------------------------------------------
# Default
# ------------------------------------------------------------
default:
    @just --list

# ------------------------------------------------------------
# Vérification de l'environnement Rust sur Windows
# ------------------------------------------------------------
check-windows:
    @echo "🔍 Vérification de l'environnement Rust sur Windows..."
    ssh -i {{SSH_KEY}} -o IdentitiesOnly=yes {{VM_USER}}@{{VM_IP}} \
        "powershell -NoProfile -NonInteractive -Command \"cargo --version; & '{{DIST_EXE_PATH}}' --version\""

# ------------------------------------------------------------
# Nettoyage du workspace Windows
# ------------------------------------------------------------
clean-remote:
    @echo "🧹 Nettoyage du workspace Windows..."
    ssh -i {{SSH_KEY}} -o IdentitiesOnly=yes {{VM_USER}}@{{VM_IP}} \
        "powershell -NoProfile -NonInteractive -Command \"if (Test-Path '{{REMOTE_ROOT}}') { Remove-Item -Recurse -Force '{{REMOTE_ROOT}}' }; New-Item -ItemType Directory -Force -Path '{{REMOTE_ROOT}}'\""

# ------------------------------------------------------------
# Synchronisation des deux projets (CORRIGÉE)
# ------------------------------------------------------------
@sync:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "📦 Préparation du workspace Windows..."
    ssh -i {{SSH_KEY}} -o IdentitiesOnly=yes {{VM_USER}}@{{VM_IP}} \
        "powershell -NoProfile -NonInteractive -Command \"New-Item -ItemType Directory -Force -Path '{{REMOTE_ROOT}}'\""
    echo "📤 Création de l'archive locale (depuis {{PROJECT_ROOT}})..."
    tar -czf /tmp/heelonvault-sources.tar.gz \
        -C {{PROJECT_ROOT}} \
        --exclude='heelonvault-core/target' \
        --exclude='heelonvault-core/.git' \
        --exclude='heelonvault-core/logs' \
        --exclude='heelonvault-core/data' \
        --exclude='heelonvault-premium/target' \
        --exclude='heelonvault-premium/.git' \
        --exclude='heelonvault-premium/logs' \
        --exclude='heelonvault-premium/data' \
        heelonvault-core heelonvault-premium
    echo "📥 Transfert et extraction sur Windows..."
    scp -i {{SSH_KEY}} /tmp/heelonvault-sources.tar.gz {{VM_USER}}@{{VM_IP}}:C:/Users/{{VM_USER}}/Downloads/
    ssh -i {{SSH_KEY}} -o IdentitiesOnly=yes {{VM_USER}}@{{VM_IP}} \
        "powershell -NoProfile -NonInteractive -Command \"Set-Location '{{REMOTE_ROOT}}'; tar -xzf 'C:/Users/{{VM_USER}}/Downloads/heelonvault-sources.tar.gz'\""
    rm -f /tmp/heelonvault-sources.tar.gz
    echo "✅ Sources transférées."

# ------------------------------------------------------------
# Build MSI
# ------------------------------------------------------------
@build-msi: sync
    #!/usr/bin/env bash
    set -euxo pipefail
    echo "🔨 Compilation du MSI sur Windows..."
    echo "🔍 Vérification de dist.exe..."
    ssh -i {{SSH_KEY}} -o IdentitiesOnly=yes {{VM_USER}}@{{VM_IP}} \
        "powershell -NoProfile -NonInteractive -Command \"cargo --version; & '{{DIST_EXE_PATH}}' --version\""
    echo "🚀 Lancement de dist build (default inclut heelonvault-premium)..."
    ssh -i {{SSH_KEY}} -o IdentitiesOnly=yes {{VM_USER}}@{{VM_IP}} \
        "powershell -NoProfile -NonInteractive -Command \"Set-Location '{{REMOTE_CORE}}/{{CRATE_DIR}}'; & '{{DIST_EXE_PATH}}' build --target={{WINDOWS_TARGET}}\""
    echo "📦 Recherche du MSI..."
    mkdir -p {{LOCAL_DIST}}
    MSI_PATH=$(ssh -i {{SSH_KEY}} -o IdentitiesOnly=yes {{VM_USER}}@{{VM_IP}} \
        "powershell -NoProfile -NonInteractive -Command \
            \"\$msi = Get-ChildItem '{{REMOTE_CORE}}/{{CRATE_DIR}}/target/dist/*.msi' | Select-Object -First 1; \
            if (-not \$msi) { Write-Error 'Aucun MSI trouvé'; exit 1 }; \
            Write-Output \$msi.FullName\"" \
        | tr -d '\r')
    echo "📄 MSI trouvé : $MSI_PATH"
    test -n "$MSI_PATH" || { echo "❌ MSI introuvable"; exit 1; }
    ssh -i {{SSH_KEY}} -o IdentitiesOnly=yes {{VM_USER}}@{{VM_IP}} \
        "powershell -NoProfile -NonInteractive -Command \"[IO.File]::ReadAllBytes('$MSI_PATH')\"" \
        > {{LOCAL_DIST}}/Heelonvault.msi
    test -s {{LOCAL_DIST}}/Heelonvault.msi
    echo "✅ MSI disponible dans {{LOCAL_DIST}}/Heelonvault.msi"

# ------------------------------------------------------------
# Build local (pour développement)
# ------------------------------------------------------------
build:
    # Build avec code premium (par défaut)
    cargo build

build-community:
    # Build SANS code premium (pour tests community)
    cargo build --no-default-features

# ------------------------------------------------------------
# Rebuild complet
# ------------------------------------------------------------
rebuild-msi:
    just clean-remote
    just build-msi