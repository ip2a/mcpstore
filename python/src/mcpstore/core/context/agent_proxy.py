"""Rust-backed compatibility wrapper for the historical AgentProxy."""

from typing import Any

from mcpstore.core.store.rust_backend import RustStoreContext


class AgentProxy:
    """Object-style agent handle that delegates to an agent scoped context."""

    def __init__(self, context: RustStoreContext, agent_id: str):
        self._agent_id = agent_id
        if getattr(context, "agent_id", None) == agent_id:
            self._context = context
        else:
            self._context = context.find_agent(agent_id)

    def get_context(self) -> RustStoreContext:
        return self._context

    def get_id(self) -> str:
        return self._context.get_id()

    def __getattr__(self, name: str) -> Any:
        return getattr(self._context, name)
