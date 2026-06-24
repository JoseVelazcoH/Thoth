from pathlib import Path


def test_thoth_db_env_override(tmp_path, monkeypatch):
    from tth import database

    custom = str(tmp_path / "custom.db")
    monkeypatch.setenv("THOTH_DB", custom)
    resolved = database._resolve_db_path()
    assert str(resolved) == custom


def test_thoth_db_xdg_data_home(tmp_path, monkeypatch):
    from tth import database

    monkeypatch.delenv("THOTH_DB", raising=False)
    monkeypatch.setenv("XDG_DATA_HOME", str(tmp_path / "xdg"))
    resolved = database._resolve_db_path()
    assert resolved == tmp_path / "xdg" / "thoth" / "history.db"


def test_thoth_db_fallback(monkeypatch):
    from tth import database

    monkeypatch.delenv("THOTH_DB", raising=False)
    monkeypatch.delenv("XDG_DATA_HOME", raising=False)
    resolved = database._resolve_db_path()
    assert resolved == Path("~/.local/share/thoth/history.db").expanduser()


def test_thoth_error_log_env_override(tmp_path, monkeypatch):
    from tth import recorder

    custom = str(tmp_path / "custom.log")
    monkeypatch.setenv("THOTH_ERROR_LOG", custom)
    resolved = recorder._resolve_error_log()
    assert str(resolved) == custom


def test_thoth_error_log_xdg(tmp_path, monkeypatch):
    from tth import recorder

    monkeypatch.delenv("THOTH_ERROR_LOG", raising=False)
    monkeypatch.setenv("XDG_DATA_HOME", str(tmp_path / "xdg"))
    resolved = recorder._resolve_error_log()
    assert resolved == tmp_path / "xdg" / "thoth" / "error.log"


def test_thoth_error_log_fallback(monkeypatch):
    from tth import recorder

    monkeypatch.delenv("THOTH_ERROR_LOG", raising=False)
    monkeypatch.delenv("XDG_DATA_HOME", raising=False)
    resolved = recorder._resolve_error_log()
    assert resolved == Path("~/.local/share/thoth/error.log").expanduser()
