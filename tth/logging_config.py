import logging
import logging.handlers
from pathlib import Path

_DEFAULT_MAX_BYTES = 1_000_000
_DEFAULT_BACKUP_COUNT = 2


def setup(log_path: Path, max_bytes: int = _DEFAULT_MAX_BYTES, backup_count: int = _DEFAULT_BACKUP_COUNT) -> None:
    log_path = Path(log_path)
    log_path.parent.mkdir(parents=True, exist_ok=True)

    root = logging.getLogger("tth")
    root.handlers.clear()
    root.setLevel(logging.WARNING)

    handler = logging.handlers.RotatingFileHandler(
        str(log_path),
        maxBytes=max_bytes,
        backupCount=backup_count,
        encoding="utf-8",
    )
    handler.setFormatter(logging.Formatter("%(asctime)s %(name)s %(levelname)s %(message)s"))
    root.addHandler(handler)
    root.propagate = False
