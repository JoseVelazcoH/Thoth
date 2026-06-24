import logging
import os
import sqlite3
import time
from pathlib import Path

from tth.schema import SCHEMA_V1, SCHEMA_V2_FTS

logger = logging.getLogger(__name__)

BUSY_TIMEOUT_MS = 2000

DEFAULT_DB_PATH = Path("~/.local/share/thoth/history.db").expanduser()


def _resolve_db_path() -> Path:
    override = os.environ.get("THOTH_DB")
    if override:
        return Path(override)
    xdg = os.environ.get("XDG_DATA_HOME")
    if xdg:
        return Path(xdg) / "thoth" / "history.db"
    return Path("~/.local/share/thoth/history.db").expanduser()


MIGRATIONS: list[tuple[int, str]] = [
    (1, SCHEMA_V1),
    (2, SCHEMA_V2_FTS),
]


def connect(db_path: str = ":memory:") -> sqlite3.Connection:
    conn = sqlite3.connect(db_path, check_same_thread=False)
    conn.row_factory = sqlite3.Row
    return conn


def get_connection(db_path: Path | None = None) -> sqlite3.Connection:
    db_path = Path(db_path) if db_path is not None else _resolve_db_path()
    db_path.parent.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(str(db_path), check_same_thread=False)
    conn.row_factory = sqlite3.Row
    conn.execute("PRAGMA journal_mode=WAL")
    conn.execute(f"PRAGMA busy_timeout={BUSY_TIMEOUT_MS}")
    conn.execute("PRAGMA synchronous=NORMAL")
    apply_migrations(conn)
    return conn


def current_version(conn: sqlite3.Connection) -> int:
    try:
        row = conn.execute(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version"
        ).fetchone()
        return row[0] if row else 0
    except sqlite3.OperationalError:
        return 0


def fts5_available(conn: sqlite3.Connection) -> bool:
    try:
        conn.execute("CREATE VIRTUAL TABLE temp.__fts_probe USING fts5(x)")
        conn.execute("DROP TABLE IF EXISTS temp.__fts_probe")
        return True
    except sqlite3.OperationalError:
        return False


def _split_sql(sql: str) -> list[str]:
    statements: list[str] = []
    buf = ""
    for line in sql.splitlines(keepends=True):
        buf += line
        if sqlite3.complete_statement(buf):
            stmt = buf.strip()
            if stmt:
                statements.append(stmt)
            buf = ""

    remainder = buf.strip()
    if remainder:
        statements.append(remainder)

    return statements


def apply_migrations(conn: sqlite3.Connection) -> None:
    ver = current_version(conn)

    for version, sql in MIGRATIONS:
        if version <= ver:
            continue

        if version == 2 and not fts5_available(conn):
            logger.warning("FTS5 not available - skipping FTS migration v%d", version)
            continue

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
