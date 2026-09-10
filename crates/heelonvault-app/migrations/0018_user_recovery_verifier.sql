-- Add recovery_verifier column to users table.
-- Stores an Argon2id verifier over the user's recovery phrase so an export can
-- confirm the phrase re-typed by the user without ever deriving it from the vault.
-- Layout: version(1) || salt(32) || tag(32)

ALTER TABLE users ADD COLUMN recovery_verifier BLOB DEFAULT NULL;
