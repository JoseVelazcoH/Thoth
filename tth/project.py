"""Project inference: walk directory markers, 24-hour cache."""

import json
import sqlite3
import time
import tomllib
from collections.abc import Callable
from pathlib import Path

_CACHE_TTL_SECONDS = 86400  # 24 hours
_MAX_WALK_DEPTH = 20


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



def _read_nested(data: dict, *keys: str) -> str | None:
    """Walk a chain of dict keys; return the string value or None."""
    node = data
    for key in keys:
        if not isinstance(node, dict):
            return None
        node = node.get(key)
    return node if isinstance(node, str) and node else None


def _load_toml(p: Path) -> dict | None:
    """Parse a TOML file; return the data dict or None on any error."""
    try:
        return tomllib.loads(p.read_text(encoding="utf-8"))
    except Exception:
        return None


def _load_json(p: Path) -> dict | None:
    """Parse a JSON file; return the data dict or None on any error."""
    try:
        data = json.loads(p.read_text(encoding="utf-8"))
        return data if isinstance(data, dict) else None
    except Exception:
        return None


def _extract_pyproject(p: Path) -> str | None:
    data = _load_toml(p)
    if data is None:
        return None
    return _read_nested(data, "project", "name") or _read_nested(data, "tool", "poetry", "name")


def _extract_package_json(p: Path) -> str | None:
    data = _load_json(p)
    return _read_nested(data, "name") if data is not None else None


def _extract_cargo(p: Path) -> str | None:
    data = _load_toml(p)
    return _read_nested(data, "package", "name") if data is not None else None


def _extract_gomod(p: Path) -> str | None:
    try:
        for line in p.read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if line.startswith("module "):
                module_path = line[len("module "):].strip()
                return module_path.rstrip("/").split("/")[-1] or None
        return None
    except Exception:
        return None


# Each entry is (filename, extractor). Directory-name markers use is_dir() check below.
_MARKER_STRATEGIES: list[tuple[str, Callable[[Path], str | None]]] = [
    (".git", lambda p: p.parent.name),          # directory marker: use is_dir() check below
    ("package.json", _extract_package_json),
    ("pyproject.toml", _extract_pyproject),
    ("Cargo.toml", _extract_cargo),
    ("go.mod", _extract_gomod),
    ("docker-compose.yml", lambda p: p.parent.name),
    ("compose.yml", lambda p: p.parent.name),
]

# Filenames that must exist as directories, not regular files.
_DIR_MARKERS = {".git"}



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
    for filename, extractor in _MARKER_STRATEGIES:
        candidate = directory / filename
        if filename in _DIR_MARKERS:
            if candidate.is_dir():
                return extractor(candidate)
        else:
            if candidate.exists():
                name = extractor(candidate)
                if name:
                    return name
    return None
