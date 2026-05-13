PRAGMA foreign_keys = ON;

-- Per-user encrypted copy of a vault key, used for multi-user vault sharing.
--
-- When an owner or admin shares a vault with a user, the vault key is
-- re-encrypted with the recipient's master key and stored here.
-- The owner's access is always through vaults.vault_key_envelope; this table
-- is only for *additional* recipients.
--
-- Key rotation: on member removal the caller should call
-- TeamService::rotate_vault_key() which replaces ALL rows for that vault
-- (including re-encrypting all secret items) so the removed member's copy
-- becomes useless even if they retained it in memory.
--
-- granted_via_team is informational (nullable) and helps identify which team
-- grant produced this share so it can be bulk-revoked when a team is deleted.
CREATE TABLE IF NOT EXISTS vault_key_shares (
    vault_id         TEXT NOT NULL,
    user_id          TEXT NOT NULL,
    key_envelope     BLOB NOT NULL,          -- vault key encrypted with user's master key
    granted_by       TEXT,                   -- actor who created the share
    granted_via_team TEXT,                   -- team_id if created via a team grant
    granted_at       TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (vault_id, user_id),
    FOREIGN KEY (vault_id)         REFERENCES vaults(id) ON DELETE CASCADE,
    FOREIGN KEY (user_id)          REFERENCES users(id)  ON DELETE CASCADE,
    FOREIGN KEY (granted_via_team) REFERENCES teams(id)  ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_vault_key_shares_user  ON vault_key_shares(user_id);
CREATE INDEX IF NOT EXISTS idx_vault_key_shares_vault ON vault_key_shares(vault_id);
CREATE INDEX IF NOT EXISTS idx_vault_key_shares_team  ON vault_key_shares(granted_via_team);
