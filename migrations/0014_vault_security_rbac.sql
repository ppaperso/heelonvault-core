PRAGMA foreign_keys = ON;

-- Security hardening migration for vault/secret lifecycle.
-- 1) RBAC roles on vault shares
-- 2) Key/versioning metadata for cryptographic migrations
-- 3) Soft-delete for vaults
-- 4) Immutable audit log table alias for security operations
-- 5) Access view to avoid repetitive UNION logic in application code

-- Vault key versioning + soft delete marker.
ALTER TABLE vaults ADD COLUMN vault_key_version INTEGER NOT NULL DEFAULT 1;
ALTER TABLE vaults ADD COLUMN deleted_at TEXT NULL;

-- Secret encryption versioning for forward crypto migrations.
ALTER TABLE secret_items ADD COLUMN encryption_version INTEGER NOT NULL DEFAULT 1;

-- Shared vault role.
ALTER TABLE vault_key_shares ADD COLUMN role TEXT NOT NULL DEFAULT 'read' CHECK(role IN ('read','write','admin'));

-- Defensive triggers: a secret must always be attached to exactly one vault.
-- (The NOT NULL + FK already exist in schema; this keeps behavior explicit.)
CREATE TRIGGER IF NOT EXISTS trg_secret_items_vault_id_not_null_insert
BEFORE INSERT ON secret_items
FOR EACH ROW
WHEN NEW.vault_id IS NULL
BEGIN
    SELECT RAISE(ABORT, 'secret_items.vault_id must not be NULL');
END;

CREATE TRIGGER IF NOT EXISTS trg_secret_items_vault_id_not_null_update
BEFORE UPDATE OF vault_id ON secret_items
FOR EACH ROW
WHEN NEW.vault_id IS NULL
BEGIN
    SELECT RAISE(ABORT, 'secret_items.vault_id must not be NULL');
END;

-- New immutable audit table dedicated to vault security flows.
CREATE TABLE IF NOT EXISTS audit_logs (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id    TEXT,
    action     TEXT NOT NULL,
    vault_id   TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (vault_id) REFERENCES vaults(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_audit_logs_user_created ON audit_logs(user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_logs_vault_created ON audit_logs(vault_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_logs_action_created ON audit_logs(action, created_at DESC);

-- Enforce append-only semantics.
CREATE TRIGGER IF NOT EXISTS trg_audit_logs_no_update
BEFORE UPDATE ON audit_logs
BEGIN
    SELECT RAISE(ABORT, 'audit_logs is immutable');
END;

CREATE TRIGGER IF NOT EXISTS trg_audit_logs_no_delete
BEFORE DELETE ON audit_logs
BEGIN
    SELECT RAISE(ABORT, 'audit_logs is immutable');
END;

-- Access projection: one row per (user_id, vault_id) with effective role.
-- owner role is always admin.
DROP VIEW IF EXISTS accessible_vaults;
CREATE VIEW accessible_vaults AS
SELECT
    v.owner_user_id AS user_id,
    v.id AS vault_id,
    v.owner_user_id,
    v.name,
    v.vault_key_version,
    'admin' AS role,
    'owner' AS access_kind
FROM vaults v
WHERE v.deleted_at IS NULL
UNION ALL
SELECT
    s.user_id AS user_id,
    v.id AS vault_id,
    v.owner_user_id,
    v.name,
    v.vault_key_version,
    s.role AS role,
    CASE
        WHEN s.granted_via_team IS NULL THEN 'direct_share'
        ELSE 'team_share'
    END AS access_kind
FROM vaults v
INNER JOIN vault_key_shares s ON s.vault_id = v.id
WHERE v.deleted_at IS NULL
  AND v.owner_user_id <> s.user_id;

CREATE INDEX IF NOT EXISTS idx_vault_key_shares_vault_user_role
ON vault_key_shares(vault_id, user_id, role);
