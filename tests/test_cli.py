"""CLI integration tests for `tth record` (Domain 4)."""

import os
import sqlite3
import sys

import pytest

from tth.database import apply_migrations


def _make_conn(db_path):
    conn = sqlite3.connect(str(db_path), check_same_thread=False)
    conn.row_factory = sqlite3.Row
    apply_migrations(conn)
    return conn


@pytest.fixture
def temp_db(tmp_path, monkeypatch):
    """Provide a temp DB path and monkeypatch get_connection to use it."""
    db_path = tmp_path / "test.db"

    def _patched_get_connection(path=None):
        return _make_conn(db_path)

    monkeypatch.setattr("tth.cli.get_connection", _patched_get_connection)
    return db_path


def _invoke(argv):
    """Call cli.main() and return (exit_code, raised_exception)."""
    from tth import cli
    try:
        cli.main(argv)
        return 0, None
    except SystemExit as e:
        return (e.code if e.code is not None else 0), None
    except Exception as exc:
        return 1, exc


def test_typer_not_imported():
    """After importing tth.cli, typer and click must NOT be in sys.modules."""
    # Force a fresh import by removing cached modules if present.
    for mod in list(sys.modules.keys()):
        if mod == "tth.cli" or mod.startswith("typer") or mod.startswith("click"):
            del sys.modules[mod]
    import tth.cli  # noqa: F401
    assert "typer" not in sys.modules, "typer was imported by tth.cli (hot-path regression)"
    assert "click" not in sys.modules, "click was imported by tth.cli (hot-path regression)"


def test_cli_exits_0_on_failure(monkeypatch, tmp_path_factory):
    """Even if get_connection raises, CLI must exit 0."""
    log_dir = tmp_path_factory.mktemp("cli_fail_log")
    log_file = log_dir / "error.log"

    monkeypatch.setattr(
        "tth.cli.get_connection",
        lambda: (_ for _ in ()).throw(RuntimeError("db boom")),
    )
    monkeypatch.setattr("tth.recorder.ERROR_LOG", log_file)

    code, exc = _invoke(["record", "--cmd", "echo test"])
    assert code == 0, f"Expected exit 0, got {code} (exc={exc})"


def test_setup_failure_logs_and_exits_0(monkeypatch, tmp_path_factory):
    """get_connection() raising must log the error AND keep exit code 0."""
    log_dir = tmp_path_factory.mktemp("setup_fail_logs")
    log_file = log_dir / "error.log"

    monkeypatch.setattr(
        "tth.cli.get_connection",
        lambda: (_ for _ in ()).throw(RuntimeError("disk full")),
    )
    monkeypatch.setattr("tth.recorder.ERROR_LOG", log_file)

    code, _ = _invoke(["record", "--cmd", "echo hi"])
    assert code == 0
    assert log_file.exists(), "Error log must be written on setup failure"
    assert "disk full" in log_file.read_text()


def test_cli_all_six_flags(temp_db):
    code, exc = _invoke([
        "record",
        "--cmd", "make build",
        "--dir", "/home/user/project",
        "--exit", "1",
        "--duration", "3500",
        "--timestamp", "1700001000",
        "--tags", '["ci","build"]',
    ])
    assert code == 0, f"Exit {code}, exc={exc}"
    conn = sqlite3.connect(str(temp_db))
    conn.row_factory = sqlite3.Row
    row = conn.execute(
        "SELECT exit_code, duration_ms, tags FROM commands WHERE command='make build'"
    ).fetchone()
    conn.close()
    assert row is not None
    assert row["exit_code"] == 1
    assert row["duration_ms"] == 3500
    assert row["tags"] == '["ci","build"]'


def test_cli_default_dir_is_cwd(temp_db, monkeypatch, tmp_path):
    """Omitting --dir should store the real cwd as directory (hermetic, no /tmp)."""
    monkeypatch.chdir(tmp_path)
    code, exc = _invoke(["record", "--cmd", "pwd"])
    assert code == 0, f"Exit {code}, exc={exc}"
    conn = sqlite3.connect(str(temp_db))
    conn.row_factory = sqlite3.Row
    row = conn.execute(
        "SELECT directory FROM commands WHERE command='pwd'"
    ).fetchone()
    conn.close()
    assert row is not None
    expected = os.path.realpath(str(tmp_path))
    assert row["directory"] == expected, (
        f"Expected directory '{expected}', got '{row['directory']}'"
    )
