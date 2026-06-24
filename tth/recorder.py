import json
import os
import sqlite3
import traceback
from pathlib import Path

from tth.project import infer_project
from tth.session import get_or_create


def _resolve_error_log() -> Path:
    override = os.environ.get("THOTH_ERROR_LOG")
    if override:
        return Path(override)
    xdg = os.environ.get("XDG_DATA_HOME")
    if xdg:
        return Path(xdg) / "thoth" / "error.log"
    return Path("~/.local/share/thoth/error.log").expanduser()


ERROR_LOG = _resolve_error_log()


def _normalize_tags(tags_json) -> str:
    if not tags_json:
        return "[]"
    try:
        parsed = json.loads(tags_json)
        if not isinstance(parsed, list) or not all(isinstance(e, str) for e in parsed):
            return "[]"
        return tags_json
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
    tags = _normalize_tags(tags_json)
    conn.execute("BEGIN IMMEDIATE")
    try:
        project = infer_project(directory, conn)
        sid = get_or_create(project, timestamp, conn)

        conn.execute(
            "INSERT INTO commands(command, directory, project, session_id, timestamp, exit_code, duration_ms, tags) "
            "VALUES(?, ?, ?, ?, ?, ?, ?, ?)",
            (command, directory, project, sid, timestamp, exit_code, duration_ms, tags),
        )

        conn.execute(
            "INSERT INTO projects(path, name, last_seen, command_count) VALUES(?, ?, ?, 1) "
            "ON CONFLICT(path) DO UPDATE SET "
            "name=excluded.name, last_seen=excluded.last_seen, command_count=command_count + 1",
            (directory, project, timestamp),
        )

        conn.execute(
            "UPDATE sessions SET ended_at=?, command_count=command_count + 1 WHERE session_id=?",
            (timestamp, sid),
        )

        conn.commit()
    except Exception:
        conn.rollback()
        raise


def _log_error(exc: Exception) -> None:
    try:
        ERROR_LOG.parent.mkdir(parents=True, exist_ok=True)
        with ERROR_LOG.open("a", encoding="utf-8") as f:
            f.write(traceback.format_exc())
            f.write(f"\n{exc}\n")
    except Exception:
        pass


def record(
    command: str,
    directory: str,
    exit_code: int,
    duration_ms: int,
    timestamp: int,
    tags_json: str,
    conn: sqlite3.Connection,
) -> None:
    try:
        _record_inner(command, directory, exit_code, duration_ms, timestamp, tags_json, conn)
    except sqlite3.OperationalError:
        try:
            conn.rollback()
        except Exception:
            pass
        try:
            _record_inner(command, directory, exit_code, duration_ms, timestamp, tags_json, conn)
        except Exception as exc2:
            _log_error(exc2)
    except Exception as exc:
        _log_error(exc)
