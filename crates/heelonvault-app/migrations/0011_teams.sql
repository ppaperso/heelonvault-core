PRAGMA foreign_keys = ON;

-- Teams regroup users sharing a common set of vault accesses.
-- created_by is nullable so a user deletion doesn't block the cascade.
CREATE TABLE IF NOT EXISTS teams (
    id         TEXT PRIMARY KEY NOT NULL,
    name       TEXT NOT NULL UNIQUE,
    created_by TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (created_by) REFERENCES users(id) ON DELETE SET NULL
);

-- Members of a team.  role: 'member' | 'leader'
-- Leaders can manage membership; admins can always manage.
CREATE TABLE IF NOT EXISTS team_members (
    team_id   TEXT NOT NULL,
    user_id   TEXT NOT NULL,
    role      TEXT NOT NULL DEFAULT 'member',
    joined_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (team_id, user_id),
    FOREIGN KEY (team_id) REFERENCES teams(id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    CHECK (role IN ('member', 'leader'))
);

CREATE INDEX IF NOT EXISTS idx_teams_name          ON teams(name);
CREATE INDEX IF NOT EXISTS idx_team_members_user   ON team_members(user_id);
CREATE INDEX IF NOT EXISTS idx_team_members_team   ON team_members(team_id);
