CREATE TABLE IF NOT EXISTS db_metadata (
	key TEXT PRIMARY KEY NOT NULL,
	value TEXT NOT NULL
);

INSERT INTO db_metadata (key, value)
VALUES ('schema_version', '1')
ON CONFLICT(key) DO NOTHING;
