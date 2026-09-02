-- Add recovery_phrase_envelope column to users table
-- This stores the user's recovery key (24-word BIP39 mnemonic) encrypted with their master key
-- The recovery key is generated once during bootstrap and reused for all exports

ALTER TABLE users ADD COLUMN recovery_phrase_envelope BLOB DEFAULT NULL;
