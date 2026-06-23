"""Recorder: core record() function and tth record CLI command."""

import json
import os
import sqlite3
import time
import traceback
import typer
from pathlib import Path

from tth.database import get_connection
from tth.project import infer_project
from tth.session import get_or_create

ERROR_LOG = Path("~/.local/share/thoth/error.log").expanduser()

app = typer.Typer()



def _normalize_tags(tags_json) -> str:
    """Return tags as a JSON array string. Normalize invalid/empty to '[]'."""
    if not tags_json:
        return "[]"
    try:
        parsed = json.loads(tags_json)
        if not isinstance(parsed, list):
            return "[]"
        return tags_json  # preserve the user-supplied JSON verbatim: do not re-serialize
    except (json.JSONDecodeError, TypeError, ValueError):
        return "[]"



def _record_inner(
    command: str,
    directory: str,
    exit_code: int,
    duration_ms: int,
    timestamp: int,
    tags_json: str,
    conn: sqlite3.Connection,
) -> None:
    """Insert a command row and update related session/project state."""
    tags = _normalize_tags(tags_json)
    project = infer_project(directory, conn)
    sid = get_or_create(project, timestamp, conn)

    conn.execute(
        "INSERT INTO commands(command, directory, project, session_id, timestamp, exit_code, duration_ms, tags) "
        "VALUES(?, ?, ?, ?, ?, ?, ?, ?)",
        (command, directory, project, sid, timestamp, exit_code, duration_ms, tags),
    )

    # Upsert project: refresh name/last_seen, increment command_count (never reset)
    conn.execute(
        "INSERT INTO projects(path, name, last_seen, command_count) VALUES(?, ?, ?, 1) "
        "ON CONFLICT(path) DO UPDATE SET "
        "name=excluded.name, last_seen=excluded.last_seen, command_count=command_count + 1",
        (directory, project, timestamp),
    )

    # Update session: bump ended_at and command_count
    conn.execute(
        "UPDATE sessions SET ended_at=?, command_count=command_count + 1 WHERE session_id=?",
        (timestamp, sid),
    )

    conn.commit()


def _log_error(exc: Exception) -> None:
    """Append exception info to ERROR_LOG, creating parent dirs as needed."""
    try:
        ERROR_LOG.parent.mkdir(parents=True, exist_ok=True)
        with ERROR_LOG.open("a", encoding="utf-8") as f:
            f.write(traceback.format_exc())
            f.write(f"\n{exc}\n")
    except Exception:
        pass  # truly silent — nothing we can do


def record(
    command: str,
    directory: str,
    exit_code: int,
    duration_ms: int,
    timestamp: int,
    tags_json: str,
    conn: sqlite3.Connection,
) -> None:
    """Never-raise wrapper around _record_inner. Retries once on SQLite lock."""
    try:
        _record_inner(command, directory, exit_code, duration_ms, timestamp, tags_json, conn)
    except sqlite3.OperationalError:
        # Rollback the dirty transaction before retrying; orphan session rows are a known follow-up.
        try:
            conn.rollback()
        except Exception:
            pass  # guard: rollback must not escape
        try:
            _record_inner(command, directory, exit_code, duration_ms, timestamp, tags_json, conn)
        except Exception as exc2:
            _log_error(exc2)
    except Exception as exc:
        _log_error(exc)



@app.command()
def record_cmd(
    cmd: str = typer.Option(..., "--cmd", help="The shell command that was executed"),
    dir_: str = typer.Option(None, "--dir", help="Working directory (default: cwd)"),
    exit_: int = typer.Option(0, "--exit", help="Exit code of the command"),
    duration: int = typer.Option(0, "--duration", help="Duration in milliseconds"),
    timestamp: int = typer.Option(None, "--timestamp", help="Unix timestamp (default: now)"),
    tags: str = typer.Option("[]", "--tags", help="JSON array of tags"),
) -> None:
    """Record a shell command into Thoth's history database."""
    conn = None
    try:
        if dir_ is None:
            dir_ = os.getcwd()
        if timestamp is None:
            timestamp = int(time.time())
        conn = get_connection()
        record(cmd, dir_, exit_, duration, timestamp, tags, conn)
    except Exception:
        pass
    finally:
        if conn is not None:
            try:
                conn.close()
            except Exception:
                pass
    raise typer.Exit(0)
