pub const SCHEMA_V1: &str = "
CREATE TABLE IF NOT EXISTS schema_version (
    version    INTEGER PRIMARY KEY,
    applied_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS commands (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    command     TEXT    NOT NULL,
    directory   TEXT    NOT NULL,
    project     TEXT    NOT NULL,
    session_id  TEXT    NOT NULL,
    timestamp   INTEGER NOT NULL,
    exit_code   INTEGER NOT NULL DEFAULT 0,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    tags        TEXT    NOT NULL DEFAULT '[]'
);

CREATE INDEX IF NOT EXISTS idx_commands_session   ON commands(session_id);
CREATE INDEX IF NOT EXISTS idx_commands_timestamp ON commands(timestamp);

CREATE TABLE IF NOT EXISTS sessions (
    session_id    TEXT    PRIMARY KEY,
    project       TEXT    NOT NULL,
    started_at    INTEGER NOT NULL,
    ended_at      INTEGER NOT NULL,
    command_count INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS projects (
    path          TEXT    PRIMARY KEY,
    name          TEXT    NOT NULL,
    last_seen     INTEGER NOT NULL,
    command_count INTEGER NOT NULL DEFAULT 0
);
";

pub const SCHEMA_V3_TERMINAL_ID: &str = "ALTER TABLE commands ADD COLUMN terminal_id TEXT;";

pub const SCHEMA_V2_FTS: &str = "
CREATE VIRTUAL TABLE IF NOT EXISTS commands_fts
    USING fts5(command, content='commands', content_rowid='id');

CREATE TRIGGER IF NOT EXISTS commands_ai AFTER INSERT ON commands BEGIN
    INSERT INTO commands_fts(rowid, command) VALUES (new.id, new.command);
END;

CREATE TRIGGER IF NOT EXISTS commands_ad AFTER DELETE ON commands BEGIN
    INSERT INTO commands_fts(commands_fts, rowid, command) VALUES ('delete', old.id, old.command);
END;
";
