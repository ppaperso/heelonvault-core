-- Account key wrapped under a key derived from the recovery phrase, so a restored backup can be
-- reopened with the phrase alone. Layout: version(1) || salt(32) || nonce(12) || ciphertext.
ALTER TABLE users ADD COLUMN recovery_key_envelope BLOB DEFAULT NULL;
