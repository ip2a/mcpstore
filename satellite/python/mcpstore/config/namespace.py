"""Cache namespace helpers."""

from __future__ import annotations

from typing import Any


def get_namespace(config: Any, default: str = "mcpstore") -> str:
    return getattr(config, "namespace", None) or default
