-- Migration pour le rate limiting par IP
-- Issue: #41 - Ajouter rate limiting IP-based pour le login

CREATE TABLE IF NOT EXISTS login_attempts_ip (
    ip TEXT PRIMARY KEY NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0,
    first_attempt_at TEXT NOT NULL,
    locked_until TEXT
);

CREATE INDEX IF NOT EXISTS idx_login_attempts_ip_locked_until ON login_attempts_ip(locked_until);
