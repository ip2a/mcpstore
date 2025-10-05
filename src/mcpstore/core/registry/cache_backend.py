from __future__ import annotations

from typing import Protocol, Dict, Any, Optional, List


class CacheBackend(Protocol):
    """Abstract cache backend interface for ServiceRegistry state.

    This isolates the storage layer (in-memory, Redis, etc.) from the registry's
    public API. All operations are partitioned by agent_id.
    """

    # ---- Client/Service mappings ----
    def add_agent_client_mapping(self, agent_id: str, client_id: str) -> None:
        ...

    def remove_agent_client_mapping(self, agent_id: str, client_id: str) -> None:
        ...

    def get_agent_clients_from_cache(self, agent_id: str) -> List[str]:
        ...

    def add_client_config(self, client_id: str, config: Dict[str, Any]) -> None:
        ...

    def update_client_config(self, client_id: str, updates: Dict[str, Any]) -> None:
        ...

    def get_client_config_from_cache(self, client_id: str) -> Optional[Dict[str, Any]]:
        ...

    def remove_client_config(self, client_id: str) -> None:
        ...

    def add_service_client_mapping(self, agent_id: str, service_name: str, client_id: str) -> None:
        ...

    def get_service_client_id(self, agent_id: str, service_name: str) -> Optional[str]:
        ...

    def remove_service_client_mapping(self, agent_id: str, service_name: str) -> None:
        ...

    # ---- Tools (mapping + optional definitions) ----
    def map_tool_to_service(self, agent_id: str, tool_name: str, service_name: str) -> None:
        ...

    def unmap_tool(self, agent_id: str, tool_name: str) -> None:
        ...

    # Optional: store normalized tool definitions (for full cache mode)
    def upsert_tool_def(self, agent_id: str, tool_name: str, tool_def: Dict[str, Any]) -> None:
        ...

    def delete_tool_def(self, agent_id: str, tool_name: str) -> None:
        ...

    def get_tool_def(self, agent_id: str, tool_name: str) -> Optional[Dict[str, Any]]:
        ...

    def list_tool_names(self, agent_id: str) -> List[str]:
        ...

    # ---- Optional: service/session state (kept minimal for M2) ----
    def set_session(self, agent_id: str, service_name: str, session: Any) -> None:
        ...

    def get_session(self, agent_id: str, service_name: str) -> Optional[Any]:
        ...

    # ---- Bulk maintenance helpers ----
    def clear_agent(self, agent_id: str) -> None:
        ...

    # ---- Optional transaction & health (M3) ----
    def begin(self) -> None:
        """Start a backend transaction if supported; Memory is no-op."""
        ...

    def commit(self) -> None:
        """Commit a backend transaction if supported; Memory is no-op."""
        ...

    def rollback(self) -> None:
        """Rollback a backend transaction if supported; Memory is no-op."""
        ...

    def health_check(self) -> bool:
        """Return True if backend is healthy/available."""
        ...

