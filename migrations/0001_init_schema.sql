PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS users (
	id TEXT PRIMARY KEY NOT NULL,
	username TEXT NOT NULL UNIQUE,
	role TEXT NOT NULL,
	password_envelope BLOB,
	totp_secret_envelope BLOB,
	created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
	last_login TEXT,
	CHECK (role IN ('user', 'admin'))
);

CREATE TABLE IF NOT EXISTS vaults (
	id TEXT PRIMARY KEY NOT NULL,
	owner_user_id TEXT NOT NULL,
	name TEXT NOT NULL,
	vault_key_envelope BLOB,
	created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
	FOREIGN KEY (owner_user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_users_username ON users(username);
CREATE INDEX IF NOT EXISTS idx_vaults_owner_user_id ON vaults(owner_user_id);
CREATE INDEX IF NOT EXISTS idx_vaults_owner_user_id_name ON vaults(owner_user_id, name);
