# ============================================================
# HeelonVault — Justfile Minimal
# Synchronisation avec VM Windows de test
# ============================================================

# ------------------------------------------------------------
# Configuration VM Windows
# ------------------------------------------------------------
VM_IP := "192.168.122.47"
VM_USER := "builduser"
SSH_KEY := "~/.ssh/id_ed25519"

PROJECT_ROOT := ".."
REMOTE_ROOT := "C:/Users/" + VM_USER + "/build/heelonvault"
REMOTE_CORE := REMOTE_ROOT + "/heelonvault-core"

# ------------------------------------------------------------
# Default
# ------------------------------------------------------------
@default:
    just --list

# ------------------------------------------------------------
# Nettoyage du workspace Windows
# ------------------------------------------------------------
@clean-remote:
    echo "🧹 Nettoyage du workspace Windows..."
    ssh -i {{SSH_KEY}} -o IdentitiesOnly=yes {{VM_USER}}@{{VM_IP}} "pwsh -NoProfile -NonInteractive -Command \"if (Test-Path '{{REMOTE_ROOT}}') { Remove-Item -Recurse -Force '{{REMOTE_ROOT}}' }; New-Item -ItemType Directory -Force -Path '{{REMOTE_ROOT}}'\""
    echo "✅ Workspace Windows nettoyé"

# ------------------------------------------------------------
# Synchronisation des sources vers la VM
# ------------------------------------------------------------
@sync:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "📦 Synchronisation des sources vers la VM..."
    
    # Créer le répertoire sur la VM
    ssh -i {{SSH_KEY}} -o IdentitiesOnly=yes {{VM_USER}}@{{VM_IP}} "pwsh -NoProfile -NonInteractive -Command \"New-Item -ItemType Directory -Force -Path '{{REMOTE_ROOT}}'\""
    
    # Créer l'archive locale
    echo "   → Création de l'archive locale..."
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
    
    # Transfert vers la VM
    echo "   → Transfert vers la VM..."
    scp -i {{SSH_KEY}} /tmp/heelonvault-sources.tar.gz {{VM_USER}}@{{VM_IP}}:C:/Users/{{VM_USER}}/Downloads/
    
    # Extraction sur la VM
    echo "   → Extraction sur la VM..."
    ssh -i {{SSH_KEY}} -o IdentitiesOnly=yes {{VM_USER}}@{{VM_IP}} "pwsh -NoProfile -NonInteractive -Command \"Set-Location '{{REMOTE_ROOT}}'; tar -xzf 'C:/Users/{{VM_USER}}/Downloads/heelonvault-sources.tar.gz'\""
    
    # Nettoyage local
    rm -f /tmp/heelonvault-sources.tar.gz
    
    echo "✅ Sources synchronisées avec la VM"
