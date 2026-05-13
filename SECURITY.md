# Security Guide (Rust Runtime)

Language: EN | [FR](SECURITY.fr.md)

Last update: 27 March 2026
Scope: active runtime in src/

This document replaces legacy Python-era notes and reflects the current Rust codebase.

Language note / Note de langue:

- primary wording is English for technical consistency;
- key disclosure and compliance terms are mirrored in French when useful.

## 1. Security Scope and Threat Model

HeelonVault is a local-first desktop password manager.

Security goals:

- protect vault secrets at rest;
- protect authentication material (master password derivatives, not plaintext);
- limit brute-force attempts on login;
- reduce accidental leaks in UI and logs;
- preserve session safety (auto-lock and explicit logout).

Main assumptions:

- if the OS account is fully compromised while the app is unlocked, attacker impact remains high;
- the project focuses on local storage security, not cloud account security.

## 2. Cryptography in Rust

Current primitives used in src/services/crypto_service.rs:

- KDF: Argon2id (v=19)
- default KDF params: memory 64 MiB, time cost 3, parallelism 1
- derived key size: 32 bytes
- random salt size: 32 bytes
- encryption: AES-256-GCM
- nonce size: 12 bytes (fresh random nonce per encryption)
- RNG: getrandom (OS CSPRNG)
- sensitive buffers: secrecy + zeroize/Zeroizing patterns

Implementation notes:

- decryption/authentication failures return generic crypto errors;
- salts and keys are never stored as plaintext passwords;
- key derivation and encryption are isolated in dedicated services.

## 3. Authentication and Password Material

Current auth model in src/services/auth_service.rs:

- plaintext passwords are converted to secret strings in memory only;
- password verification uses constant-time byte comparison;
- credentials are stored as a versioned envelope:
  - envelope version byte
  - salt length + hash length
  - Argon2id salt and derived hash bytes
- persisted value in database: users.password_envelope (binary)

Important:

- no plaintext password is persisted;
- changing password rotates salt and hash;
- auth service supports shutdown signaling to block operations during controlled shutdown.

## 4. Login Identifier UX and Security

Login now accepts a single identifier field with resolution order:

1. username (exact logical match, case-insensitive after trim)
2. email (if present)
3. display_name (if present)

Security behavior:

- lock policy is applied on the resolved canonical username;
- failed attempts increment the policy counter for that account;
- unknown identifier is treated as invalid credentials.

## 5. Brute-force and Session Controls

Current lock controls in src/services/auth_policy_service.rs:

- threshold: 5 failed attempts
- lock window: 5 minutes
- counters are persisted in auth_policy table
- successful login resets failed_attempts and last_attempt_at

Current session controls:

- auto-lock delay per user: allowed values 0, 1, 5, 10, 15, 30 minutes
- default auto-lock delay: 5 minutes
- app returns to login screen on logout/auto-lock
- close-window path triggers secure logout behavior
- login flow enforces TOTP verification when 2FA is enabled for the resolved account

## 6. Password Policy and ANSSI Positioning

This project follows an internal baseline inspired by ANSSI guidance (long unique passphrases, no reuse, context-based hardening).

Current technical rules in Rust:

- password service policy (generator/validator):
  - min length 16, max 128
  - at least one lowercase, uppercase, digit, symbol
  - no whitespace
- generated passwords default to length 24

Current master password change rule in user flow:

- minimum length check currently set to 10 before update.

Security recommendation for operations:

- for admin and sensitive environments, use passphrases >= 16 chars;
- target roadmap is to align all entry points to a unified >= 16 policy.

## 7. Data Protection Boundaries

Data encrypted with AES-256-GCM:

- secret payloads in vault items (passwords and sensitive blobs)
- key envelopes used by vault service
- persisted password envelopes for authentication material

Data that may remain in clear text for UX/indexing reasons:

- some metadata fields such as labels/titles/tags/URLs.

Operational recommendation:

- do not place highly sensitive values in metadata fields intended for search/display.

## 8. 2FA Status

2FA/TOTP is active end-to-end in the current runtime flow.

Current behavior:

- users can activate/deactivate TOTP in profile/security UI;
- login requires password verification first, then TOTP code when enabled;
- TOTP invalid/missing code returns explicit user feedback and blocks login;
- TOTP secret is stored encrypted in database;
- migration handling keeps backward compatibility for legacy encrypted payloads.

Audit position:

- MFA can now be claimed as enforced for accounts with TOTP enabled.

## 8.1 First-Admin Bootstrap and Recovery Key

When the application starts with no admin account, a 3-step bootstrap wizard is presented:

