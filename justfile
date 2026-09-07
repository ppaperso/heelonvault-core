# ============================================================
# HeelonVault — Remote Windows MSI Build (VERSION FINALE)
# ============================================================

# ------------------------------------------------------------
# Configuration
# ------------------------------------------------------------
VM_IP := "192.168.122.47"
VM_USER := "builduser"
SSH_KEY := "~/.ssh/id_ed25519"

PROJECT_ROOT := ".."
REMOTE_ROOT := "C:/Users/" + VM_USER + "/build/heelonvault"
REMOTE_CORE := REMOTE_ROOT + "/heelonvault-core"
CRATE_DIR := "crates/heelonvault-app"
LOCAL_DIST := "./dist"

# Chemin vers dist.exe sur la VM Windows
DIST_EXE_PATH := "C:/Users/" + VM_USER + "/.cargo/bin/dist.exe"
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
# Synchronisation des deux projets
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
@build-msi:
    #!/usr/bin/env bash
    set -euo pipefail

    echo "🔨 Compilation du MSI sur Windows..."

    # Verification de l'initialisation dist
    echo "🔧 Vérification de la configuration cargo-dist..."
    ssh -i {{SSH_KEY}} -o IdentitiesOnly=yes {{VM_USER}}@{{VM_IP}} \
        "powershell -NoProfile -NonInteractive -Command \
        \"if (-not (Test-Path '{{REMOTE_CORE}}/crates/heelonvault-app/dist-workspace.toml')) { \
            Write-Error 'Configuration cargo-dist manquante. Exécutez dist init manuellement sur la VM.'; \
            exit 1 \
        }\""

    echo "🚀 Génération et compilation avec MSVC..."
    ssh -i {{SSH_KEY}} -o IdentitiesOnly=yes {{VM_USER}}@{{VM_IP}} \
        "powershell -NoProfile -NonInteractive -Command \
        \"\$msvc_path = 'C:\\Program Files (x86)\\Microsoft Visual Studio\\18\\BuildTools\\VC\\Tools\\MSVC\\14.51.36231\\bin\\Hostx64\\x64'; \
         \$env:PATH = \$msvc_path + ';' + \$env:USERPROFILE + '\\.cargo\\bin;' + 'C:\\msys64\\mingw64\\bin;' + \$env:PATH; \
         Set-Location '{{REMOTE_CORE}}/{{CRATE_DIR}}'; \
         & '{{DIST_EXE_PATH}}' generate --mode=msi --target={{WINDOWS_TARGET}}; \
         & '{{DIST_EXE_PATH}}' build --artifacts=local --target={{WINDOWS_TARGET}}\""

    echo "🔎 Recherche du MSI généré..."

    MSI_PATH=$(ssh -i {{SSH_KEY}} -o IdentitiesOnly=yes {{VM_USER}}@{{VM_IP}} \
        "powershell -NoProfile -NonInteractive -Command \
        \"\$msi = Get-ChildItem '{{REMOTE_CORE}}/{{CRATE_DIR}}/target' -Recurse -Filter '*.msi' | Sort-Object LastWriteTime -Descending | Select-Object -First 1; \
        if (-not \$msi) { Write-Error 'Aucun MSI trouvé sous target'; exit 1 }; \
        Write-Output \$msi.FullName\"" \
        | tr -d '\r')

    echo "📄 MSI trouvé : $MSI_PATH"

    test -n "$MSI_PATH" || {
        echo "❌ MSI introuvable"
        exit 1
    }

    echo "📦 Préparation du MSI pour transfert..."

    ssh -i {{SSH_KEY}} -o IdentitiesOnly=yes {{VM_USER}}@{{VM_IP}} \
        "powershell -NoProfile -NonInteractive -Command \
        \"Copy-Item -LiteralPath '$MSI_PATH' -Destination 'C:/Users/{{VM_USER}}/Downloads/HeelonVault.msi' -Force\""

    echo "📥 Transfert du MSI vers Fedora..."

    mkdir -p {{LOCAL_DIST}}

    scp -i {{SSH_KEY}} \
        {{VM_USER}}@{{VM_IP}}:"C:/Users/{{VM_USER}}/Downloads/HeelonVault.msi" \
        {{LOCAL_DIST}}/HeelonVault.msi

    echo "🔍 Vérification du fichier MSI..."

    test -s {{LOCAL_DIST}}/HeelonVault.msi || {
        echo "❌ MSI vide ou transfert échoué"
        exit 1
    }

    echo "📦 Taille du MSI :"
    ls -lh {{LOCAL_DIST}}/HeelonVault.msi

    echo ""
    echo "✅ MSI disponible : {{LOCAL_DIST}}/HeelonVault.msi"

# ------------------------------------------------------------
# Pipelines combinés
# ------------------------------------------------------------
full-build: sync build-msi

rebuild-msi: clean-remote full-build