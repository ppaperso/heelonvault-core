# Updating and Deployment

Language: EN | [FR](UPDATE_GUIDE.md)

This guide explains how to move to a new HeelonVault version and, for administrators, how to
perform a Linux system-wide install. For a first install on a computer, see
[QUICKSTART.md](QUICKSTART.md).

## Before any update

1. Make an **encrypted `.hvb` export** of your vaults from **Profile & Security > Data
   management**, and keep it off the computer.
2. Note the installed version (header of the [changelog](CHANGELOG.en.md)) and read the sections
   of the versions in between, especially "Breaking changes".
3. Close HeelonVault.

Database schema changes are applied **automatically** on the first launch of the new version.
They cannot be undone: going back to an older version requires restoring a backup made before
the update.

## Updating a computer

Download and verify the new package as for a first install
([sections 1 and 2](QUICKSTART.md#1-download) of the quickstart), then:

| System | Procedure |
| ------ | --------- |
| Windows | Run the new `.msi`. It replaces the installed version; no need to uninstall first. An `.msi` older than the installed version is refused. |
| macOS | Drag the new application into **Applications** and choose **Replace**. Redo the Gatekeeper first-launch step. |
| Linux (AppImage) | Replace the old `.AppImage` file with the new one, then make it executable (`chmod +x`). |

Replacing the application does not touch your data: it lives in a separate folder (see
[Where is my data?](QUICKSTART.md#6-where-is-my-data)).

### AppImage: data created with version 2.0.0

The 2.0.0 AppImage stored its database in a `data/` folder relative to the folder it was
launched from (often the home folder: `~/data/heelonvault-rust-dev.db`). Later versions use
`~/.local/share/heelonvault/heelonvault-rust.db`. On first launch, if that new file does not
exist yet, the AppImage automatically **copies** the old database found in the launch folder or
in the home folder. The old file is left in place: delete it yourself once you have checked your
secrets.

## Linux system install

For administrators who want an install in `/opt/heelonvault` with application-menu integration,
or a shared database on a server. For a single computer, the AppImage is enough.

The install scripts detect the distribution (Debian/Ubuntu family or Fedora/RHEL/Rocky/AlmaLinux),
install the system dependencies and offer two profiles:

| Profile | Database | Logs |
| ------- | -------- | ---- |
| **Personal** (default) | `~/.local/share/heelonvault/heelonvault-rust.db` | `~/.local/state/heelonvault/logs/` |
| **Enterprise** | `/var/lib/heelonvault/heelonvault-rust.db` | `/var/log/heelonvault/` |

The Personal profile shares the AppImage's database: both can coexist.

### Install

Prerequisite: a `heelonvault-core` checkout at the desired tag, with the `heelonvault` binary
built in release mode and copied to the repository root (see the
[development documentation](internal/DEVELOPMENT.md)).

```bash
# Preview, without changing anything
sudo env HEELONVAULT_DRY_RUN=1 ./scripts/install.sh

# Install
sudo ./scripts/install.sh
```

The script checks the binary's integrity when a `heelonvault.sha256` file sits next to it, copies
the application and its migrations to `/opt/heelonvault`, and generates the `run.sh` launcher and
the `com.heelonvault.rust.desktop` menu entry. `install-ubuntu.sh` and `install-rhel.sh` force a
distribution family.

With the Enterprise profile, the script only sets up the shared paths: network publishing (RDS,
VDI, RemoteApp, bastion…) is up to you. Keep the database on low-latency storage, ideally local
to the server running the application.

### Update

From the checkout at the new tag, with the new binary at the root:

```bash
sudo ./scripts/install.sh
```

Before redeploying, the script backs up the detected databases to `/var/backups/heelonvault`
(`heelonvault_user_<user>_backup_YYYYMMDD_HHMMSS.db` or
`heelonvault_enterprise_backup_YYYYMMDD_HHMMSS.db`). A failed backup stops the update: do not work
around it. Check disk space first (`df -h /var/backups`).

Post-update check:

```bash
test -x /opt/heelonvault/heelonvault && test -x /opt/heelonvault/run.sh && echo OK
stat -c "%a %n" ~/.local/share/heelonvault/heelonvault-rust.db   # expected: 600
```

### Roll back

```bash
# 1. Reinstall the previous version (checkout at the previous tag, matching binary)
sudo ./scripts/install.sh

# 2. Restore the backup made before the update
ls -lth /var/backups/heelonvault/
cp /var/backups/heelonvault/<backup>.db ~/.local/share/heelonvault/heelonvault-rust.db
```

### Uninstall

```bash
sudo ./scripts/remove.sh
```

Data and backups are only deleted if you explicitly confirm it.

## Migrating from the old Python version (0.4)

HeelonVault never reads or modifies the old Python application's data. To bring it over, export
it to CSV, then import that file from **Profile & Security > Data management > Import data
(CSV)**:

```bash
# Data in the default folder (~/.local/share/passwordmanager), given profile
./scripts/export-legacy-v0.4-to-csv.py --profile <profile> --output legacy_export.csv

# Or explicit paths (shared deployment, for example under /var/lib/heelonvault-shared)
./scripts/export-legacy-v0.4-to-csv.py \
  --db-path <path>/passwords_<profile>.db \
  --salt-path <path>/salt_<profile>.bin \
  --output legacy_export.csv
```

The CSV file holds your passwords **in clear text**: delete it securely as soon as the import is
done.
