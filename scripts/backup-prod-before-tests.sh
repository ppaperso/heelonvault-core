#!/bin/bash
# Script de backup de sécurité AVANT tests
# NE PAS MODIFIER - Usage: ./backup-prod-before-tests.sh

set -e

PROD_DIR="/var/lib/heelonvault-rust-shared"
BACKUP_BASE="$HOME/backups/heelonvault-prod"
TIMESTAMP=$(date +%Y%m%d-%H%M%S)
BACKUP_DIR="${BACKUP_BASE}/backup-${TIMESTAMP}"

echo "🔒 BACKUP DE SÉCURITÉ - PRODUCTION"
echo "===================================="
echo ""

# Vérifier que le répertoire de production existe
if [ ! -d "$PROD_DIR" ]; then
    echo "⚠️  Le répertoire de production n'existe pas : $PROD_DIR"
    echo "   Rien à sauvegarder."
    exit 0
fi

# Vérifier qu'il contient des fichiers
if [ -z "$(ls -A $PROD_DIR)" ]; then
    echo "⚠️  Le répertoire de production est vide : $PROD_DIR"
    echo "   Rien à sauvegarder."
    exit 0
fi

# Créer le répertoire de backup
mkdir -p "$BACKUP_DIR"

echo "📂 Source      : $PROD_DIR"
echo "💾 Destination : $BACKUP_DIR"
echo ""

# Copier tous les fichiers
echo "📋 Copie des fichiers..."
cp -r "$PROD_DIR"/* "$BACKUP_DIR/" 2>/dev/null || {
    echo "❌ Erreur lors de la copie des fichiers"
    exit 1
}

# Lister les fichiers sauvegardés
echo ""
echo "✅ Fichiers sauvegardés :"
ls -lh "$BACKUP_DIR"

# Créer une archive compressée
echo ""
echo "📦 Création de l'archive..."
ARCHIVE="${BACKUP_BASE}/backup-${TIMESTAMP}.tar.gz"
tar czf "$ARCHIVE" -C "$BACKUP_DIR/.." "$(basename $BACKUP_DIR)"

# Vérifier l'archive
if [ -f "$ARCHIVE" ]; then
    ARCHIVE_SIZE=$(du -h "$ARCHIVE" | cut -f1)
    echo "✅ Archive créée : $ARCHIVE ($ARCHIVE_SIZE)"
    
    # Supprimer le répertoire temporaire
    rm -rf "$BACKUP_DIR"
else
    echo "❌ Erreur lors de la création de l'archive"
    exit 1
fi

# Garder seulement les 10 derniers backups
echo ""
echo "🧹 Nettoyage des anciens backups (conservation des 10 derniers)..."
cd "$BACKUP_BASE"
ls -t backup-*.tar.gz | tail -n +11 | xargs -r rm -v

echo ""
echo "===================================="
echo "✅ BACKUP TERMINÉ AVEC SUCCÈS"
echo "===================================="
echo ""
echo "Pour restaurer ce backup :"
echo "  tar xzf $ARCHIVE -C /tmp/"
echo "  sudo cp -r /tmp/backup-${TIMESTAMP}/* $PROD_DIR/"
echo ""
