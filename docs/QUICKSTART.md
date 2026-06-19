# Quickstart (Rust)

Language: EN | [FR](QUICKSTART.fr.md)

Documented quickstart version: `1.1.0`

## 1. Build Check

```bash
cargo check --workspace
```

## 2. Run in Development

From repository root:

```bash
./scripts/run-dev.sh
```

Development database path:

- `data/heelonvault-rust-dev.db`

## 3. Run Tests

```bash
cargo test secret_repository:: -- --nocapture
cargo test secret_service:: -- --nocapture
cargo test --workspace --test login_history_integration
```

## 3bis. Recommended UI checks

1. Open `Profile & Security` from the sidebar.
2. Close the main window from the title bar close button: the login screen should reappear.
3. Re-login immediately: secret cards should be visible.
4. Single-click a secret card: it should become active without opening edit mode.
5. Double-click the same card: the edit form should open.
6. On the active card, test quick shortcuts: `Ctrl+C`, `Ctrl+L`, and `Ctrl+U`.
7. In create/edit, toggle "Health data access", save, then verify search with `#sante`.

## 4. Production Build

```bash
cargo build --release
```

The packaged Linux installer deploys:

- Binary path: `/opt/heelonvault/heelonvault`
- Launcher: `/opt/heelonvault/run.sh`
- Desktop entry: `/usr/share/applications/com.heelonvault.rust.desktop`
- Legacy desktop entry: `/usr/share/applications/heelonvault.desktop`
- User database path: `~/.local/share/heelonvault/heelonvault-rust.db`
- User logs path: `~/.local/state/heelonvault/logs`

Post-install sanity checks (Ubuntu):

```bash
test -x /opt/heelonvault/heelonvault
test -x /opt/heelonvault/run.sh
test -f /usr/share/applications/com.heelonvault.rust.desktop
test -f /usr/share/applications/heelonvault.desktop
desktop-file-validate /usr/share/applications/com.heelonvault.rust.desktop
gtk-launch com.heelonvault.rust
```

Legacy upgrade note:

- Older installers may have stored the database in `/opt/heelonvault/data/heelonvault-rust-dev.db`; the packaged launcher copies that file into the user data directory on first launch when needed.
