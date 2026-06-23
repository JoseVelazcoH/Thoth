"""Work-session grouping: get or create a session UUID."""

import sqlite3
import uuid

from tth.constants import SESSION_GAP_MINUTES

_GAP_SECONDS = SESSION_GAP_MINUTES * 60


def get_or_create(project: str, timestamp: int, conn: sqlite3.Connection) -> str:
    """Return existing session_id or create a new one.

    Rules (evaluated inside BEGIN IMMEDIATE):
    - New session if: no prior session, gap > 30 min, or project differs.
    - Reuse session if: gap <= 30 min AND project matches.
    """
    try:
        conn.execute("BEGIN IMMEDIATE")
    except sqlite3.OperationalError:
        pass  # already in a transaction

    row = conn.execute(
        "SELECT session_id, project, ended_at FROM sessions ORDER BY ended_at DESC LIMIT 1"
    ).fetchone()

    create_new = (
        row is None
        or (timestamp - row["ended_at"]) > _GAP_SECONDS
        or row["project"] != project
    )

    if create_new:
        sid = str(uuid.uuid4())
        conn.execute(
            "INSERT INTO sessions(session_id, project, started_at, ended_at, command_count) "
            "VALUES(?, ?, ?, ?, 0)",
            (sid, project, timestamp, timestamp),
        )
    else:
        sid = row["session_id"]
        conn.execute(
            "UPDATE sessions SET ended_at=? WHERE session_id=?",
            (timestamp, sid),
        )

    conn.commit()
    return sid