1. **Identity step** — the user provides a username and password (minimum strength enforced, confirmation required);
2. **Oath step** — a 24-word BIP39-style mnemonic recovery phrase is generated via `BackupService::generate_recovery_key()`. The user must verify two randomly drawn words before confirming, proving they have recorded the phrase;
3. **Pending step** — `AdminService::bootstrap_first_admin()` is called in a background thread; on success the session is opened automatically.

Recovery key security properties:

- generated from a cryptographically secure RNG (`getrandom`);
- the phrase is never persisted in the database; it is the user's sole responsibility to store it safely;
- clipboard copy sets a 60-second auto-clear timer; the clipboard is also wiped when the dialog closes;
- after bootstrap, the recovery key can be re-exported at any time from `Profile & Security` (admin only), generating a new phrase wrapped in the same secure export dialog;
- backup export/import (`.hvb` files) is gated behind `BackupApplicationService` which enforces RBAC: only admin-role users may perform these operations.

## 9. Logging and Security Events

Current event coverage includes:

- successful login history (table login_history);
- auth failure threshold events (critical counters);
- policy reset/update traces.

Logging rules:

- never log plaintext secrets;
- keep technical details sufficient for incident triage without sensitive payload leakage.

## 10. Security Testing

Minimum test routine before release:

1. cargo check
2. cargo test
3. targeted security suites:

    tests/security_auth.rs
    tests/security_crypto.rs
    tests/totp_activation_integration.rs
    tests/twofa_messages_integration.rs
    tests/backup_security_integration.rs

Recommended manual checks:

1. verify login lock behavior after repeated failures
2. verify auto-lock behavior and forced return to login
3. verify password change rotates auth envelope and old password no longer works
4. verify login via username, display name, and email

## 11. Vulnerability Disclosure

Do not open public issues for security vulnerabilities.

Primary contact channel / Canal de contact principal:

  <security@heelonys.fr>

Email subject format / Format d'objet:

  SECURITY-HeelonVault : short title

Confidentiality recommendation / Recommandation de confidentialite:

- avoid sending plaintext secrets, full database files, or master passwords;
- share minimal proof-of-concept and redacted logs;
- if you need encrypted exchange, request secure keying instructions in your first message.

Please include / Merci d'inclure:

- impacted version
- environment
- reproduction steps
- expected vs actual behavior
- impact assessment
- proof of concept if available

Target response process (SLA) / Delais cibles de reponse:

1. acknowledgment within 24h
2. initial triage and severity classification within 3 business days
3. status update cadence at least every 7 days until closure

### 11.1 Mini CVSS Prioritization Matrix

Use this matrix for fast intake prioritization before full CVSS scoring.

| Priority | Exploitability | Impact on CIA | Operational target |
| -------- | -------------- | ------------- | ------------------ |
| P1 (Critical) | trivial or low-complexity remote exploit, no strong preconditions | high impact on confidentiality **and/or** integrity, or major availability loss | immediate handling, mitigation/fix target <= 7 days |
| P2 (High) | realistic exploit with limited prerequisites | moderate/high impact on one or more of confidentiality, integrity, availability | accelerated handling, mitigation/fix target <= 14 days |
| P3 (Medium) | exploit requires specific conditions, chaining, or user interaction | limited-to-moderate impact on confidentiality, integrity, availability | planned handling, mitigation/fix target <= 30 days |
| P4 (Low) | difficult exploit path or theoretical only | low impact on confidentiality, integrity, availability | best effort in regular release cycle |

CIA interpretation:

- confidentiality: unauthorized disclosure of secrets, metadata, keys, or sensitive logs;
- integrity: unauthorized modification of credentials, vault entries, or policy controls;
- availability: lockout, crash loop, data corruption, or sustained denial of service.

Remediation targets by severity:

- Critical: mitigation or fix target within 7 days
- High: mitigation or fix target within 14 days
- Medium: mitigation or fix target within 30 days
- Low: best-effort in regular release cycle

Coordinated disclosure policy:

- public disclosure only after a fix or mitigation is available, unless required by law;
- reporters are credited (optional) after coordinated disclosure.

## 12. Compliance and Hardening Roadmap

Completed:

- [x] MFA lifecycle hardened: TOTP activation/deactivation fully implemented
- [x] Backup/recovery workflow secured: recovery key generation with clipboard auto-wipe, mandatory verification, RBAC-gated backup operations
- [x] Audit trail for admin-sensitive operations (audit_log table, migration 0013)

Near-term priorities:

- unify master password policy to >= 16 across all flows
- document hardening profiles (standard, admin, high assurance)

Reference standards:

- ANSSI password and authentication recommendations
- OWASP Password Storage Cheat Sheet
- NIST SP 800-63B
