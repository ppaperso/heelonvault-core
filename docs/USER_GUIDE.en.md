# User Guide

Language: EN | [FR](USER_GUIDE.md)

Documented target version: `1.1.0`

## Purpose

This user manual describes HeelonVault from an end-user perspective. It is intended for people who need to access, secure, organize, and maintain secrets in the product without relying on internal technical documentation.

The document follows the main product screens and explains, for each one, its role, the available actions, and the associated best practices.

This user guide covers day-to-day HeelonVault usage on the desktop side:

- first launch;
- sign-in and session security;
- creating, editing, and searching secrets;
- import, export, and trash workflows;
- security best practices.

## Table of contents

1. User journey overview
2. Screen 1 - Bootstrap wizard
3. Screen 2 - Sign-in
4. Screen 3 - Main vault view
5. Screen 4 - Create a secret
6. Screen 5 - Edit, delete, and trash
7. Screen 6 - Search and organization
8. Screen 7 - Profile and security
9. Screen 8 - Import and export
10. Screen 9 - Dashboard and audit
11. Best practices
12. Quick troubleshooting
13. Useful references

## Screenshot placeholders

Screenshots can be added later at the dedicated locations already prepared in this document. The numbering below makes it easier to align future visuals with the relevant product screens.

Suggested naming convention:

- `docs/images/user-guide/login-en.png`
- `docs/images/user-guide/dashboard-en.png`
- `docs/images/user-guide/editor-en.png`

## 1. User journey overview

The standard HeelonVault user journey usually follows this sequence:

1. initialize or open a vault;
2. sign in;
3. browse or search a secret;
4. create, edit, share, or delete an item;
5. manage session security and advanced operations.

This structure is used throughout the manual because it reflects the actual product flow experienced by end users.

## 2. Screen 1 - Bootstrap wizard

On first start, HeelonVault opens a guided bootstrap flow to create the first administrator account.

Screen role:

- prepare the vault for first use;
- create the first account with administration privileges;
- record the recovery information required for future access.

General steps:

1. Choose the administrator login.
2. Define a strong master password.
3. Save the generated recovery key.
4. Complete initialization and open the vault.

Important notes:

- the recovery key should be stored in a secure location outside the workstation;
- the master password directly affects vault access security;
- the process should not be interrupted before the displayed recovery material is safely recorded.

Screenshot placeholder: Screen 1 - bootstrap wizard

![Screen 1a - Bootstrap step 1](../assets/images/user-guide/hv_first_init_1.png)

Capture 01a - Bootstrap wizard, step 1 (first administrator account creation).

![Screen 1b - Bootstrap step 2](../assets/images/user-guide/hv_first_init_2.png)

Capture 01b - Bootstrap wizard, step 2 (24-word recovery key).

## 3. Screen 2 - Sign-in

After initialization, the sign-in screen accepts account credentials and, when enabled, the one-time TOTP code.

Screen role:

- authenticate the user;
- protect access to the vault;
- enforce configured account security rules.

Best practices:

- use a unique, long password;
- store the recovery key outside the workstation;
- verify system time if TOTP codes are rejected.

Screenshot placeholder: Screen 2 - sign-in

![Screen 2 - Sign-in](../assets/images/user-guide/hv_login_screen_after_init.png)

Capture 02 - Sign-in screen with username, password, and database recovery entry point (.hvb).

## 4. Screen 3 - Main vault view

Once signed in, the user reaches the main vault view with:

- the secrets list;
- search and filtering functions;
- create, edit, delete, and share actions;
- profile and security access.

Screen role:

- act as the main entry point for daily work;
- centralize vault navigation;
- provide fast access to priority actions.

Screenshot placeholder: Screen 3 - main window

![Screen 3 - Main vault view](../assets/images/user-guide/hv_dashboard_empty.png)

Capture 03 - Main vault view with search, categories, security audit filters, and central workspace.

## 5. Screen 4 - Create a secret

To add a new secret:

1. Open the create action.
2. Select the appropriate type or category.
3. Fill in relevant fields: title, login, password, URL, notes, tags.
4. Review the strength indicator.
5. Save.

Recommendations:

- use explicit titles;
- add tags to improve searchability;
- avoid unnecessary sensitive notes.

From a product perspective, this is one of the key screens because it balances fast data entry with data quality and security requirements.

Screenshot placeholder: Screen 4 - secret editor

