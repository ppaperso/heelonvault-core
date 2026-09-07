# ============================================================
# HeelonVault — Remote Windows MSI Build (WiX Manual Approach)
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

# Chemin vers les outils Windows
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
        "powershell -NoProfile -NonInteractive -Command \"cargo --version\""

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
# Build MSI (Approche manuelle avec WiX Toolset)
# ------------------------------------------------------------
@build-msi:
    #!/usr/bin/env bash
    set -euo pipefail

    echo "🔨 Génération manuelle du MSI avec WiX Toolset..."

    # 1. Vérification de l'environnement WiX
    echo "🔧 Vérification de WiX Toolset..."
    ssh -i {{SSH_KEY}} -o IdentitiesOnly=yes {{VM_USER}}@{{VM_IP}} \
        "powershell -NoProfile -NonInteractive -Command \
        \"if (-not (Get-Command candle.exe -ErrorAction SilentlyContinue)) { \\
            Write-Error 'WiX Toolset non installé. Exécutez : winget install WiXToolset.WiXToolset'; \\
            exit 1 \\
        }\""

    # 2. Vérification que MSYS2 est accessible
    echo "🔧 Vérification de MSYS2..."
    ssh -i {{SSH_KEY}} -o IdentitiesOnly=yes {{VM_USER}}@{{VM_IP}} \
        "powershell -NoProfile -NonInteractive -Command \
        \"if (-not (Test-Path 'C:/msys64/mingw64/bin')) { \\
            Write-Error 'MSYS2 non installé ou chemin incorrect'; \\
            exit 1 \\
        }\""

    # 3. Compilation du binaire Rust
    echo "🚀 Compilation Rust avec MSVC..."
    ssh -i {{SSH_KEY}} -o IdentitiesOnly=yes {{VM_USER}}@{{VM_IP}} \
        "powershell -NoProfile -NonInteractive -Command \
        \"\$msvc_path = 'C:\\Program Files (x86)\\Microsoft Visual Studio\\18\\BuildTools\\VC\\Tools\\MSVC\\14.51.36231\\bin\\Hostx64\\x64'; \\
         \$env:PATH = \$msvc_path + ';' + \$env:USERPROFILE + '\\.cargo\\bin;' + 'C:\\msys64\\mingw64\\bin;' + \$env:PATH; \\
         Set-Location '{{REMOTE_CORE}}/{{CRATE_DIR}}'; \\
         cargo build --release --target {{WINDOWS_TARGET}} --manifest-path Cargo.toml 2>&1 | Write-Output\""

    # 4. Préparation du staging
    echo "📦 Préparation du dossier de staging..."
    ssh -i {{SSH_KEY}} -o IdentitiesOnly=yes {{VM_USER}}@{{VM_IP}} \
        "powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command \
        \"Set-Location '{{REMOTE_CORE}}/{{CRATE_DIR}}'; \\
         & 'scripts\\windows\\prepare-staging.ps1' \\
            -BinaryPath 'target\\{{WINDOWS_TARGET}}\\release\\heelonvault.exe' \\
            -Msys2Bin 'C:\\msys64\\mingw64\\bin' \\
            -StagingDir 'wix\\staging' \\
            -OutputDir 'wix\\output'\""

    # 5. Récupération de la version
    echo "📋 Récupération de la version..."
    VERSION=$(ssh -i {{SSH_KEY}} -o IdentitiesOnly=yes {{VM_USER}}@{{VM_IP}} \
        "powershell -NoProfile -NonInteractive -Command \
        \"Set-Location '{{REMOTE_CORE}}/{{CRATE_DIR}}'; \\
         (Get-Content Cargo.toml | Select-String -Pattern '^version\\s*=\\s*\"([^\"]+)\"').Matches[0].Groups[1].Value\"" | tr -d '\r')
    
    echo "Version: $VERSION"

    # 6. Compilation WiX avec candle (avec WixUtilExtension pour <Files Include="...">)
    echo "🕯️  Exécution de candle (compilation WiX)..."
    ssh -i {{SSH_KEY}} -o IdentitiesOnly=yes {{VM_USER}}@{{VM_IP}} \
        "powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command \
        \"Set-Location '{{REMOTE_CORE}}/{{CRATE_DIR}}'; \\
         \$wixBin = 'C:\\Program Files (x86)\\WiX Toolset v3.11\\bin'; \\
         \$env:PATH = \"\$wixBin;\$env:PATH\"; \\
         candle.exe 'wix\\main.wxs' 'wix\\staging.wxs' \\
            -out 'wix\\output' \\
            -dVersion=$VERSION \\
            -ext WixUtilExtension \\
            2>&1 | Write-Output\""

    # 7. Linkage avec light (avec WixUIExtension + WixUtilExtension)
    echo "💡 Exécution de light (linkage MSI)..."
    ssh -i {{SSH_KEY}} -o IdentitiesOnly=yes {{VM_USER}}@{{VM_IP}} \
        "powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command \
        \"Set-Location '{{REMOTE_CORE}}/{{CRATE_DIR}}'; \\
         \$wixBin = 'C:\\Program Files (x86)\\WiX Toolset v3.11\\bin'; \\
         \$env:PATH = \"\$wixBin;\$env:PATH\"; \\
         light.exe 'wix\\output\\main.wixobj' 'wix\\output\\staging.wixobj' \\
            -out 'wix\\output\\HeelonVault-$VERSION.msi' \\
            -ext WixUIExtension \\
            -ext WixUtilExtension \\
            -cultures:en-us \\
            2>&1 | Write-Output\""

    # 8. Vérification et transfert du MSI
    MSI_PATH="{{REMOTE_CORE}}/{{CRATE_DIR}}/wix/output/HeelonVault-$VERSION.msi"
    
    echo "🔎 Vérification du MSI généré..."
    ssh -i {{SSH_KEY}} -o IdentitiesOnly=yes {{VM_USER}}@{{VM_IP}} \
        "powershell -NoProfile -NonInteractive -Command \
        \"if (-not (Test-Path '$MSI_PATH')) { \\
            Write-Error 'MSI non trouvé'; \\
            exit 1 \\
        }\""

    echo "📄 MSI trouvé : $MSI_PATH"

    # 9. Transfert vers Downloads (pour compatibilité)
    echo "📦 Copie vers Downloads..."
    ssh -i {{SSH_KEY}} -o IdentitiesOnly=yes {{VM_USER}}@{{VM_IP}} \
        "powershell -NoProfile -NonInteractive -Command \
        \"Copy-Item -LiteralPath '$MSI_PATH' -Destination 'C:/Users/{{VM_USER}}/Downloads/HeelonVault.msi' -Force\""

    # 10. Transfert vers Fedora
    echo "📥 Transfert du MSI vers Fedora..."
    mkdir -p {{LOCAL_DIST}}
    scp -i {{SSH_KEY}} \
        {{VM_USER}}@{{VM_IP}}:"$MSI_PATH" \
        {{LOCAL_DIST}}/HeelonVault-$VERSION.msi

    echo ""
    echo "✅ MSI disponible : {{LOCAL_DIST}}/HeelonVault-$VERSION.msi"
    echo "📦 Taille du MSI :"
    ls -lh {{LOCAL_DIST}}/HeelonVault-$VERSION.msi

