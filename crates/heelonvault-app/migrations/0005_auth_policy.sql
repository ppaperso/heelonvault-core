PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS auth_policy (
    username TEXT PRIMARY KEY NOT NULL,
    failed_attempts INTEGER NOT NULL DEFAULT 0,
    last_attempt_at INTEGER
);

CREATE INDEX IF NOT EXISTS idx_auth_policy_failed_attempts
ON auth_policy(failed_attempts);
