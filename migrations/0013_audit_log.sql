PRAGMA foreign_keys = ON;

-- Tamper-evident audit trail required for medical-lab compliance (RGPD, ISO 27001).
-- actor_user_id is nullable so user deletion does not erase history.
-- action uses dot-namespaced strings for easy filtering (e.g. "vault.key_rotated").
-- target_type / target_id identify the affected entity ("user", "vault", "team", …).
-- detail carries free-form JSON for extra context without breaking the schema.
CREATE TABLE IF NOT EXISTS audit_log (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    actor_user_id TEXT,
    action        TEXT NOT NULL,
    target_type   TEXT,
    target_id     TEXT,
    detail        TEXT,
    performed_at  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (actor_user_id) REFERENCES users(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_audit_log_actor      ON audit_log(actor_user_id);
CREATE INDEX IF NOT EXISTS idx_audit_log_performed  ON audit_log(performed_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_log_target     ON audit_log(target_type, target_id);
CREATE INDEX IF NOT EXISTS idx_audit_log_action     ON audit_log(action);
