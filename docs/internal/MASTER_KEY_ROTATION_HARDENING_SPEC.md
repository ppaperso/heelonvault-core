# Master Key Rotation Hardening Spec

Date: 2026-05-19
Scope: user password rotation safety and zero data-loss guarantees.

## 0) Implementation Status (2026-05-19)

- Phase A implemented: hardened models and UserService API in place.
- Phase B implemented: secured backup ticket export/restore flow in place.
- Phase C implemented: orchestration with pre-check, atomic DB mutation, and post-validation in place.
- Runtime validation completed: master key change executed successfully in the application.
- Automated validation completed: broad test run (`cargo test`) green.

## 1) Objective

Implement a robust master key rotation workflow that guarantees:

- backup before mutation,
- deterministic rewrap of all reachable vault key envelopes,
- post-rotation validation of vault and secret access,
- automatic rollback and restore on any integrity failure.

## 2) Clarification on Secrets vs Vaults

Secrets are encrypted with the vault key, not with the user master key.
Therefore, during master key rotation:

- mandatory re-encryption target: vault key envelopes (owner envelope and per-user share envelopes),
- no bulk re-encryption of secret blobs required,
- mandatory validation target: opening vaults and decrypting at least one secret per non-empty vault.

## 3) Current Risk to Eliminate

Current flow can enter a partially migrated state:

- password update and auth envelope update occur before complete vault envelope migration,
- if one envelope rewrap fails, user may lose access to a subset of vaults,
- no automatic backup restore path exists.

## 4) Service API Additions

### 4.1 UserService API

Add a hardened endpoint in src/services/user_service.rs:

- New input model: MasterKeyRotationRequest
  - user_id: Uuid
  - current_password: `SecretBox<Vec<u8>>`
  - new_password: `SecretBox<Vec<u8>>`
  - actor_id: Uuid (for backup authorization and audit trace)
  - policy: MasterKeyRotationPolicy

- New policy model: MasterKeyRotationPolicy
  - require_backup: bool (default true)
  - keep_backup_on_success: bool (default false)
  - validation_mode: RotationValidationMode
  - max_secrets_validate_per_vault: usize (default 1)

- New validation enum: RotationValidationMode
  - VaultOpenOnly
  - VaultAndSampleSecret

- New output model: MasterKeyRotationReport
  - rotation_id: Uuid
  - backup_path: `Option<String>`
  - scanned_vaults: usize
  - owner_vaults_rewrapped: usize
  - shared_vaults_rewrapped: usize
  - sample_secrets_validated: usize
  - elapsed_ms: u128

- New method:
  - rotate_master_key_hardened(request: MasterKeyRotationRequest) -> Result<MasterKeyRotationReport, AppError>

Keep existing change_master_password method for compatibility, but migrate UI callsites to the hardened method.

### 4.2 AuthService API

Add deterministic, non-mutating envelope generation to avoid mutating in-memory auth state too early.

In src/services/auth_service.rs add:

- build_password_envelope_for_rotation(
  username: &str,
  current_password: `SecretBox<Vec<u8>>`,
  new_password: `SecretBox<Vec<u8>>`,
) -> `Result<SecretBox<Vec<u8>>, AppError>`

Behavior:

- verify current password,
- enforce password policy,
- derive new hash/salt envelope,
- return envelope only,
- do not mutate in-memory credential store.

Also add:

- reload_user_envelope(username: &str, envelope: `SecretBox<Vec<u8>>`) -> Result<(), AppError>

This can delegate to upsert_password_envelope.

### 4.3 BackupApplicationService API

In src/services/backup_application_service.rs add:

- export_rotation_backup_secured(
  actor_id: Uuid,
  sqlite_db_path: &Path,
  backup_file_path: &Path,
) -> Result<RotationBackupTicket, AppError>

- restore_rotation_backup_secured(
  actor_id: Uuid,
  ticket: &RotationBackupTicket,
  sqlite_db_path: &Path,
) -> Result<(), AppError>

New ticket model:

- backup_file_path: String
- recovery_phrase: SecretString
- metadata_sha256_hex: String
- created_at: String

Rationale:

- rotation flow needs backup and possible restore without user interaction.

### 4.4 VaultService Optional Validation API

In src/services/vault_service.rs add optional helper:

- open_all_accessible_vaults_for_user(
  user_id: Uuid,
  master_key: `SecretBox<Vec<u8>>`,
) -> `Result<Vec<(Uuid, SecretBox<Vec<u8>>)>, AppError>`

This is convenience only. Validation can also be implemented directly by list_user_vaults + open_vault_for_user.

## 5) Exact Operation Order

Execution order for rotate_master_key_hardened.

1. Generate rotation_id and start audit trace.
2. Load user profile and normalize username.
3. Preflight inventory:
   - list accessible vaults,
   - classify owner vs shared access,
   - fail fast if no accessible vault only if policy requires at least one (optional).
4. Preflight old-key validation:
   - derive old master key from current password,
   - open each accessible vault with old key,
   - if validation_mode is VaultAndSampleSecret: decrypt first secret in each non-empty vault.
   - on failure: stop before any write.
5. Build new password envelope without mutation via build_password_envelope_for_rotation.
6. Derive new master key using same credentials (non-mutating path).
7. Create automatic backup (if require_backup):
   - generate recovery phrase,
   - export encrypted backup,
   - keep backup ticket in memory until operation completes.
