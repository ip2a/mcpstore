"""
ContextKernel abstraction (internal).

Phase 1: minimal scaffold used by MCPStoreContext for read paths only (services/tools).
No external API changes; callers still use MCPStoreContext methods.
"""

from __future__ import annotations

from typing import Any

from ..types import ContextType


class ContextKernel:
    """Kernel interface for context-specific operations."""

    def list_services(self) -> Any:  # returns List[ServiceInfo] or compatible
        raise NotImplementedError

    def list_tools(self) -> Any:  # returns List[ToolInfo] or compatible
        raise NotImplementedError


class StoreContextKernel(ContextKernel):
    def __init__(self, ctx: 'MCPStoreContext') -> None:
        self.ctx = ctx

    def list_services(self) -> Any:
        # Delegate to store layer directly
        return self.ctx._run_async_via_bridge(
            self.ctx._store.list_services(),
            op_name="context_kernel.store.list_services"
        )

    def list_tools(self) -> Any:
        # Prefer orchestrator snapshot
        return self.ctx.list_tools()


class AgentContextKernel(ContextKernel):
    def __init__(self, ctx: 'MCPStoreContext') -> None:
        self.ctx = ctx

    def list_services(self) -> Any:
        # Keep existing agent-view logic
        return self.ctx._run_async_via_bridge(
            self.ctx._get_agent_service_view(),
            op_name="context_kernel.agent.list_services"
        )

    def list_tools(self) -> Any:
        # Keep existing agent-view logic
        return self.ctx._run_async_via_bridge(
            self.ctx._get_agent_tools_view(),
            op_name="context_kernel.agent.list_tools"
        )


def create_kernel(ctx: 'MCPStoreContext') -> ContextKernel:
    return StoreContextKernel(ctx) if ctx.context_type == ContextType.STORE else AgentContextKernel(ctx)

