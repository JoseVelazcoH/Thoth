"""CLI integration tests for `tth record` (Domain 4)."""

import os
import sqlite3

import pytest
from typer.testing import CliRunner

from tth.recorder import app
from tth.db import apply_migrations


runner = CliRunner()


@pytest.fixture
def temp_db(tmp_path, monkeypatch):
    """Provide a temp DB path and monkeypatch get_connection to use it."""
    db_path = tmp_path / "test.db"

    def _patched_get_connection(path=None):
        conn = sqlite3.connect(str(db_path), check_same_thread=False)
        conn.row_factory = sqlite3.Row
        apply_migrations(conn)
        return conn

    monkeypatch.setattr("tth.recorder.get_connection", _patched_get_connection)
    return db_path



def test_cli_exits_0_on_failure(monkeypatch):
    """Even if get_connection raises, CLI must exit 0."""
    monkeypatch.setattr(
        "tth.recorder.get_connection",
        lambda: (_ for _ in ()).throw(RuntimeError("db boom")),
    )
    result = runner.invoke(app, ["--cmd", "echo test"])
    assert result.exit_code == 0


def test_cli_all_six_flags(temp_db):
    result = runner.invoke(
        app,
        [
            "--cmd", "make build",
            "--dir", "/home/user/project",
            "--exit", "1",
            "--duration", "3500",
            "--timestamp", "1700001000",
            "--tags", '["ci","build"]',
        ],
    )
    assert result.exit_code == 0
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
    result = runner.invoke(app, ["--cmd", "pwd"])
    assert result.exit_code == 0
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
