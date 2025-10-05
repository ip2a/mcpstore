"""
Repository-style thin facade for cache operations.

Goals:
- Provide a small, cohesive API that wraps CacheBackend writes in atomic transactions
- Offer methods suitable for multi-key write sequences (e.g., registering service tools)
- Keep domain logic in ServiceRegistry, but enable reuse in orchestrators or tests

This repository expects `registry` to be a ServiceRegistry-like object that exposes
`cache_backend` for storage operations.
"""
from __future__ import annotations

from typing import Dict, Iterable, Optional, Any

from .atomic import atomic_write


class CacheRepository:
    """Thin facade over the cache backend with atomic write helpers.

    Typical usage:
        repo = CacheRepository(registry)
        await repo.apply_service_snapshot(agent_id, service_name, client_id, tools_dict)
    """

    def __init__(self, registry: Any) -> None:
        self.registry = registry
        # Provide direct field so @atomic_write can resolve backend quickly
        self.cache_backend = getattr(registry, "cache_backend")

    # ----------------------- Bulk / Composite operations -----------------------

    @atomic_write(agent_id_param="agent_id", use_lock=True)
    async def apply_service_snapshot(
        self,
        agent_id: str,
        service_name: str,
        client_id: str,
        tools: Dict[str, Dict[str, Any]],
    ) -> None:
        """Apply a full set of tool mappings and definitions for one service.

        - Maps each tool to the service
        - Upserts each tool's definition (normalized by backend)
        - Ensures agent-client and service-client relationships
        """
        be = self.cache_backend
        for tool_name, tool_def in tools.items():
            be.map_tool_to_service(agent_id, tool_name, service_name)
            be.upsert_tool_def(agent_id, tool_name, tool_def)
        be.add_agent_client_mapping(agent_id, client_id)
        be.add_service_client_mapping(agent_id, service_name, client_id)

    @atomic_write(agent_id_param="agent_id", use_lock=True)
    async def clear_service_tools(self, agent_id: str, service_name: str, tool_names: Iterable[str]) -> None:
        """Remove tool defs and tool→service mappings for given names.
        Service→client mapping is not modified here.
        """
        be = self.cache_backend
        for tool_name in tool_names:
            be.delete_tool_def(agent_id, tool_name)
            # remove tool→service mapping if backend supports it (optional semantics)
            # For simplicity we can re-map whole hash by deleting specific field when available
            try:
                be.unmap_tool_from_service(agent_id, tool_name)  # type: ignore[attr-defined]
            except Exception:
                # Optional method; ignore if not provided by backend
                pass

    # ---------------------------- Small granular ops ---------------------------

    @atomic_write(agent_id_param="agent_id", use_lock=True)
    async def map_service_client(self, agent_id: str, service_name: str, client_id: str) -> None:
        self.cache_backend.add_service_client_mapping(agent_id, service_name, client_id)

    @atomic_write(agent_id_param="agent_id", use_lock=True)
    async def add_agent_client(self, agent_id: str, client_id: str) -> None:
        self.cache_backend.add_agent_client_mapping(agent_id, client_id)

    @atomic_write(agent_id_param="agent_id", use_lock=True)
    async def upsert_tool(self, agent_id: str, service_name: str, tool_name: str, tool_def: Dict[str, Any]) -> None:
        self.cache_backend.map_tool_to_service(agent_id, tool_name, service_name)
        self.cache_backend.upsert_tool_def(agent_id, tool_name, tool_def)

    # ------------------------------ Read-throughs ------------------------------

    def list_tool_names(self, agent_id: str):
        return self.cache_backend.list_tool_names(agent_id)

    def get_tool_def(self, agent_id: str, tool_name: str):
        return self.cache_backend.get_tool_def(agent_id, tool_name)

    def get_service_client_id(self, agent_id: str, service_name: str) -> Optional[str]:
        return self.cache_backend.get_service_client_id(agent_id, service_name)

    def get_agent_clients(self, agent_id: str):
        return self.cache_backend.get_agent_clients_from_cache(agent_id)

    def get_client_config(self, client_id: str):
        return self.cache_backend.get_client_config_from_cache(client_id)

