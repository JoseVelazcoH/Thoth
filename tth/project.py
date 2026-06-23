"""Project inference: walk directory markers, 24-hour cache."""

import json
import sqlite3
import time
import tomllib
from pathlib import Path


_MAX_WALK_DEPTH = 20
_CACHE_TTL_SECONDS = 86400  # 24 hours


# ---------------------------------------------------------------------------
# Public API
# ---------------------------------------------------------------------------


def infer_project(cwd: str, conn: sqlite3.Connection) -> str:
    """Return project name for cwd. Uses 24h cache; miss triggers FS walk."""
    now = int(time.time())
    cutoff = now - _CACHE_TTL_SECONDS

    row = conn.execute(
        "SELECT name FROM projects WHERE path=? AND last_seen > ?",
        (cwd, cutoff),
    ).fetchone()
    if row:
        return row[0]

    return _walk_markers(cwd) or "ungrouped"


# ---------------------------------------------------------------------------
# Internal: marker walk
# ---------------------------------------------------------------------------


def _walk_markers(cwd: str) -> str:
    """Walk up <=20 directory levels checking project markers. Returns name or 'ungrouped'."""
    current = Path(cwd).resolve()
    for _ in range(_MAX_WALK_DEPTH):
        name = _check_markers(current)
        if name:
            return name
        parent = current.parent
        if parent == current:
            break
        current = parent
    return "ungrouped"


def _check_markers(directory: Path) -> str | None:
    """Check a single directory for project markers in priority order."""
    # 1. .git directory
    if (directory / ".git").is_dir():
        return directory.name

    # 2. package.json
    pkg = directory / "package.json"
    if pkg.exists():
        name = _parse_package_json(pkg)
        if name:
            return name

    # 3. pyproject.toml
    pyproj = directory / "pyproject.toml"
    if pyproj.exists():
        name = _parse_pyproject(pyproj)
        if name:
            return name

    # 4. Cargo.toml
    cargo = directory / "Cargo.toml"
    if cargo.exists():
        name = _parse_cargo(cargo)
        if name:
            return name

    # 5. go.mod
    gomod = directory / "go.mod"
    if gomod.exists():
        name = _parse_gomod(gomod)
        if name:
            return name

    # 6. docker-compose.yml / compose.yml
    for compose_name in ("docker-compose.yml", "compose.yml"):
        if (directory / compose_name).exists():
            return directory.name

    return None


# ---------------------------------------------------------------------------
# File parsers
# ---------------------------------------------------------------------------


def _parse_pyproject(p: Path) -> str | None:
    try:
        data = tomllib.loads(p.read_text(encoding="utf-8"))
        name = data.get("project", {}).get("name")
        if name:
            return name
        return data.get("tool", {}).get("poetry", {}).get("name")
    except Exception:
        return None


def _parse_package_json(p: Path) -> str | None:
    try:
        data = json.loads(p.read_text(encoding="utf-8"))
        return data.get("name") or None
    except Exception:
        return None


def _parse_cargo(p: Path) -> str | None:
    try:
        data = tomllib.loads(p.read_text(encoding="utf-8"))
        return data.get("package", {}).get("name") or None
    except Exception:
        return None


def _parse_gomod(p: Path) -> str | None:
    try:
        for line in p.read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if line.startswith("module "):
                module_path = line[len("module "):].strip()
                return module_path.rstrip("/").split("/")[-1] or None
        return None
    except Exception:
        return None
