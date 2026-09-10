PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS secret_items (
	id TEXT PRIMARY KEY NOT NULL,
	vault_id TEXT NOT NULL,
	secret_type TEXT NOT NULL,
	title TEXT,
	metadata_json TEXT,
	secret_blob BLOB,
	file_blob_ref BLOB,
	blob_storage TEXT NOT NULL,
	tags TEXT,
	expires_at TEXT,
	created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
	modified_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
	usage_count INTEGER NOT NULL DEFAULT 0,
	deleted_at TEXT,
	FOREIGN KEY (vault_id) REFERENCES vaults(id) ON DELETE CASCADE,
	CHECK (secret_type IN ('password', 'api_token', 'ssh_key', 'secure_document')),
	CHECK (blob_storage IN ('inline', 'file')),
	CHECK (
		(blob_storage = 'inline' AND secret_blob IS NOT NULL)
		OR (blob_storage = 'file' AND file_blob_ref IS NOT NULL)
	)
);

CREATE INDEX IF NOT EXISTS idx_secret_items_vault_id ON secret_items(vault_id);
CREATE INDEX IF NOT EXISTS idx_secret_items_vault_id_deleted_at ON secret_items(vault_id, deleted_at);
CREATE INDEX IF NOT EXISTS idx_secret_items_secret_type ON secret_items(secret_type);
CREATE INDEX IF NOT EXISTS idx_secret_items_expires_at ON secret_items(expires_at);
CREATE INDEX IF NOT EXISTS idx_secret_items_deleted_at ON secret_items(deleted_at);