![Screen 4a - Secret type selection](../assets/images/user-guide/hv_add_menu.png)

Capture 04a - Secret type selection (password, api_token, ssh_key, secure_document).

![Screen 4b - Password secret form](../assets/images/user-guide/hv_add_password1.png)

Capture 04b - Password secret creation form (main fields).

![Screen 4c - Password secret advanced area](../assets/images/user-guide/hv_add_password2.png)

Capture 04c - Additional password-secret parameters (notes, validity, save actions).

![Screen 4d - API token form](../assets/images/user-guide/hv_add_apikey.png)

Capture 04d - API token secret form.

![Screen 4e - SSH key form](../assets/images/user-guide/hv_add_sshkey.png)

Capture 04e - SSH key secret form.

![Screen 4f - Secure document form](../assets/images/user-guide/hv_add_securedoc.png)

Capture 04f - Secure document secret form.

## 6. Screen 5 - Edit, delete, and trash

Each secret can be edited from the integrated editor. Deletion goes through the trash to reduce the risk of immediate data loss.

Screen role:

- maintain existing vault content;
- secure deletion through an intermediate recovery step;
- provide fast restoration when needed.

Recommended flow:

1. Edit the secret if needed.
2. Use soft-delete to move it to trash.
3. Restore it if deletion was accidental.
4. Permanently purge only after confirmation.

Screenshot placeholder: Screen 5 - trash

![Screen 5 - Trash and restore](../assets/images/user-guide/hv_trash.png)

Capture 05 - Trash view with restore and purge actions for deleted items.

## 7. Screen 6 - Search and organization

HeelonVault supports real-time multi-field search across all secret data.

### Search mode

- **Active vault (default)**: search is scoped to the vault selected in the left sidebar.
- **MultiVault**: activate the **MultiVault** toggle button to the left of the search bar to search across all accessible vaults simultaneously.

### Search without a prefix

A term typed on its own is matched against all fields: title, type, login, email, URL, notes, category, tags, and vault name. Fuzzy matching tolerates one typo.

### `field:value` syntax

To target a specific field, use `field:value` syntax (with or without a space after the colon):

| Accepted keys | Field searched |
| --- | --- |
| `title`, `name`, `titre`, `nom` | Title |
| `login`, `user`, `username`, `identifiant` | Login |
| `email`, `mail` | Email |
| `url`, `site`, `domaine`, `domain` | URL |
| `notes`, `note` | Notes |
| `category`, `categorie`, `cat` | Category |
| `tag`, `tags` | Tags |
| `type`, `kind` | Secret type |
| `vault`, `coffre`, `vault-name` | Vault name |

Examples: `login:alice` · `vault:perso` · `title:gmail` · `url:google`

The `?` button to the right of the search bar shows this reference directly in the app.

### Best practices

Improve search relevance with consistent organization:

- adopt a stable naming convention;
- use tags consistently;
- group secrets by type, use case, or team when relevant.

Screenshot placeholder: Screen 6 - search

![Screen 6 - Search and navigation](../assets/images/user-guide/hv_dashboard_empty.png)

Capture 06 - Search bar with MultiVault toggle and help button, left-side navigation.

## 8. Screen 7 - Profile and security

From the profile area, users can review security- and session-related settings, including:

- TOTP activation;
- auto-lock policy;
- master password change with vault key-envelope rotation;
- PIN Quick-Unlock activation;
- some display preferences depending on role and configuration.

### PIN Quick-Unlock

HeelonVault provides a PIN-based quick-unlock to avoid retyping the master password after every auto-lock event.

**Activation** (Profile → Session security section):

1. Click "Enable PIN".
2. Enter a 4-to-8 digit PIN code.
3. Confirm the PIN.
4. The PIN is active immediately for the current session.

**Usage during auto-lock**:

- When auto-lock triggers (after the configured inactivity timeout), the PIN entry dialog appears.
- Enter the PIN and press "Unlock".
- Up to 3 attempts are allowed before the cache is wiped.
- After 3 failed attempts or 12 hours of inactivity, the system automatically falls back to the full master-password login.
- The "Use master password" button lets the user switch to the full login flow at any time.

**Deactivation**:

- From the profile, click "Disable PIN" to wipe the cache immediately.

**Session-time indicator in the title bar**:

When the PIN is active, a "PIN active" badge appears in the application title bar. The badge changes appearance based on the time remaining before the cache expires:

