# Quickstart (Rust)

Language: EN | [FR](QUICKSTART.fr.md)

Documented quickstart version: `1.2.0-rc.1`

---

## Prerequisites

- **Rust Toolchain**: `1.98.0` (pinned via `rust-toolchain.toml`)
- **GTK4**: Required for the desktop UI
- **libadwaita**: GTK4 companion library
- **SQLite**: Backend database

Verify your environment:

```bash
rustc --version  # Should be 1.98.0
cargo --version
```

---

## 1. Build Check

Verify the workspace compiles without errors:

```bash
cargo check --workspace
```

For a full build with optimizations:

```bash
cargo build --workspace
```

---

## 2. Run in Development

From repository root:

```bash
./scripts/run-dev.sh
```

**Development environment specifics**:
- Development database path: `data/heelonvault-rust-dev.db`
- Log level: `debug` (via `HEELONVAULT_LOG_LEVEL=debug`)
- Log directory: `./logs`
- Database is created automatically on first run

**Environment variables** (optional):

```bash
# Override log level
HEELONVAULT_LOG_LEVEL=trace ./scripts/run-dev.sh

# Override log directory
HEELONVAULT_LOG_DIR=/tmp/heelonvault-logs ./scripts/run-dev.sh

# Override database path
HEELONVAULT_DB_PATH=/tmp/heelonvault-dev.db ./scripts/run-dev.sh
```

---

## 3. Run Tests

### Unit and Integration Tests

Run all tests:

```bash
cargo test --workspace
```

Run specific test modules:

```bash
# Repository tests
cargo test secret_repository:: -- --nocapture
cargo test user_repository:: -- --nocapture

# Service tests
cargo test secret_service:: -- --nocapture
cargo test auth_service:: -- --nocapture

# Integration tests
cargo test --workspace --test login_history_integration
cargo test --workspace --test account_rekey_integration  # NEW in v1.2.0-rc.1
```

### Clippy Linting

Ensure code quality:

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

---

## 3bis. Recommended UI Checks (v1.2.0-rc.1)

### Session and PIN Features (NEW)

1. **PIN Setup**:
   - Open `Profile & Security` from the sidebar
   - Set up a 4-8 digit PIN code
   - Verify the PIN badge appears in the header bar
   - Test auto-lock: the PIN unlock dialog should appear

2. **PIN Badge and Timer**:
   - Verify the PIN badge shows correct state (nominal, warning, critical)
   - Hover over the badge to see the session countdown tooltip
   - Verify the badge text is readable against the dark header bar

3. **Session Lifecycle**:
   - Close the main window from the title bar close button: the login screen should reappear
   - Re-login immediately: secret cards should be visible
   - Verify auto-lock triggers PIN unlock (when PIN is set)

### Card Navigation and Shortcuts

4. **Card Selection**:
   - Single-click a secret card: it should become active without opening edit mode
   - Verify the active card has a visible selection highlight

5. **Card Editing**:
   - Double-click the same card: the edit form should open
   - Verify all fields are populated correctly

6. **Keyboard Shortcuts**:
   - On the active card, test quick shortcuts:
     - `Ctrl+C`: copy secret value to clipboard
     - `Ctrl+L`: copy login value to clipboard
     - `Ctrl+U`: open URL in default browser
   - Verify clipboard auto-clear after 60 seconds

### Search and MultiVault (NEW in v1.2.0-rc.1)

7. **Search Functionality**:
   - Use the search bar to find secrets by title, login, email, URL, notes, category, tags, or type
   - Test fielded syntax: `email:`, `tag:`, `type:`
   - Test space-after-colon syntax: `field: value` == `field:value`

8. **MultiVault Toggle**:
   - Click the **MultiVault** toggle button (left of search bar)
   - Verify search works across all vaults when enabled
   - Verify search is limited to active vault when disabled

9. **Health Marker**:
   - In create/edit, toggle "Health data access"
   - Save the secret
   - Verify search with `#sante` finds the marked secret
   - Verify the "Sante" badge appears on matching cards

### Import and Recovery (NEW in v1.2.0-rc.1)

10. **CSV Import** (Premium feature):
    - Navigate to `Profile & Security` > Import
    - Select a CSV file
    - Verify 3-step flow: preview, progress, final summary
    - Verify error tolerance: partial imports should work
    - Check `HEELONVAULT_LOG_DIR` for `csv_import_rejects_*.txt` if rows are rejected

11. **Account Recovery** (NEW in v1.2.0-rc.1):
    - During bootstrap: verify recovery key generation (24-word BIP39 phrase)
    - Verify mandatory spot-check of 2 random words
    - Verify clipboard copy with 60-second auto-clear
    - From `Profile & Security`: verify recovery key re-export

---

## 4. Production Build

Build for release:

```bash
cargo build --release
```

The binary will be created at:

```bash
./target/release/heelonvault
```

### Packaged Linux Installer

The tarball (`heelonvault-linux-x86_64.tar.gz`) deploys:

- **Binary path**: `/opt/heelonvault/heelonvault`
- **Launcher**: `/opt/heelonvault/run.sh`
- **Desktop entry**: `/usr/share/applications/com.heelonvault.rust.desktop`
- **Legacy desktop entry**: `/usr/share/applications/heelonvault.desktop`
- **User database path**: `~/.local/share/heelonvault/heelonvault-rust.db`
- **User logs path**: `~/.local/state/heelonvault/logs`
- **Migrations directory**: `/opt/heelonvault/migrations` (19 migrations in v1.2.0-rc.1)

