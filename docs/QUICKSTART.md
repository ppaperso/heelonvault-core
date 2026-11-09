# Installation and Quickstart

Language: EN | [FR](QUICKSTART.fr.md)

This guide covers installing HeelonVault on your computer and the first launch, up to your
first saved secret. Allow about ten minutes.

**Current platforms: Linux first, then Windows 11.** macOS development and distribution are
temporarily paused because of known window-control and secret clipboard-copy issues. No new
DMGs are built or published, including through manual CI/CD runs; existing DMGs are not
recommended for production. Resumption will be reassessed after Linux and Windows 11 have
been validated on other machines and in production, with no announced date.

## 1. Download

Packages are published on the
[GitHub Releases](https://github.com/ppaperso/heelonvault-core/releases/latest) page. Pick the
file for your system:

| System | File | Requirements |
| ------ | ---- | ------------ |
| Linux | `heelonvault-linux-x86_64-vX.Y.Z.AppImage` | Recent 64-bit distribution (x86_64), Wayland session; X11/XWayland not certified |
| Windows 11 | `heelonvault-windows-x86_64-vX.Y.Z.msi` | 64-bit, administrator rights to install |

Also download the matching `.sha256` file: it lets you check that the package was not tampered
with.

## 2. Verify the download

Put the package and its `.sha256` file in the same folder, then:

- **Linux**: `sha256sum -c heelonvault-linux-x86_64-vX.Y.Z.AppImage.sha256`
- **Windows** (PowerShell): compare the value printed by
  `Get-FileHash .\heelonvault-windows-x86_64-vX.Y.Z.msi -Algorithm SHA256`
  with the content of the `.sha256` file.

The command must answer `OK` (Linux) or print the same hash (Windows). If it does not,
do not run the package and download it again.

## 3. Install

### Windows

1. Double-click the `.msi` file.
2. The package is not code-signed: if SmartScreen shows "Windows protected your PC", click
   **More info**, then **Run anyway**.
3. Accept the elevation prompt: the application is installed in `C:\Program Files\HeelonVault`
   for every user of the computer.
4. Start **HeelonVault** from the Start menu. The very first launch can take a few seconds while
   Windows Defender scans the application.

### macOS

Installation is not offered during the pause. The macOS paths below are retained only for
users of older packages; they do not constitute platform validation.

### Linux (AppImage)

The AppImage bundles GTK4 and libadwaita: nothing else to install.

```bash
chmod +x heelonvault-linux-x86_64-vX.Y.Z.AppImage
./heelonvault-linux-x86_64-vX.Y.Z.AppImage
```

If launching fails with an error mentioning FUSE, install the `libfuse2` library (Ubuntu 24.04:
`sudo apt install libfuse2t64`; Fedora: `sudo dnf install fuse-libs`).

To get HeelonVault into your application menu, keep the AppImage in a stable folder (for example
`~/Applications`) and use an integration tool such as Gear Lever or AppImageLauncher.

> Administrators: a Linux system-wide install (personal or enterprise profile, shared database)
> is described in [UPDATE_GUIDE.en.md](UPDATE_GUIDE.en.md#linux-system-install).

## 4. First launch: create your account

On first start, a wizard creates the vault's administrator account:

1. Choose a **username** and a strong **master password**. It is the only password you need to
   remember: the publisher cannot recover it.
2. Write down the **24-word recovery phrase** shown. It lets you set a new master password if you
   forget it. Keep it away from the computer (on paper, stored somewhere safe).
3. Confirm two randomly chosen words to prove the phrase was written down.
4. The wizard closes and the sign-in screen opens with your username pre-filled: sign in.

> Without the master password or the recovery phrase, your secrets are **unrecoverable**. That
> is the price of encryption nobody but you can open.

## 5. First steps

- **Create a secret**: **Add** button in the top-right corner, then pick the type (password, API
  key, SSH key, secure document…). The **+** button in the sidebar creates a new vault instead.
- **Copy a password**: select the card, then `Ctrl+C`. The clipboard is cleared automatically
  after 20 seconds; the clipboard icon in the header bar shows the countdown, and clicking it
  clears the clipboard at once.
- **Secure your session**: in **Profile & Security**, enable two-factor authentication (TOTP),
  set the auto-lock delay and, if you like, a quick-unlock PIN.
- **Import your passwords**: **Profile & Security > Data management > Import data (CSV)** accepts
  a CSV export (columns `name`, `url`, `username`, `password`, `notes`).

The [user guide](USER_GUIDE.en.md) describes every screen in detail.

## 6. Where is my data?

Everything stays on your computer, in an encrypted SQLite database:

| System | Database | Logs |
| ------ | -------- | ---- |
| Windows | `%LOCALAPPDATA%\Heelonys\HeelonVault\data\heelonvault\data\` | `%LOCALAPPDATA%\Heelonys\HeelonVault\data\heelonvault\logs\` |
| macOS | `~/Library/Application Support/fr.Heelonys.HeelonVault/heelonvault/data/` | `~/Library/Application Support/fr.Heelonys.HeelonVault/heelonvault/logs/` |
| Linux | `~/.local/share/heelonvault/` | `~/.local/state/heelonvault/logs/` |

To back up your secrets, prefer the encrypted `.hvb` export in **Profile & Security** over a raw
copy of the database file.

## 7. Professional license (optional)

Without a license, HeelonVault runs as the **Community** edition, with no time limit and no cap
on the number of secrets. A **Professional** license also enables user administration, teams and
shared vaults, and signed audit reports: see
[HeelonVault Premium](https://doc.heelonvault.heelonys.fr/en/premium/).

To activate it, copy the `license.hvl` file you received to this location, then restart the
application:

| System | Location |
| ------ | -------- |
| Windows | `C:\ProgramData\HeelonVault\license.hvl` |
| macOS | `~/Library/Application Support/heelonvault/license.hvl` |
| Linux | `/etc/heelonvault/license.hvl` (administrator rights required) |

The "Free license" badge on the sign-in screen is replaced by the "Certified by Heelonys" seal
with your organization's name. An invalid or expired file is ignored: the application stays on the Community edition.

## 8. Uninstall

- **Windows**: Settings > Apps > HeelonVault > Uninstall.
- **macOS**: drag HeelonVault from Applications to the Trash.
- **Linux**: delete the AppImage file.

Uninstalling **keeps your data** (see section 6). Delete that folder by hand only if you want to
erase your secrets for good, after exporting them if needed.

## Going further

- [User guide](USER_GUIDE.en.md) — every screen and feature
- [Updating](UPDATE_GUIDE.en.md) — moving to a new version
- [Security](../SECURITY.md) — threat model, cryptography, vulnerability reporting