- **Nominal** (more than 2 hours remaining): standard semi-transparent white text, normal behaviour.
- **Warning** (between 15 min and 2 h): amber border and text — consider logging out and back in to renew the session if needed.
- **Critical** (less than 15 min): amber-filled badge with a pulsing animation, text shows "PIN · Xm" (X = minutes remaining). The cache will expire within the displayed minute count.

Hovering over the badge shows a tooltip with the exact time remaining (e.g. "Expires in 1h 23m"). Clicking the badge opens the PIN management panel in the profile view.

The badge and its countdown timer are automatically removed when the cache expires, when the session is unlocked with the master password, or when the application is closed.

**Security boundaries to keep in mind**:

- The PIN does not replace the master password; it only speeds up session unlock.
- The PIN cache exists only in RAM and disappears when the application closes.
- Do not reuse a PIN you already use elsewhere (phone lock screen, bank card, etc.).

General recommendations:

- enable TOTP as early as possible;
- use a short auto-lock delay on shared workstations;
- after changing the master password, quickly verify access to main vaults;
- never leave an open session unattended.

This screen is the core user trust area of the product. It contains the settings that most directly affect protection of the vault and session behavior.

Screenshot placeholder: Screen 7 - profile and security

![Screen 7 - Profile and security](../assets/images/user-guide/hv_userprofil.png)

Capture 07 - Profile settings, session security, TOTP controls, import/export, and preferences.

## 9. Screen 8 - Import and export

Depending on granted permissions, HeelonVault supports:

- CSV import;
- `.hvb` export;
- operations constrained by RBAC rules.

The CSV import flow is now explicit and guided:

- **Step 1 - Preview**: after selecting a file, the app shows detected secrets, importable rows, and rows that require manual review.
- **Step 2 - Progress**: during import, a dedicated window displays live progress (processed/imported/failed).
- **Step 3 - Final summary**: the app shows a detailed report (total/imported/failed) and lists the first non-imported rows with reasons for manual correction.

Before importing:

- verify file format and encoding;
- clean unnecessary columns;
- confirm the correct target vault.
- review the final summary and fix flagged rows before running a focused re-import.
- check the reject-report path when shown (`logs/csv_import_rejects_*.txt`).

Before exporting:

- limit the operation to the actual need;
- protect the exported file;
- delete the artifact after use when possible.

Screenshot placeholder: Screen 8 - import / export

![Screen 8 - Import/Export from profile](../assets/images/user-guide/hv_userprofil.png)

Capture 08 - Data management area (.hvb export and CSV import) from Profile & Security.

## 10. Screen 9 - Dashboard and audit

The security dashboard provides a summary of vault health. Audit logs trace sensitive actions.

Screen role:

- quickly expose attention points;
- review recent activity;
- support security and compliance reviews.

Common use cases:

- identify weak secrets;
- review recent events;
- track deletions, edits, and sharing operations.

Screenshot placeholder: Screen 9 - security dashboard

![Screen 9a - Security dashboard](../assets/images/user-guide/hv_dashboard_empty.png)

Capture 09a - Main dashboard and security audit indicators.

![Screen 9b - Team management](../assets/images/user-guide/hv_team.png)

Capture 09b - Team administration view (vault sharing and member management).

![Screen 9c - User management](../assets/images/user-guide/hv_users.png)

Capture 09c - User administration view (create users, roles, reset, delete).

## 11. Best practices

- Use a unique and strong master password.
- Enable TOTP as soon as the account is activated.
- Store the recovery key away from the machine.
- Lock or close the session when leaving the workstation.
- Review obsolete secrets regularly.
- Keep exports to a minimum.

## 12. Quick troubleshooting

### Unable to sign in

- verify the account name;
- verify the password;
- verify system time if TOTP fails.

### The application locks too quickly

- review the auto-lock delay in session-related settings.

### A secret seems to be missing

- check the trash first;
- review the audit log if available.

### CSV import fails with a decryption error

- verify the target vault is accessible in the current session;
- sign out and sign in again if a master password change was just performed;
- retry the import and review the rejected-row summary.

## Useful references

- [QUICKSTART.md](QUICKSTART.md)
- [ARCHITECTURE.en.md](ARCHITECTURE.en.md)
- [UPDATE_GUIDE.en.md](UPDATE_GUIDE.en.md)
