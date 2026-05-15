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
3. Le toolchain Rust est disponible (`cargo`).
4. Vous etes dans le dossier source qui contient `update.sh`.

## Procedure de mise a jour

```bash
cd /chemin/vers/HeelonVault
sudo bash update.sh
```

Le script effectue:

1. Verification des preconditions (`sudo`, `cargo`, dossier d'installation).
2. Backup de `/opt/heelonvault`.
3. Verification d'integrite de l'archive backup.
4. Synchronisation des fichiers source vers `/opt/heelonvault` via `rsync`.
5. Build release Rust (`cargo build --release`).

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
# 1. Identifier le backup cible
ls -lth /var/backups/heelonvault/

# 2. Restaurer installation
sudo tar -xzf /var/backups/heelonvault/heelonvault_YYYYMMDD_HHMMSS.tar.gz -C /

# 3. Relancer
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

## Bonnes pratiques

- Toujours lancer `update.sh` depuis le code source cible.
- Verifier l'espace disque avant mise a jour (`df -h /var/backups`).
- Ne pas modifier les donnees pendant la mise a jour.
- Conserver plusieurs backups recents avant nettoyage manuel.

## A ne pas faire

- Ne pas reutiliser d'anciennes procedures `venv`/`pip`.
- Ne pas modifier les anciens chemins Python (`/var/lib/heelonvault-shared`).
- Ne pas contourner les erreurs backup: un echec backup doit bloquer l'update.
