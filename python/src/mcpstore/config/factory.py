"""Removed Python cache factory entry point.

The SDK no longer creates Python-side cache stores. Cache inspection and
health checks must go through the Rust-backed store facade.
"""

from __future__ import annotations

from typing import Any


def create_kv_store(cache_config: Any):
    raise RuntimeError(
        "create_kv_store() was removed with the Python core cache runtime. "
        "Use MCPStore.setup_store(cache=...).for_store().find_cache() so cache "
        "operations go through the Rust/PyO3 backend."
    )
