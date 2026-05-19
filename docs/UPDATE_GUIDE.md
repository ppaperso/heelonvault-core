# Guide de Mise a Jour en Production (Rust)

Langue : FR | [EN](UPDATE_GUIDE.en.md)

Version documentée: `1.1.0`

Ce guide decrit la mise a jour de HeelonVault dans son architecture Rust-only.

## Portee

- Application: `/opt/heelonvault`
- Profil Personnel: base `~/.local/share/heelonvault/heelonvault-rust.db`, logs `~/.local/state/heelonvault/logs`
- Profil Entreprise: base `/var/lib/heelonvault/heelonvault-rust.db`, logs `/var/log/heelonvault`
- Recommandation performance Entreprise: héberger la base sur un stockage à faible latence, idéalement local au serveur d'exécution.
- Backups: `/var/backups/heelonvault`
- Legacy Python a ne jamais modifier: `/var/lib/heelonvault-shared`

## Prerequis

1. L'application est deja installee via `scripts/install.sh` (auto-détection OS), ou explicitement via `scripts/install-ubuntu.sh` / `scripts/install-rhel.sh`.
2. Vous avez les droits `sudo`.
3. Vous etes dans le dossier source de la version cible (avec `scripts/install.sh`, le binaire `heelonvault` et le dossier `migrations/`).

## Procedure de mise a jour

```bash
cd /chemin/vers/HeelonVault
sudo ./scripts/install.sh
```

Le script effectue:

1. Verification des preconditions et de l'integrite de l'artefact.
2. Detection du mode de deploiement (personnel/entreprise) et des bases existantes.
3. Backup automatique des bases detectees dans `/var/backups/heelonvault` (rotation des backups conservee).
4. Redeploiement vers `/opt/heelonvault`.
5. Regeneration du lanceur `run.sh`, integration desktop et verification des artefacts.

## Verifications post-update

```bash
# binaire present
test -x /opt/heelonvault/heelonvault && echo OK

# lanceur et entrees desktop
test -x /opt/heelonvault/run.sh
test -f /usr/share/applications/com.heelonvault.rust.desktop
test -f /usr/share/applications/heelonvault.desktop

# controle local optionnel
cd /opt/heelonvault && cargo check
```

## Restauration (rollback)

Si une mise a jour doit etre annulee:

```bash
# 1. Revenir au code/source de la version precedente
cd /chemin/vers/HeelonVault
# exemple: git checkout <tag_precedent>

# 2. Reinstaller cette version
sudo ./scripts/install.sh

# 3. Restaurer la base depuis un backup recent (choisir selon mode)
ls -lth /var/backups/heelonvault/
# personnel: heelonvault_user_<user>_backup_YYYYMMDD_HHMMSS.db
# entreprise: heelonvault_enterprise_backup_YYYYMMDD_HHMMSS.db

# 4. Relancer
/opt/heelonvault/run.sh
```

Verifications fonctionnelles recommandees:

1. Se connecter puis cliquer sur la croix de la fenêtre: l'écran de login doit réapparaître.
2. Se reconnecter immédiatement: la grille des cartes doit être rechargée.
3. Ouvrir `Profil & Sécurité` et changer la préférence d'affichage du mot de passe en édition.
4. Modifier un secret de type mot de passe pour vérifier le comportement du champ selon la préférence.
5. En tant qu'admin, ouvrir `Equipes` puis lancer un partage: un sélecteur explicite de coffre doit être proposé avant confirmation.
6. Vérifier qu'un membre de team reçoit bien le coffre partagé et peut l'ouvrir selon son rôle (READ/WRITE/ADMIN).
7. Vérifier le marquage visuel d'un coffre partagé: icône de partage visible sur les coffres partagés, sans badge texte redondant côté propriétaire/admin.
8. Enchaîner plusieurs échecs d'authentification et vérifier que l'attente augmente progressivement avant nouvelle tentative (backoff).
9. Si la 2FA est activée, vérifier qu'un code TOTP valide ne peut pas être réutilisé immédiatement (anti-rejeu).
10. Importer un CSV de test et vérifier que les URL non `http/https`, les fichiers trop volumineux et les champs anormalement longs sont rejetés.
11. Après export backup/restauration, vérifier les permissions Linux avec `stat -c "%a %n" /chemin/backup.hvb` et `stat -c "%a %n" /chemin/heelonvault-rust.db` (valeur attendue: `600`).
12. Changer le mot de passe maître, puis vérifier l'accès aux coffres principaux après reconnexion (rotation master key durcie).
13. Vérifier le flux CSV en 3 étapes (prévisualisation, progression, résumé) et, en cas de rejet, noter le chemin `csv_import_rejects_*.txt` indiqué dans le résumé.

## Bonnes pratiques

- Toujours lancer `scripts/install.sh` (ou le wrapper OS explicite) depuis le code source cible.
- Verifier l'espace disque avant mise a jour (`df -h /var/backups`).
- Ne pas modifier les donnees pendant la mise a jour.
- Conserver plusieurs backups recents avant nettoyage manuel.

## A ne pas faire

- Ne pas reutiliser d'anciennes procedures `venv`/`pip`.
- Ne pas modifier les anciens chemins Python (`/var/lib/heelonvault-shared`).
- Ne pas contourner les erreurs backup: un echec backup doit bloquer la mise a jour.
