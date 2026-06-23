"""Tests for project inference (Domain 2)."""

import json
import time
from unittest.mock import patch


from tth.project import infer_project, _walk_markers


# ---------------------------------------------------------------------------
# Marker detection tests
# ---------------------------------------------------------------------------


def test_git_marker(tmp_path):
    project_dir = tmp_path / "my-app"
    project_dir.mkdir()
    (project_dir / ".git").mkdir()
    result = _walk_markers(str(project_dir))
    assert result == "my-app"


def test_package_json_over_pyproject(tmp_path):
    project_dir = tmp_path / "frontend"
    project_dir.mkdir()
    (project_dir / "package.json").write_text(json.dumps({"name": "frontend-app"}))
    (project_dir / "pyproject.toml").write_text('[project]\nname = "backend"\n')
    result = _walk_markers(str(project_dir))
    assert result == "frontend-app"


def test_pyproject_project_name(tmp_path):
    project_dir = tmp_path / "mylib"
    project_dir.mkdir()
    (project_dir / "pyproject.toml").write_text('[project]\nname = "foo"\n')
    result = _walk_markers(str(project_dir))
    assert result == "foo"


def test_pyproject_poetry_name(tmp_path):
    project_dir = tmp_path / "poetic"
    project_dir.mkdir()
    (project_dir / "pyproject.toml").write_text('[tool.poetry]\nname = "bar"\n')
    result = _walk_markers(str(project_dir))
    assert result == "bar"


def test_cargo_toml(tmp_path):
    project_dir = tmp_path / "rustproj"
    project_dir.mkdir()
    (project_dir / "Cargo.toml").write_text('[package]\nname = "mycrate"\n')
    result = _walk_markers(str(project_dir))
    assert result == "mycrate"


def test_go_mod(tmp_path):
    project_dir = tmp_path / "goprj"
    project_dir.mkdir()
    (project_dir / "go.mod").write_text("module github.com/org/mylib\n\ngo 1.21\n")
    result = _walk_markers(str(project_dir))
    assert result == "mylib"


def test_docker_compose(tmp_path):
    project_dir = tmp_path / "myservice"
    project_dir.mkdir()
    (project_dir / "docker-compose.yml").write_text("version: '3'\n")
    result = _walk_markers(str(project_dir))
    assert result == "myservice"


def test_fallback_ungrouped(tmp_path):
    project_dir = tmp_path / "emptyfolder"
    project_dir.mkdir()
    result = _walk_markers(str(project_dir))
    assert result == "ungrouped"


def test_depth_cap(tmp_path):
    """Walk must stop at 20 levels and return 'ungrouped', no error."""
    deep = tmp_path
    for i in range(25):
        deep = deep / f"level{i}"
        deep.mkdir()
    result = _walk_markers(str(deep))
    assert result == "ungrouped"


# ---------------------------------------------------------------------------
# Cache tests (use mem_conn fixture from conftest)
# ---------------------------------------------------------------------------


def test_cache_hit(mem_conn, tmp_path):
    """Cache hit: last_seen < 24h ago -> no FS walk."""
    project_dir = str(tmp_path / "cached-app")
    now = int(time.time())
    one_hour_ago = now - 3600
    mem_conn.execute(
        "INSERT INTO projects(path, name, last_seen, command_count) VALUES(?, ?, ?, ?)",
        (project_dir, "cached-app", one_hour_ago, 0),
    )
    mem_conn.commit()

    with patch("tth.project._walk_markers", side_effect=RuntimeError("should not walk")):
        result = infer_project(project_dir, mem_conn)

    assert result == "cached-app"


def test_cache_miss_stale(mem_conn, tmp_path):
    """Stale cache (>24h): must re-walk."""
    project_dir = tmp_path / "stale-app"
    project_dir.mkdir()
    (project_dir / "pyproject.toml").write_text('[project]\nname = "fresh-name"\n')

    now = int(time.time())
    stale_time = now - (25 * 3600)
    mem_conn.execute(
        "INSERT INTO projects(path, name, last_seen, command_count) VALUES(?, ?, ?, ?)",
        (str(project_dir), "old-name", stale_time, 0),
    )
    mem_conn.commit()

    result = infer_project(str(project_dir), mem_conn)
    assert result == "fresh-name"


def test_command_count_not_reset_on_refresh(mem_conn, tmp_path):
    """A stale-cache refresh must never reset command_count to 0 or 1.

    The recorder's upsert uses command_count=command_count+1, so after a
    re-walk the count must be >= the count before the refresh, never reset.
    """
    import time
    from tth.recorder import _record_inner

    project_dir = tmp_path / "counted-app"
    project_dir.mkdir()
    (project_dir / "pyproject.toml").write_text('[project]\nname = "counted-app"\n')

    now = int(time.time())
    stale_time = now - (25 * 3600)

    # Seed project with command_count=5 and a stale last_seen
    mem_conn.execute(
        "INSERT INTO projects(path, name, last_seen, command_count) VALUES(?, ?, ?, ?)",
        (str(project_dir), "counted-app", stale_time, 5),
    )
    mem_conn.commit()

    # Trigger a record which exercises infer_project (cache miss → re-walk) + the upsert
    _record_inner(
        command="ls",
        directory=str(project_dir),
        exit_code=0,
        duration_ms=1,
        timestamp=now,
        tags_json="[]",
        conn=mem_conn,
    )

    row = mem_conn.execute(
        "SELECT command_count FROM projects WHERE path=?", (str(project_dir),)
    ).fetchone()
    assert row is not None
    count = row[0]
    # Must be incremented (was 5, +1 = 6), never reset to 0 or 1
    assert count >= 6, f"command_count was reset to {count}; expected >= 6"


def test_malformed_pyproject_continues(tmp_path):
    """Invalid TOML in pyproject.toml -> no raise, return ungrouped (no parent markers)."""
    project_dir = tmp_path / "broken"
    project_dir.mkdir()
    (project_dir / "pyproject.toml").write_text("this is not toml [[[\n")
    result = _walk_markers(str(project_dir))
    # Malformed -> skip, no other markers, fallback
    assert result == "ungrouped"
