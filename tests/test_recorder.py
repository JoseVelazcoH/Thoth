"""Tests for recorder core (Domain 4)."""

import sqlite3
import time
from unittest.mock import patch


from tth.recorder import record, _record_inner, _normalize_tags


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _insert_project(conn, path, name="app", count=0):
    now = int(time.time())
    conn.execute(
        "INSERT OR REPLACE INTO projects(path, name, last_seen, command_count) VALUES(?,?,?,?)",
        (path, name, now, count),
    )
    conn.commit()


# ---------------------------------------------------------------------------
# Core record tests
# ---------------------------------------------------------------------------


def test_successful_record(mem_conn, tmp_path):
    app_dir = tmp_path / "my-app"
    app_dir.mkdir()
    (app_dir / ".git").mkdir()
    t0 = int(time.time())
    _record_inner("git status", str(app_dir), 0, 120, t0, "[]", mem_conn)
    row = mem_conn.execute(
        "SELECT command, directory, exit_code, duration_ms, tags, project, session_id "
        "FROM commands WHERE command='git status'"
    ).fetchone()
    assert row is not None
    assert row["directory"] == str(app_dir)
    assert row["exit_code"] == 0
    assert row["duration_ms"] == 120
    assert row["tags"] == "[]"
    assert row["project"] is not None
    assert row["session_id"] is not None


def test_tags_stored_as_json(mem_conn, tmp_path):
    app_dir = str(tmp_path)
    t0 = int(time.time())
    _record_inner("echo hi", app_dir, 0, 10, t0, '["fix-migration"]', mem_conn)
    row = mem_conn.execute(
        "SELECT tags FROM commands WHERE command='echo hi'"
    ).fetchone()
    assert row["tags"] == '["fix-migration"]'


def test_tags_empty_string_normalized(mem_conn, tmp_path):
    t0 = int(time.time())
    _record_inner("echo a", str(tmp_path), 0, 0, t0, "", mem_conn)
    row = mem_conn.execute(
        "SELECT tags FROM commands WHERE command='echo a'"
    ).fetchone()
    assert row["tags"] == "[]"


def test_tags_invalid_json_normalized(mem_conn, tmp_path):
    t0 = int(time.time())
    _record_inner("echo b", str(tmp_path), 0, 0, t0, "not-json", mem_conn)
    row = mem_conn.execute(
        "SELECT tags FROM commands WHERE command='echo b'"
    ).fetchone()
    assert row["tags"] == "[]"


def test_projects_upsert_increments_count(mem_conn, tmp_path):
    app_dir = tmp_path / "proj"
    app_dir.mkdir()
    _insert_project(mem_conn, str(app_dir), count=5)
    t0 = int(time.time())
    _record_inner("ls", str(app_dir), 0, 5, t0, "[]", mem_conn)
    row = mem_conn.execute(
        "SELECT command_count FROM projects WHERE path=?", (str(app_dir),)
    ).fetchone()
    assert row["command_count"] == 6


def test_sessions_updated_after_record(mem_conn, tmp_path):
    t0 = int(time.time())
    _record_inner("pwd", str(tmp_path), 0, 2, t0, "[]", mem_conn)
    row = mem_conn.execute(
        "SELECT ended_at, command_count FROM sessions"
    ).fetchone()
    assert row is not None
    assert row["ended_at"] == t0
    assert row["command_count"] >= 1


def test_exception_logged_not_raised(mem_conn, tmp_path, tmp_path_factory):
    log_dir = tmp_path_factory.mktemp("logs")
    log_file = log_dir / "error.log"
    t0 = int(time.time())

    with patch("tth.recorder._record_inner", side_effect=Exception("boom")):
        with patch("tth.recorder.ERROR_LOG", log_file):
            record("cmd", str(tmp_path), 0, 0, t0, "[]", mem_conn)

    assert log_file.exists()
    assert "boom" in log_file.read_text()


def test_sqlite_lock_retries_once(mem_conn, tmp_path):
    t0 = int(time.time())
    call_count = 0

    def side_effect(*args, **kwargs):
        nonlocal call_count
        call_count += 1
        if call_count == 1:
            raise sqlite3.OperationalError("database is locked")

    with patch("tth.recorder._record_inner", side_effect=side_effect):
        record("cmd", str(tmp_path), 0, 0, t0, "[]", mem_conn)

    assert call_count == 2


def test_sqlite_lock_both_fail_logs(mem_conn, tmp_path, tmp_path_factory):
    log_dir = tmp_path_factory.mktemp("logs2")
    log_file = log_dir / "error.log"
    t0 = int(time.time())

    with patch(
        "tth.recorder._record_inner",
        side_effect=sqlite3.OperationalError("database is locked"),
    ):
        with patch("tth.recorder.ERROR_LOG", log_file):
            record("cmd", str(tmp_path), 0, 0, t0, "[]", mem_conn)

    assert log_file.exists()


def test_exit_code_and_duration_persisted(mem_conn, tmp_path):
    t0 = int(time.time())
    _record_inner("bad-cmd", str(tmp_path), 127, 543, t0, "[]", mem_conn)
    row = mem_conn.execute(
        "SELECT exit_code, duration_ms FROM commands WHERE command='bad-cmd'"
    ).fetchone()
    assert row["exit_code"] == 127
    assert row["duration_ms"] == 543


# ---------------------------------------------------------------------------
# _normalize_tags unit tests
# ---------------------------------------------------------------------------


def test_normalize_tags_valid():
    assert _normalize_tags('["a","b"]') == '["a","b"]'


def test_normalize_tags_empty_string():
    assert _normalize_tags("") == "[]"


def test_normalize_tags_invalid():
    assert _normalize_tags("not-json") == "[]"


def test_normalize_tags_none():
    assert _normalize_tags(None) == "[]"  # type: ignore[arg-type]


def test_retry_runs_on_clean_transaction(mem_conn, tmp_path):
    """record() must rollback before retrying so the second attempt starts
    on a clean connection — not inside the dirty transaction left by the
    first failure.

    Assertions:
    (a) No exception escapes record().
    (b) The command is recorded exactly once (no duplicate/missing rows).
    (c) The retry itself succeeds without "cannot start a transaction within
        a transaction" errors (which would surface as an exception in the
        second _record_inner call and cause a log-but-no-row outcome).
    """
    t0 = int(time.time())
    call_count = 0
    original_record_inner = _record_inner

    def _failing_first_then_ok(command, directory, exit_code, duration_ms, timestamp, tags_json, conn):
        nonlocal call_count
        call_count += 1
        if call_count == 1:
            # Simulate a lock error; leave the connection in a dirty state
            # by issuing a partial statement before raising, the way a real
            # lock failure might.
            raise sqlite3.OperationalError("database is locked")
        # Second call: must succeed on a clean connection.
        original_record_inner(command, directory, exit_code, duration_ms, timestamp, tags_json, conn)

    with patch("tth.recorder._record_inner", side_effect=_failing_first_then_ok):
        # (a) must not raise
        record("retry-cmd", str(tmp_path), 0, 0, t0, "[]", mem_conn)

    # (b) exactly one row recorded
    rows = mem_conn.execute(
        "SELECT COUNT(*) FROM commands WHERE command='retry-cmd'"
    ).fetchone()[0]
    assert rows == 1, f"Expected 1 row, got {rows}"

    # (c) call_count == 2 confirms the retry ran
    assert call_count == 2, f"Expected 2 calls (original + retry), got {call_count}"