Installation:

```bash
tar -xzf heelonvault-linux-x86_64.tar.gz
cd heelonvault-linux-x86_64
sudo ./scripts/install.sh
```

### Production Environment Variables

The generated `run.sh` exports:

```bash
HEELONVAULT_MIGRATIONS_DIR=/opt/heelonvault/migrations
HEELONVAULT_DB_PATH=~/.local/share/heelonvault/heelonvault-rust.db
HEELONVAULT_LOG_DIR=~/.local/state/heelonvault/logs
HEELONVAULT_LOG_LEVEL=info
```

---

## Post-Installation Sanity Checks (Ubuntu)

### Binary and Launcher Checks

```bash
# Verify binary exists and is executable
test -x /opt/heelonvault/heelonvault

# Verify launcher exists and is executable
test -x /opt/heelonvault/run.sh

# Verify desktop entries exist
test -f /usr/share/applications/com.heelonvault.rust.desktop
test -f /usr/share/applications/heelonvault.desktop

# Validate desktop entry format
desktop-file-validate /usr/share/applications/com.heelonvault.rust.desktop

# Test launching via desktop entry
gtk-launch com.heelonvault.rust
```

### Database and Migrations Checks

```bash
# Verify database directory exists
ls -la ~/.local/share/heelonvault/

# Verify migrations directory and files (19 migrations in v1.2.0-rc.1)
ls -la /opt/heelonvault/migrations/ | wc -l  # Should be 19 + 1 (header)

# Verify each migration SQL file content
grep -c "CREATE TABLE\|ALTER TABLE\|CREATE INDEX" /opt/heelonvault/migrations/*.sql
```

### PIN and Session Checks (NEW in v1.2.0-rc.1)

```bash
# Verify PIN-related database tables
sqlite3 ~/.local/share/heelonvault/heelonvault-rust.db "SELECT name FROM sqlite_master WHERE type='table' AND name LIKE '%pin%';"

# Verify rate limiting tables (IP-based)
sqlite3 ~/.local/share/heelonvault/heelonvault-rust.db "SELECT name FROM sqlite_master WHERE type='table' AND name='login_attempts_ip';"

# Verify recovery key envelope table (NEW in v1.2.0-rc.1)
sqlite3 ~/.local/share/heelonvault/heelonvault-rust.db "SELECT name FROM sqlite_master WHERE type='table' AND name='user_recovery_key_envelopes';"
```

### Permissions Checks

```bash
# Verify database permissions (should be 0600)
stat -c "%a" ~/.local/share/heelonvault/heelonvault-rust.db

# Verify log directory permissions
stat -c "%a" ~/.local/state/heelonvault/
```

---

## Legacy Upgrade Notes

### From v0.4 to v1.2.0-rc.1

Older installers may have stored the database in `/opt/heelonvault/data/heelonvault-rust-dev.db`. 
The packaged launcher copies that file into the user data directory on first launch when needed.

For manual migration:

```bash
# Use the provided migration script
./scripts/export-legacy-v0.4-to-csv.py --db-path /var/lib/heelonvault-shared/old.db \
  --salt-path /var/lib/heelonvault-shared/salt.txt \
  --output legacy_export.csv

# Then import via CSV import flow (Premium feature)
```

### From v1.1.0 to v1.2.0-rc.1

The database schema has been updated with 5 new migrations (14 -> 19 total):

- Migration 0015: Additional indexes for performance
- Migration 0016: IP-based rate limiting table (`login_attempts_ip`)
- Migration 0017: PIN cache table
- Migration 0018: Session state table
- Migration 0019: User recovery key envelope table (Account Key Recovery)

These migrations are applied automatically on first run.

---

## Troubleshooting

### Common Issues

**Issue: GSK_RENDERER not set**

Solution: Ensure GTK rendering variables are set before Tokio initialization:

```bash
# Check if variable is exported
echo $GSK_RENDERER

# Run with explicit renderer
gsk_renderer=gl ./scripts/run-dev.sh
```

**Issue: Database already exists with older schema**

Solution: Backup and let migrations run:

```bash
mv data/heelonvault-rust-dev.db data/heelonvault-rust-dev.db.bak
./scripts/run-dev.sh  # Will create fresh DB with current schema
```

**Issue: Missing GTK4 dependencies**

Solution (Ubuntu/Debian):

```bash
sudo apt-get install libgtk-4-dev libadwaita-1-dev
```

**Issue: Logs not appearing**

Solution: Check environment variables:

```bash
# Verify log directory exists
mkdir -p ./logs

# Run with explicit log settings
HEELONVAULT_LOG_LEVEL=debug HEELONVAULT_LOG_DIR=./logs ./scripts/run-dev.sh
```

---

## Additional Resources

- [Full Documentation Index](../README.md)
- [Architecture Details](ARCHITECTURE.md)
- [User Guide](USER_GUIDE.md)
- [Changelog](CHANGELOG.md)

---

> **Note**: For premium features (CSV import, team sharing, etc.), ensure your license is properly configured in `~/.config/heelonvault/license.hvl` (dev) or `/etc/heelonvault/license.hvl` (prod).
