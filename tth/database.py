"""Database layer: schema migrations, connection factory, FTS5 helpers."""

import logging
import sqlite3
import time
from pathlib import Path

logger = logging.getLogger(__name__)

DEFAULT_DB_PATH = Path("~/.local/share/thoth/history.db").expanduser()
BUSY_TIMEOUT_MS = 2000

_SCHEMA_V1 = """
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
    session_id   TEXT    PRIMARY KEY,
    project      TEXT    NOT NULL,
    started_at   INTEGER NOT NULL,
    ended_at     INTEGER NOT NULL,
    command_count INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS projects (
    path          TEXT    PRIMARY KEY,
    name          TEXT    NOT NULL,
    last_seen     INTEGER NOT NULL,
    command_count INTEGER NOT NULL DEFAULT 0
);
"""

_SCHEMA_V2_FTS = """
CREATE VIRTUAL TABLE IF NOT EXISTS commands_fts
    USING fts5(command, content='commands', content_rowid='id');

CREATE TRIGGER IF NOT EXISTS commands_ai AFTER INSERT ON commands BEGIN
    INSERT INTO commands_fts(rowid, command) VALUES (new.id, new.command);
END;

CREATE TRIGGER IF NOT EXISTS commands_ad AFTER DELETE ON commands BEGIN
    INSERT INTO commands_fts(commands_fts, rowid, command) VALUES ('delete', old.id, old.command);
END;

-- Resync after bulk changes:
-- INSERT INTO commands_fts(commands_fts) VALUES('rebuild');
"""

MIGRATIONS: list[tuple[int, str]] = [
    (1, _SCHEMA_V1),
    (2, _SCHEMA_V2_FTS),
]



def connect(db_path: str = ":memory:") -> sqlite3.Connection:
    """Bare connection for tests. Caller must call apply_migrations."""
    conn = sqlite3.connect(db_path, check_same_thread=False)
    conn.row_factory = sqlite3.Row
    return conn


def get_connection(db_path: Path = DEFAULT_DB_PATH) -> sqlite3.Connection:
    """Production connection: ensure directory, apply PRAGMAs, run migrations."""
    db_path = Path(db_path)
    db_path.parent.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(str(db_path), check_same_thread=False)
    conn.row_factory = sqlite3.Row
    conn.execute("PRAGMA journal_mode=WAL")
    conn.execute(f"PRAGMA busy_timeout={BUSY_TIMEOUT_MS}")
    conn.execute("PRAGMA synchronous=NORMAL")
    apply_migrations(conn)
    return conn



def current_version(conn: sqlite3.Connection) -> int:
    """Return max applied migration version, or 0 if schema_version doesn't exist."""
    try:
        row = conn.execute(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version"
        ).fetchone()
        return row[0] if row else 0
    except sqlite3.OperationalError:
        return 0


def fts5_available(conn: sqlite3.Connection) -> bool:
    """Return True if the current SQLite build supports FTS5."""
    try:
        conn.execute("CREATE VIRTUAL TABLE temp.__fts_probe USING fts5(x)")
        conn.execute("DROP TABLE IF EXISTS temp.__fts_probe")
        return True
    except sqlite3.OperationalError:
        return False


def _split_sql(sql: str) -> list[str]:
    """Split a SQL script into individual top-level statements.

    Uses ``sqlite3.complete_statement()`` from the stdlib, which correctly
    tracks string-literal quote state, comment boundaries, and BEGIN…END
    trigger-body nesting.  This replaces the old hand-rolled depth-counter
    that had three latent bugs:

    1. Any ``END`` keyword (including ``CASE…END``) decremented the depth
       counter, which could prematurely close a trigger body.
    2. ``BEGIN``/``;``/``END`` inside string literals were treated as
       structural tokens, not literal text.
    3. ``line.split("--")[0]`` stripped content after ``--`` even when it
       appeared inside a string literal.

    Algorithm: accumulate input lines into a buffer one line at a time.
    Whenever ``sqlite3.complete_statement(buffer)`` returns ``True``, the
    buffer contains exactly one complete statement — strip it, keep it if
    non-empty (skip whitespace-only leftovers), and reset the buffer.
    After the loop, anything left in the buffer is a trailing statement
    without a final semicolon.
    """
    statements: list[str] = []
    buf = ""
    for line in sql.splitlines(keepends=True):
        buf += line
        if sqlite3.complete_statement(buf):
            stmt = buf.strip()
            if stmt:
                statements.append(stmt)
            buf = ""

    # Trailing content without a final semicolon (unlikely in well-formed SQL,
    # but handle it gracefully).
    remainder = buf.strip()
    if remainder:
        statements.append(remainder)

    return statements


def apply_migrations(conn: sqlite3.Connection) -> None:
    """Apply all pending migrations atomically.

    Each migration's DDL statements and its schema_version bump are executed
    inside a single explicit transaction so that either both commit or both
    roll back. We do NOT use executescript() because CPython's sqlite3 module
    issues an implicit COMMIT before running the script, which nullifies any
    preceding BEGIN and causes the migration body to run in autocommit mode.
    Instead, we split the SQL on semicolons and execute each statement
    individually within a manually managed transaction.
    """
    ver = current_version(conn)

    for version, sql in MIGRATIONS:
        if version <= ver:
            continue

        # Guard FTS5 migration
        if version == 2 and not fts5_available(conn):
            logger.warning("FTS5 not available — skipping FTS migration v%d", version)
            # Record the version so we do not retry on every startup.
            # This small INSERT is its own implicit transaction; that is
            # acceptable because skipping FTS is not a failure path.
            conn.execute(
                "INSERT OR IGNORE INTO schema_version(version, applied_at) VALUES(?, ?)",
                (version, int(time.time())),
            )
            conn.commit()
            continue

        # Split the SQL block via _split_sql (see its docstring for rationale).
        statements = _split_sql(sql)

        conn.execute("BEGIN IMMEDIATE")
        try:
            for stmt in statements:
                conn.execute(stmt)
            conn.execute(
                "INSERT OR IGNORE INTO schema_version(version, applied_at) VALUES(?, ?)",
                (version, int(time.time())),
            )
            conn.execute("COMMIT")
        except Exception:
            conn.execute("ROLLBACK")
            raise
