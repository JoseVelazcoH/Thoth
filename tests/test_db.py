import pytest
from unittest.mock import patch

from tth.database import connect, apply_migrations, current_version, fts5_available, MIGRATIONS


def _table_names(conn):
    rows = conn.execute(
        "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name"
    ).fetchall()
    return {row[0] for row in rows}


def test_schema_created_from_empty():
    conn = connect()
    apply_migrations(conn)
    tables = _table_names(conn)
    assert {"commands", "sessions", "projects", "schema_version"}.issubset(tables)
    conn.close()



def test_idempotent_rerun(mem_conn):
    apply_migrations(mem_conn)  # second call
    rows = mem_conn.execute("SELECT COUNT(*) FROM schema_version").fetchone()[0]
    assert rows == len([v for v, _ in MIGRATIONS])


def test_pending_migrations_applied():
    conn = connect()
    dummy_v1 = "CREATE TABLE IF NOT EXISTS schema_version (version INTEGER PRIMARY KEY, applied_at INTEGER NOT NULL);"
    dummy_v2 = "CREATE TABLE IF NOT EXISTS _dummy_v2 (x INTEGER);"
    fake_migrations = [(1, dummy_v1), (2, dummy_v2)]
    with patch("tth.database.MIGRATIONS", fake_migrations):
        apply_migrations(conn)
    assert current_version(conn) == 2
    conn.close()


def test_fts5_insert_sync(mem_conn):
    if not fts5_available(mem_conn):
        pytest.skip("FTS5 not available")
    mem_conn.execute(
        "INSERT INTO commands(command, directory, project, session_id, timestamp, exit_code, duration_ms, tags) "
        "VALUES('git status', '/home/user/app', 'app', 'sess-1', 1700000000, 0, 10, '[]')"
    )
    mem_conn.commit()
    row = mem_conn.execute(
        "SELECT rowid FROM commands_fts WHERE commands_fts MATCH 'git'"
    ).fetchone()
    assert row is not None


def test_fts5_delete_sync(mem_conn):
    if not fts5_available(mem_conn):
        pytest.skip("FTS5 not available")
    mem_conn.execute(
        "INSERT INTO commands(command, directory, project, session_id, timestamp, exit_code, duration_ms, tags) "
        "VALUES('unique-cmd-xyz', '/home/user/app', 'app', 'sess-2', 1700000001, 0, 5, '[]')"
    )
    mem_conn.commit()
    cmd_id = mem_conn.execute(
        "SELECT id FROM commands WHERE command='unique-cmd-xyz'"
    ).fetchone()[0]
    mem_conn.execute("DELETE FROM commands WHERE id=?", (cmd_id,))
    mem_conn.commit()
    row = mem_conn.execute(
        "SELECT rowid FROM commands_fts WHERE commands_fts MATCH '\"unique-cmd-xyz\"'"
    ).fetchone()
    assert row is None


def test_fts5_unavailable_no_raise():
    conn = connect()
    with patch("tth.database.fts5_available", return_value=False):
        apply_migrations(conn)  # should not raise
    conn.close()


def test_migration_is_atomic():
    """A migration whose second statement is invalid must not advance schema_version."""
    conn = connect()

    # First migration: valid — creates schema_version so current_version() works
    valid_v1 = (
        "CREATE TABLE IF NOT EXISTS schema_version "
        "(version INTEGER PRIMARY KEY, applied_at INTEGER NOT NULL);"
    )
    # Second migration: first statement valid, second statement intentionally invalid.
    # If atomicity is broken the table will be created but schema_version will be wrong.
    bad_v2 = (
        "CREATE TABLE IF NOT EXISTS partial_table (x INTEGER);"
        "THIS IS NOT VALID SQL;"
    )
    fake_migrations = [(1, valid_v1), (2, bad_v2)]

    with patch("tth.database.MIGRATIONS", fake_migrations):
        try:
            apply_migrations(conn)
        except Exception:
            pass  # allowed to raise; what matters is the DB state

    # Schema version must NOT have advanced to 2 (atomicity failed → rollback)
    ver = current_version(conn)
    assert ver != 2, f"schema_version advanced to {ver} despite bad SQL — migration is not atomic"

    # The partial DDL must also have been rolled back
    tables = _table_names(conn)
    assert "partial_table" not in tables, "partial_table exists after failed migration — DDL was not rolled back"

    conn.close()


def test_trigger_with_case_end_splits_correctly():
    """_split_sql must not confuse CASE...END with BEGIN...END trigger nesting.

    A trigger body that contains CASE WHEN ... THEN ... ELSE ... END must be
    recognised as a single statement and applied without error.  The old depth-
    counter matched any END keyword, so the CASE END prematurely closed the
    trigger's BEGIN...END block and the trigger DDL was silently split wrong.
    """
    conn = connect()

    # Migration 1 creates schema_version + the base commands table we need.
    apply_migrations(conn)

    # A second migration adds a trigger whose body uses CASE WHEN ... END.
    trigger_sql = """
CREATE TABLE IF NOT EXISTS audit_log (
    id    INTEGER PRIMARY KEY AUTOINCREMENT,
    label TEXT    NOT NULL
);

CREATE TRIGGER IF NOT EXISTS commands_audit AFTER INSERT ON commands BEGIN
    INSERT INTO audit_log(label)
    VALUES(
        CASE
            WHEN new.exit_code = 0 THEN 'ok'
            ELSE 'fail'
        END
    );
END;
"""
    from unittest.mock import patch
    fake_migrations = list(MIGRATIONS) + [(99, trigger_sql)]
    with patch("tth.database.MIGRATIONS", fake_migrations):
        apply_migrations(conn)  # must not raise

    # The trigger must exist.
    row = conn.execute(
        "SELECT name FROM sqlite_master WHERE type='trigger' AND name='commands_audit'"
    ).fetchone()
    assert row is not None, "trigger commands_audit was not created — CASE END split the body"

    # Insert a row to exercise the trigger.
    conn.execute(
        "INSERT INTO commands(command, directory, project, session_id, timestamp, exit_code, duration_ms, tags) "
        "VALUES('ls', '/tmp', 'p', 'sid', 1700000000, 0, 1, '[]')"
    )
    conn.commit()
    audit = conn.execute("SELECT label FROM audit_log").fetchone()
    assert audit is not None and audit[0] == "ok"

    conn.close()


def test_string_literal_with_semicolon_begin_and_comment():
    """A statement containing an embedded ';', 'BEGIN', and '--' in a string
    literal must be treated as ONE statement, not split in the middle.

    The old splitter stripped '--' via line.split('--')[0], which would
    truncate any string value containing that sequence, and it didn't track
    quote state, so a bare ';' inside quotes would incorrectly end the statement.
    """
    conn = connect()
    apply_migrations(conn)

    # A migration that inserts a row with tricky text in a string literal.
    tricky_value = "hello; BEGIN -- not a comment"
    literal_sql = (
        "CREATE TABLE IF NOT EXISTS string_test (val TEXT NOT NULL);\n"
        f"INSERT INTO string_test(val) VALUES('{tricky_value}');\n"
    )
    from unittest.mock import patch
    fake_migrations = list(MIGRATIONS) + [(98, literal_sql)]
    with patch("tth.database.MIGRATIONS", fake_migrations):
        apply_migrations(conn)  # must not raise

    row = conn.execute("SELECT val FROM string_test").fetchone()
    assert row is not None, "string_test row was not inserted"
    assert row[0] == tricky_value, f"value was mangled: {row[0]!r}"

    conn.close()
