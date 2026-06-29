"""Rust-backed compatibility wrapper for the historical StoreProxy."""

from typing import Any

from mcpstore.core.store.rust_backend import RustStoreContext


class StoreProxy:
    """Object-style store handle that delegates to ``RustStoreContext``.

    The historical Python implementation carried registry orchestration here.
    In the Rust-backed SDK, the context is the source of behavior; this wrapper
    only preserves the old import path and constructor shape.
    """

    def __init__(self, context: RustStoreContext):
        self._context = context

    def get_context(self) -> RustStoreContext:
        return self._context

    def get_id(self) -> str:
        return self._context.get_id()

    def find_agent(self, agent_id: str) -> "AgentProxy":
        from .agent_proxy import AgentProxy

        return AgentProxy(self._context, agent_id)

    def __getattr__(self, name: str) -> Any:
        return getattr(self._context, name)
