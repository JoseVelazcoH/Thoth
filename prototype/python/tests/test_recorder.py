import sqlite3
import time
from unittest.mock import patch


from tth.recorder import record, _record_inner, _normalize_tags


def _insert_project(conn, path, name="app", count=0):
    now = int(time.time())
    conn.execute(
        "INSERT OR REPLACE INTO projects(path, name, last_seen, command_count) VALUES(?,?,?,?)",
        (path, name, now, count),
    )
    conn.commit()


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
    content = log_file.read_text()
    assert content, "Error log must not be empty when both attempts fail"
    assert "locked" in content, f"Expected error text in log, got: {content!r}"


def test_normalize_tags_valid():
    assert _normalize_tags('["a","b"]') == '["a","b"]'


def test_normalize_tags_empty_string():
    assert _normalize_tags("") == "[]"


def test_normalize_tags_invalid():
    assert _normalize_tags("not-json") == "[]"


def test_normalize_tags_none():
    assert _normalize_tags(None) == "[]"  # type: ignore[arg-type]


def test_normalize_tags_integer_elements_rejected():
    assert _normalize_tags("[1, 2]") == "[]"


def test_normalize_tags_nested_element_rejected():
    assert _normalize_tags('["a", {"b": 1}]') == "[]"


def test_normalize_tags_valid_strings_preserved_verbatim():
    raw = '["a","b"]'
    assert _normalize_tags(raw) == raw


def test_retry_runs_on_clean_transaction(mem_conn, tmp_path):
    t0 = int(time.time())
    call_count = 0
    original_record_inner = _record_inner

    def _failing_first_then_ok(command, directory, exit_code, duration_ms, timestamp, tags_json, conn):
        nonlocal call_count
        call_count += 1
        if call_count == 1:
            raise sqlite3.OperationalError("database is locked")
        original_record_inner(command, directory, exit_code, duration_ms, timestamp, tags_json, conn)

    with patch("tth.recorder._record_inner", side_effect=_failing_first_then_ok):
        record("retry-cmd", str(tmp_path), 0, 0, t0, "[]", mem_conn)

    rows = mem_conn.execute(
        "SELECT COUNT(*) FROM commands WHERE command='retry-cmd'"
    ).fetchone()[0]
    assert rows == 1, f"Expected 1 row, got {rows}"

    assert call_count == 2, f"Expected 2 calls (original + retry), got {call_count}"


class _FailOnCommandInsert:
    def __init__(self, real_conn, *, fail_count=1):
        self._conn = real_conn
        self._fail_count = fail_count
        self._calls = 0

    def execute(self, sql, params=()):
        if sql.strip().upper().startswith("INSERT INTO COMMANDS"):
            self._calls += 1
            if self._calls <= self._fail_count:
                raise sqlite3.OperationalError("simulated command insert failure")
        return self._conn.execute(sql, params)

    def commit(self):
        return self._conn.commit()

    def rollback(self):
        return self._conn.rollback()

    def close(self):
        return self._conn.close()

    def __getattr__(self, name):
        return getattr(self._conn, name)


def test_atomicity_command_failure_leaves_no_session(mem_conn, tmp_path):
    t0 = int(time.time())
    failing_conn = _FailOnCommandInsert(mem_conn, fail_count=1)

    try:
        _record_inner("atomicity-test", str(tmp_path), 0, 0, t0, "[]", failing_conn)
    except Exception:
        pass

    sessions = mem_conn.execute("SELECT COUNT(*) FROM sessions").fetchone()[0]
    commands = mem_conn.execute(
        "SELECT COUNT(*) FROM commands WHERE command='atomicity-test'"
    ).fetchone()[0]
    assert sessions == 0, f"Orphan session found after rollback (got {sessions})"
    assert commands == 0, f"Orphan command found after rollback (got {commands})"


def test_atomicity_retry_produces_exactly_one_row(mem_conn, tmp_path, tmp_path_factory):
    log_dir = tmp_path_factory.mktemp("atom_logs")
    log_file = log_dir / "error.log"
    t0 = int(time.time())

    call_count = 0
    original = _record_inner

    def _fail_once(command, directory, exit_code, duration_ms, timestamp, tags_json, conn):
        nonlocal call_count
        call_count += 1
        if call_count == 1:
            raise sqlite3.OperationalError("simulated lock")
        original(command, directory, exit_code, duration_ms, timestamp, tags_json, conn)

    with patch("tth.recorder._record_inner", side_effect=_fail_once):
        with patch("tth.recorder.ERROR_LOG", log_file):
            record("retry-atomicity", str(tmp_path), 0, 0, t0, "[]", mem_conn)

    sessions = mem_conn.execute("SELECT COUNT(*) FROM sessions").fetchone()[0]
    commands = mem_conn.execute(
        "SELECT COUNT(*) FROM commands WHERE command='retry-atomicity'"
    ).fetchone()[0]
    assert sessions == 1, f"Expected 1 session after retry, got {sessions}"
    assert commands == 1, f"Expected 1 command after retry, got {commands}"
    assert call_count == 2, f"Expected 2 calls, got {call_count}"