# ------------------------------------------------------------
# Pipelines combinés
# ------------------------------------------------------------
full-build: sync build-msi

rebuild-msi: clean-remote full-build

# ------------------------------------------------------------
# Installation de WiX Toolset sur la VM (si necessaire)
# ------------------------------------------------------------
install-wix:
    @echo "🔧 Installation de WiX Toolset sur la VM..."
    ssh -i {{SSH_KEY}} -o IdentitiesOnly=yes {{VM_USER}}@{{VM_IP}} \
        "powershell -NoProfile -NonInteractive -Command \
        \"if (-not (Get-Command candle.exe -ErrorAction SilentlyContinue)) { \\
            Write-Host 'Installation de WiX Toolset...'; \\
            winget install --id WiXToolset.WiXToolset --accept-package-agreements --accept-source-agreements; \\
            Write-Host 'WiX Toolset installe avec succes'; \\
        } else { \\
            Write-Host 'WiX Toolset est deja installe'; \\
        }\""

# ------------------------------------------------------------
# Test du parsing ntldd sur la VM
# ------------------------------------------------------------
test-ntldd:
    @echo "🔍 Test du parsing ntldd sur la VM..."
    ssh -i {{SSH_KEY}} -o IdentitiesOnly=yes {{VM_USER}}@{{VM_IP}} \
        "powershell -NoProfile -NonInteractive -Command \
        \"Set-Location '{{REMOTE_CORE}}/{{CRATE_DIR}}'; \\
         if (Test-Path 'target\\{{WINDOWS_TARGET}}\\release\\heelonvault.exe') { \\
             & 'C:\\msys64\\mingw64\\bin\\ntldd.exe' -R 'target\\{{WINDOWS_TARGET}}\\release\\heelonvault.exe' \\
         } else { \\
             Write-Host 'Binaire non trouve. Executez d abord : just sync + compilation manuelle' \\
         }\""
