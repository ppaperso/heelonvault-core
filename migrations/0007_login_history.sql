PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS login_history (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id     TEXT NOT NULL,
    login_at    TEXT NOT NULL,
    ip_address  TEXT,
    device_info TEXT,
    FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE INDEX IF NOT EXISTS idx_login_history_user_id_login_at
    ON login_history(user_id, login_at DESC);