8. Start database transaction.
9. Persist user password envelope in DB inside transaction.
10. Rewrap all vault key envelopes in transaction:
    - owner vaults: read owner envelope, decrypt with old key, encrypt with new key, update owner envelope,
    - shared vault access rows: read user share envelope, decrypt with old key, encrypt with new key, update key share envelope.
11. Post-write validation before commit:
    - reopen every accessible vault using new key and requester context,
    - if VaultAndSampleSecret: decrypt sample secrets again.
12. Commit transaction.
13. Update in-memory auth store with new envelope via reload_user_envelope.
14. Final integrity check outside transaction (defensive repeat open on all vaults).
15. On success:
    - write audit success,
    - remove backup if keep_backup_on_success is false,
    - return MasterKeyRotationReport.

Failure paths:

- Failure before transaction commit: rollback transaction and return error. Backup retained.
- Failure after commit but before final success:
  - restore backup automatically using backup ticket,
  - reload auth envelope from restored DB,
  - emit critical audit event,
  - return explicit error requiring user re-login.

## 6) Rollback Strategy Details

### 6.1 Rollback Trigger Conditions

Trigger automatic restore when any of these happens after writes started:

- any envelope rewrap mismatch,
- any vault open failure with new master key,
- any sample secret decrypt failure,
- in-memory auth update failure after commit,
- unexpected internal error in validation phase.

### 6.2 Restore Sequence

1. Stop mutation flow and prevent concurrent rotation.
2. Restore DB from backup ticket.
3. Rebuild auth in-memory envelope by reading users.password_envelope.
4. Force current session logout to guarantee key/session coherence.
5. Show blocking error to user with rotation_id and safe retry instructions.

## 7) Concurrency and Idempotency

Add a single-process guard for rotation:

- only one rotation at a time,
- reject concurrent requests with explicit error code.

Idempotency:

- each rotation gets rotation_id,
- never reuse backup ticket across rotation_id,
- stale ticket restore must be rejected.

## 8) Observability and Audit

Add structured logs and audit events:

- rotation_started
- rotation_backup_created
- rotation_rewrap_progress (vault_id, owner_or_shared)
- rotation_validation_passed
- rotation_rollback_started
- rotation_rollback_completed
- rotation_completed

Never log secrets, master keys, plaintext secret values, or recovery phrase.

## 9) Test Matrix (Ready to Implement)

### 9.1 Owner Scenarios

1. Owner, single vault, no secrets:

- expect success,
- owner envelope changed,
- vault opens with new key,
- old password rejected.

1. Owner, single vault, with secrets:

- expect success,
- sample secret decrypt passes with new key,
- secret count unchanged,
- old password rejected.

1. Owner, two personal vaults:

- expect success across both vaults,
- both owner envelopes rewrapped.

### 9.2 Shared Scenarios

1. User has one owned vault plus one direct shared vault:

- expect success,
- owner envelope updated for owned vault,
- key_share envelope updated for shared vault,
- both vaults open with new key.

1. User has only shared vault access (no owned vault):

- expect success,
- only key_share envelope updates,
- shared vault remains readable.

1. Team share access path (if enabled in test fixtures):

- same assertions as direct share, with team access metadata.

### 9.3 Multi-vault / Multi-secret Scenarios

1. Five accessible vaults mixed owner/shared, each with secrets:

- expect success,
- all vaults open post-rotation,
- sample secret validation count equals non-empty vault count.

1. Vault with many secrets (performance smoke):

- validation mode VaultAndSampleSecret,
- verify no timeout/deadlock,
- no data drift.

### 9.4 Forced Rollback Scenarios

1. Inject failure during owner envelope update at Nth vault:

- transaction rolled back,
- old password still valid,
- no envelope changed.

1. Inject failure during shared envelope update:

- transaction rolled back,
- old access remains valid.

1. Inject failure after commit during final validation:

- automatic backup restore executed,
- restored DB matches pre-rotation digest,
- user forced logout,
- old password valid after restart.

1. Inject failure during in-memory auth reload:

- automatic backup restore executed,
- state consistency restored.

### 9.5 Security and Regression

1. Wrong current password:

- no backup creation,
- no writes,
- explicit invalid credentials.

1. New password policy failure:

- no backup creation,
- no writes,
- explicit validation error.

1. Concurrency test with two simultaneous rotations:

- one succeeds or proceeds,
- second returns rotation already in progress,
- no corruption.

## 10) Implementation Phases

Phase A: API and models

- Add request/policy/report models and new service methods.

Phase B: backup ticket and restore plumbing

- Add non-interactive backup export and restore secured API.

Phase C: hardened rotation orchestration

- Implement ordered flow with transaction + validation + rollback.

Phase D: test hooks and matrix

- Add deterministic failpoints in non-production builds,
- implement all matrix tests.

Phase E: UI migration

- update profile password change action to call hardened method,
- display rotation_id and rollback outcome messaging.

## 11) Acceptance Criteria

All criteria must pass before release:

- no partial migration state reproducible under forced failures,
- backup restore automatically recovers from post-commit validation failure,
- owner and shared access both remain operational,
- import into selected vault works after rotation,
- full integration suite plus new rotation matrix green.
