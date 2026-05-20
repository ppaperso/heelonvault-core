PRAGMA foreign_keys = ON;

-- Allow users to set a custom display order for their vaults.
-- Default 0 keeps existing alphabetical ordering as the natural tiebreaker.
ALTER TABLE vaults ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_vaults_owner_sort_order
ON vaults(owner_user_id, sort_order, name);
