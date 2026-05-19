# Production Update Guide (Rust)

Language: EN | [FR](UPDATE_GUIDE.md)

Documented version: `1.1.0`

This guide explains how to update HeelonVault in its Rust-only architecture.

## Scope

- Application: `/opt/heelonvault`
- Personal profile: DB `~/.local/share/heelonvault/heelonvault-rust.db`, logs `~/.local/state/heelonvault/logs`
- Enterprise profile: DB `/var/lib/heelonvault/heelonvault-rust.db`, logs `/var/log/heelonvault`
- Enterprise performance recommendation: host the database on low-latency storage, ideally local to the execution server.
- Backups: `/var/backups/heelonvault`
- Legacy Python path (do not modify): `/var/lib/heelonvault-shared`

## Prerequisites

1. Application already installed with `scripts/install.sh` (OS auto-detection), or explicitly with `scripts/install-ubuntu.sh` / `scripts/install-rhel.sh`.
2. `sudo` access.
3. You are in the target-version source folder (with `scripts/install.sh`, the `heelonvault` binary, and `migrations/`).

## Update Procedure

```bash
cd /path/to/HeelonVault
sudo ./scripts/install.sh
```

The script performs:

1. precondition checks and artifact integrity verification;
2. deployment mode detection (personal/enterprise) and existing database detection;
3. automatic database backups in `/var/backups/heelonvault` (with backup rotation);
4. redeploy to `/opt/heelonvault`;
5. `run.sh` regeneration, desktop integration, and artifact validation.

## Post-update checks

```bash
# binary present
test -x /opt/heelonvault/heelonvault && echo OK

# launcher and desktop entries
test -x /opt/heelonvault/run.sh
test -f /usr/share/applications/com.heelonvault.rust.desktop
test -f /usr/share/applications/heelonvault.desktop

# optional local sanity check
cd /opt/heelonvault && cargo check
```

Recommended functional checks:

1. Login, then close main window with title-bar close button: login screen should reappear.
2. Re-login immediately: secret cards should reload.
3. Open Profile and Security and change password-visibility preference.
4. Edit a password secret and verify field behavior matches preference.
5. As admin, open Teams and start a share action: an explicit vault selector must be shown before confirmation.
6. Verify that a team member receives the shared vault and can open it according to assigned role (READ/WRITE/ADMIN).
7. Verify shared-state visual marker behavior: shared icon visible on shared vaults, without redundant owner/admin text badge.
8. Trigger repeated authentication failures and verify retry delay increases progressively before next attempt (backoff).
9. When 2FA is enabled, verify a valid TOTP code cannot be reused immediately (replay guard).
10. Import a test CSV and verify non-`http/https` URLs, oversized files, and abnormally long fields are rejected.
11. After backup export/restore, verify Linux file permissions with `stat -c "%a %n" /path/to/backup.hvb` and `stat -c "%a %n" /path/to/heelonvault-rust.db` (expected value: `600`).
12. Change the master password, then verify access to main vaults after re-login (hardened master-key rotation).
13. Verify the 3-step CSV flow (preview, progress, summary) and, when rows are rejected, record the `csv_import_rejects_*.txt` path shown in the summary.

## Rollback

```bash
# 1. Switch to previous source/release version
cd /path/to/HeelonVault
# example: git checkout <previous_tag>

# 2. Reinstall that version
sudo ./scripts/install.sh

# 3. Restore DB from a recent backup (choose by deployment mode)
ls -lth /var/backups/heelonvault/
# personal: heelonvault_user_<user>_backup_YYYYMMDD_HHMMSS.db
# enterprise: heelonvault_enterprise_backup_YYYYMMDD_HHMMSS.db

# 4. Relaunch
/opt/heelonvault/run.sh
```

## Best Practices

- run `scripts/install.sh` (or explicit OS wrapper) from the target source version;
- check free space before update (`df -h /var/backups`);
- avoid modifying data during update;
- keep multiple recent backups before cleanup.

## Do Not

- do not use legacy `venv`/`pip` procedures;
- do not modify legacy Python paths;
- do not bypass backup errors.
