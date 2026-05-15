-- Add preferred UI language for per-user i18n selection.
ALTER TABLE users ADD COLUMN preferred_language TEXT NOT NULL DEFAULT 'fr';
