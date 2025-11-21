"""
Atomic config write service with cross-platform advisory locking.

Non-breaking introduction: this module is added but not yet integrated. It
provides a single entrypoint `atomic_update` that callers can adopt to avoid
read-modify-write races when updating JSON configs like mcp.json.
"""

from __future__ import annotations

import json
import os
import tempfile
from contextlib import contextmanager
from pathlib import Path
from typing import Callable, Dict, Any


class ConfigWriteService:
    """Utility for atomic JSON config updates with file locking."""

    def __init__(self, lock_suffix: str = ".lock"):
        self._lock_suffix = lock_suffix

    def atomic_update(self, json_path: str, mutator: Callable[[Dict[str, Any]], Dict[str, Any]]) -> bool:
        """Atomically update a JSON file with a user-provided mutator.

        Steps:
        - Acquire advisory lock file
        - Read current JSON (or {} if missing)
        - Apply mutator(config) -> new_config
        - Write to temp file and atomically replace

        Returns:
        - bool: True on success
        """
        path = Path(json_path)
        path.parent.mkdir(parents=True, exist_ok=True)

        with self._lock_file(path):
            current: Dict[str, Any] = {}
            if path.exists():
                try:
                    with path.open("r", encoding="utf-8") as f:
                        current = json.load(f)
                except Exception:
                    # Corrupt or empty, treat as empty structure
                    current = {}

            new_config = mutator(dict(current)) or {}

            # Serialize with stable formatting
            data = json.dumps(new_config, ensure_ascii=False, indent=2)

            # Write to temp file in same directory for atomic replace
            fd, tmp = tempfile.mkstemp(prefix=path.name + ".", dir=str(path.parent))
            try:
                with os.fdopen(fd, "w", encoding="utf-8") as f:
                    f.write(data)
                    f.flush()
                    os.fsync(f.fileno())
                # Atomic replace on POSIX; on Windows, replace should also be atomic for same-volume
                os.replace(tmp, path)
                return True
            finally:
                # If replace failed, ensure temp is removed
                if os.path.exists(tmp):
                    try:
                        os.remove(tmp)
                    except Exception:
                        pass

    @contextmanager
    def _lock_file(self, target: Path):
        """Advisory lock via lock file creation; best-effort cross-platform.

        This is intentionally simple: exclusive create, retry quickly.
        For higher contention, consider portalocker; we avoid new deps here.
        """
        lock_path = target.with_suffix(target.suffix + self._lock_suffix)
        # Busy-wait a few short tries to avoid long stalls
        import time
        delay_s = 0.02
        for _ in range(250):  # ~5 seconds max
            try:
                # Exclusive creation
                fd = os.open(str(lock_path), os.O_CREAT | os.O_EXCL | os.O_WRONLY)
                try:
                    with os.fdopen(fd, "w") as f:
                        f.write(str(os.getpid()))
                    break
                except Exception:
                    os.close(fd)
                    raise
            except FileExistsError:
                time.sleep(delay_s)
        else:
            # Last resort: proceed without lock to avoid deadlock
            fd = None

        try:
            yield
        finally:
            if lock_path.exists():
                try:
                    os.remove(lock_path)
                except Exception:
                    pass


