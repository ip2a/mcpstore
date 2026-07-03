"""FastAPI integration helpers backed by the Rust MCPStore facade."""

from __future__ import annotations


def __getattr__(name: str):
    if name == "get_store":
        from .api_dependencies import get_store

        return get_store
    if name in {
        "api_agent_router",
        "api_main_router",
        "api_session_router",
        "api_set_store",
        "api_store_router",
    }:
        from . import api_pack

        return getattr(api_pack, name)
    raise AttributeError(f"module '{__name__}' has no attribute '{name}'")


__all__ = [
    "api_agent_router",
    "api_main_router",
    "api_session_router",
    "api_set_store",
    "api_store_router",
    "get_store",
]
