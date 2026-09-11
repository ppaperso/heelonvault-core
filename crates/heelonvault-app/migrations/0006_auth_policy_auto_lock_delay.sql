PRAGMA foreign_keys = ON;

ALTER TABLE auth_policy
ADD COLUMN auto_lock_delay_mins INTEGER NOT NULL DEFAULT 5;
