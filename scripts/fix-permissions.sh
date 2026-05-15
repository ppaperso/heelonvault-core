#!/bin/bash
# Script de réparation des permissions après update.sh défectueux
# Restaure les ACL pour permettre l'accès aux utilisateurs

set -e

DATA_DIR="/var/lib/heelonvault-rust-shared"

echo "🔧 Réparation des permissions et ACL..."
echo ""

# Vérifier root
if [ "$(id -u)" -ne 0 ]; then
    echo "❌ Ce script doit être lancé avec sudo"
    exit 1
fi

# Détecter l'utilisateur qui a lancé sudo
if [ -z "$SUDO_USER" ]; then
    echo "⚠️  Variable SUDO_USER non définie"
    read -p "Entrez le nom d'utilisateur à autoriser: " USERNAME
else
    USERNAME="$SUDO_USER"
fi

echo "👤 Utilisateur: $USERNAME"
echo ""

# Vérifier que setfacl est disponible
if ! command -v setfacl >/dev/null 2>&1; then
    echo "❌ setfacl non trouvé. Installez le paquet 'acl':"
    echo "   sudo dnf install acl"
    exit 1
fi

# Restaurer les permissions de base
echo "1️⃣ Restauration des permissions de base..."
chmod 750 "$DATA_DIR"
find "$DATA_DIR" -type f -user root -exec chmod 664 {} \; 2>/dev/null || true
echo "   ✅ Permissions de base restaurées"
echo ""

# Restaurer les ACL sur le répertoire
echo "2️⃣ Restauration des ACL sur le répertoire..."
setfacl -m "u:${USERNAME}:rwx" "$DATA_DIR"
setfacl -d -m "u:${USERNAME}:rwx" "$DATA_DIR"
setfacl -d -m "g::rwx" "$DATA_DIR"
echo "   ✅ ACL du répertoire restaurées"
echo ""

# Restaurer les ACL sur les fichiers existants
echo "3️⃣ Restauration des ACL sur les fichiers..."
find "$DATA_DIR" -type f -name "*.db" -exec setfacl -m "u:${USERNAME}:rw" {} \;
find "$DATA_DIR" -type f -name "salt_*.bin" -exec setfacl -m "u:${USERNAME}:r" {} \;
echo "   ✅ ACL des fichiers restaurées"
echo ""

# Détecter et réparer pour les autres utilisateurs
echo "4️⃣ Détection des autres utilisateurs..."
OTHER_USERS=$(getfacl "$DATA_DIR" 2>/dev/null | grep "^user:" | grep -v "^user::" | grep -v "^user:${USERNAME}" | cut -d: -f2 || true)

if [ -n "$OTHER_USERS" ]; then
    echo "   Autres utilisateurs détectés:"
    for user in $OTHER_USERS; do
        echo "   • $user"
        setfacl -m "u:${user}:rwx" "$DATA_DIR" 2>/dev/null || true
        find "$DATA_DIR" -type f -name "*.db" -exec setfacl -m "u:${user}:rw" {} \; 2>/dev/null || true
        find "$DATA_DIR" -type f -name "salt_*.bin" -exec setfacl -m "u:${user}:r" {} \; 2>/dev/null || true
    done
    echo "   ✅ ACL des autres utilisateurs restaurées"
else
    echo "   ℹ️  Aucun autre utilisateur détecté"
fi
echo ""

# Vérifier le résultat
echo "5️⃣ Vérification finale..."
echo ""
echo "ACL du répertoire $DATA_DIR :"
getfacl "$DATA_DIR" | grep -E "user:|group:|mask:" | sed 's/^/   /'
echo ""
echo "ACL de users.db :"
getfacl "$DATA_DIR/users.db" | grep -E "user:|group:|mask:" | sed 's/^/   /'
echo ""

# Test d'accès
echo "6️⃣ Test d'accès pour $USERNAME..."
if sudo -u "$USERNAME" test -r "$DATA_DIR/users.db"; then
    echo "   ✅ Accès en lecture OK"
else
    echo "   ❌ Accès en lecture ÉCHOUÉ"
    exit 1
fi

if sudo -u "$USERNAME" test -w "$DATA_DIR/users.db"; then
    echo "   ✅ Accès en écriture OK"
else
    echo "   ⚠️  Accès en écriture ÉCHOUÉ (peut nécessiter une déconnexion/reconnexion)"
fi
echo ""

echo "╔══════════════════════════════════════════════════════════════╗"
echo "║             ✅ RÉPARATION TERMINÉE !                         ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""
echo "🎯 Vous pouvez maintenant relancer l'application:"
echo "   /opt/heelonvault/run.sh"
echo ""
echo "📝 Si l'accès en écriture échoue toujours:"
echo "   1. Déconnectez-vous de votre session"
echo "   2. Reconnectez-vous"
echo "   3. Relancez l'application"
echo ""
