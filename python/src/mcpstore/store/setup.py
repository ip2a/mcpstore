"""Python setup facade: resolve ``source`` + ``mode`` and hand off to Rust."""

from __future__ import annotations

import importlib
import json
import os
from typing import Any, Dict, Optional


_VALID_MODES = {"control_plane", "data_plane"}


# ---------------------------------------------------------------------------
# Resolution helpers
# ---------------------------------------------------------------------------

def _config_triplet(source: Any) -> tuple[Optional[str], Optional[dict[str, Any]], Optional[str]]:
    """Extract (store_name, config_dict, namespace) from a source config object."""
    if source is None:
        return None, None, None
    store = getattr(source, "store_name", None)
    config = getattr(source, "config", None) or {}
    namespace = getattr(source, "namespace", None)
    return store, config, namespace


def _resolve_source_mode(source: Any) -> str:
    """Map a source config object to Rust source_mode.

    FileConfig / MemoryConfig (or anything whose store_name is 'file'/'local'/'memory')
    -> "local".  Any other Store (Redis, Sqlite, ...) -> "db".
    """
    if source is None:
        return "local"
    store_name = getattr(source, "store_name", None)
    if store_name is None and isinstance(source, dict):
        store_name = str(source.get("store", "")).strip().lower()
    store_name = (store_name or "").strip().lower()
    if store_name in {"file", "local", "memory"}:
        return "local"
    return "db"


def _resolve_node_mode(mode: Optional[str], source_mode: str) -> str:
    """Resolve user-facing mode to a concrete node mode.

    Default: file/local source -> control_plane, remote store -> data_plane.
    """
    if mode is not None:
        resolved = mode.strip().lower()
        if resolved not in _VALID_MODES:
            raise ValueError(
                f"mode must be one of {_VALID_MODES}, got: {mode!r}"
            )
        return resolved
    return "data_plane" if source_mode == "db" else "control_plane"


def _extract_file_path(source: Any) -> Optional[str]:
    """If source is a FileConfig carrying a path, return it; else None."""
    path = getattr(source, "path", None)
    if path is None and isinstance(source, dict):
        path = source.get("path")
    return os.fspath(path) if path is not None else None


# ---------------------------------------------------------------------------
# Backend construction
# ---------------------------------------------------------------------------

def setup_backend(
    backend_cls: type,
    source: Any,
    source_mode: str,
    node_mode: str,
):
    """Build the Rust-backed store.

    ``source_mode`` selects where service definitions are read from
    (``local`` vs ``db``); ``node_mode`` selects the node role
    (``control_plane`` maintains clients/supervisor and runs writes
    directly; ``data_plane`` queues writes and skips local connection
    state). Both are passed through to the Rust core verbatim.
    """
    rust_mod = importlib.import_module("mcpstore._rust")
    file_path = _extract_file_path(source)
    store_name, store_config, namespace = _config_triplet(source)
    rust_store = rust_mod.MCPStore.setup_with_options(
        file_path,
        source_mode,
        store_name,
        json.dumps(store_config or {}, separators=(",", ":")),
        namespace,
        node_mode,
    )
    store = backend_cls(rust_store)
    store._source_mode = source_mode
    store._node_mode = node_mode
    store.load_from_config()
    return store


class StoreSetupManager:
    """Setup manager for the single Rust-backed MCPStore entry point."""

    @staticmethod
    def setup_store(
        source: Any,
        mode: Optional[str] = None,
        *,
        debug: bool | str = False,
        static_config: Optional[Dict[str, Any]] = None,
        **kwargs: Any,
    ):
        """Initialize MCPStore synchronously with the Rust core.

        Parameters
        ----------
        source:
            The data source config.  Its type decides where definitions come
            from: ``FileConfig`` -> local file, ``RedisConfig`` -> remote DB,
            ``MemoryConfig`` -> in-process, etc.
        mode:
            Node role: ``"control_plane"`` (default for local sources) maintains
            clients/supervisor and executes writes directly; ``"data_plane"``
            (default for remote stores) does not maintain clients and queues
            writes for a control_plane node to consume.
        """
        from mcpstore.config.config import LoggingConfig

        LoggingConfig.setup_logging(debug=debug)

        if kwargs:
            unsupported = ", ".join(sorted(kwargs))
            raise ValueError(f"setup_store 不支持参数: {unsupported}")

        source_mode = _resolve_source_mode(source)
        node_mode = _resolve_node_mode(mode, source_mode)

        from mcpstore.store.store import MCPStore as PyMCPStore
        store = PyMCPStore.setup(source=source, source_mode=source_mode, node_mode=node_mode)

        if static_config:
            StoreSetupManager._add_static_config(store, static_config)
        return store

    @staticmethod
    def _add_static_config(store: Any, static_config: Dict[str, Any]) -> None:
        services = static_config.get("mcpServers")
        if not isinstance(services, dict):
            raise ValueError("static_config must contain an 'mcpServers' object")
        for service_name, config in services.items():
            if not isinstance(service_name, str) or not service_name:
                raise ValueError("static_config service names must be non-empty strings")
            if not isinstance(config, dict):
                raise ValueError(
                    f"static_config service {service_name!r} must be an object"
                )
            store.add_service(service_name, config)
