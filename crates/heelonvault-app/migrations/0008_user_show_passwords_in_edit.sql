PRAGMA foreign_keys = ON;

ALTER TABLE users ADD COLUMN show_passwords_in_edit INTEGER NOT NULL DEFAULT 0;