import logging
from unittest.mock import patch


def test_fts5_warning_does_not_reach_stderr(tmp_path, monkeypatch, capsys):
    import tth.database as db_mod

    log_file = tmp_path / "error.log"
    monkeypatch.setenv("THOTH_ERROR_LOG", str(log_file))

    from tth import logging_config
    logging_config.setup(log_file)

    with patch.object(db_mod, "fts5_available", return_value=False):
        from tth.database import connect, apply_migrations
        conn = connect()
        apply_migrations(conn)
        conn.close()

    captured = capsys.readouterr()
    assert "FTS5" not in captured.err, (
        f"FTS5 warning leaked to stderr: {captured.err!r}"
    )


def test_warning_goes_to_log_file(tmp_path):
    log_file = tmp_path / "warn.log"
    from tth import logging_config
    logging_config.setup(log_file)

    logger = logging.getLogger("tth.test_warning_routing")
    logger.warning("sentinel-warning-xyz")

    for h in logging.getLogger("tth").handlers:
        h.flush()

    assert log_file.exists(), "Log file was not created"
    content = log_file.read_text()
    assert "sentinel-warning-xyz" in content, f"Warning not in log: {content!r}"


def test_log_rotation_bounds_file_size(tmp_path):
    log_file = tmp_path / "rotate.log"
    from tth import logging_config

    max_bytes = 10_000
    logging_config.setup(log_file, max_bytes=max_bytes, backup_count=2)

    logger = logging.getLogger("tth.rotation_test")
    line = "x" * 200
    for _ in range(200):
        logger.warning(line)

    for h in logging.getLogger("tth").handlers:
        h.flush()

    assert log_file.exists()
    assert log_file.stat().st_size <= max_bytes * 2, (
        f"Log file too large: {log_file.stat().st_size}"
    )
