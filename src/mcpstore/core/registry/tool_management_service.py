"""
Tool Management Service for ServiceRegistry

Manages tool definitions, snapshots, and tool-to-service mappings.
Extracted from core_registry.py to reduce God Object complexity.
"""

import logging
import asyncio
from time import time
from typing import Dict, Any, Optional, List, TYPE_CHECKING

from .exception_mapper import map_kv_exception

if TYPE_CHECKING:
    from key_value.aio.protocols import AsyncKeyValue
    from .state_backend import RegistryStateBackend

logger = logging.getLogger(__name__)


class ToolManagementService:
    """
    Manages tool definitions, snapshots, and mappings.
    
    Responsibilities:
    - Tool cache management
    - Tool-to-service mappings
    - Tool snapshot building and publishing
    - Batch operations for tool data
    """
    
    def __init__(self, kv_store: 'AsyncKeyValue', state_backend: 'RegistryStateBackend', 
                 kv_adapter, registry):
        """
        Initialize Tool Management service.
        
        Args:
            kv_store: AsyncKeyValue instance for data storage
            state_backend: Registry state backend for KV operations
            kv_adapter: KV storage adapter for sync operations
            registry: Parent ServiceRegistry instance (for accessing other services)
        """
        self._kv_store = kv_store
        self._state_backend = state_backend
        self._kv_adapter = kv_adapter
        self._registry = registry  # Need parent for get_all_service_names, etc.

        # agent_id -> {tool_name: session}
        self.tool_to_session_map: Dict[str, Dict[str, Any]] = {}

        # agent_id -> {tool_name: service_name} (hard mapping)
        self.tool_to_service: Dict[str, Dict[str, str]] = {}
    
    # === Tool List and Info Methods ===

    def list_tools(self, agent_id: str) -> List[Dict[str, Any]]:
        """
        同步外壳：获取工具列表

        严格按照Functional Core, Imperative Shell原则：
        - 只在最外层进行一次同步转异步
        - 完全避免sync_to_kv使用
        """
        import asyncio

        async def _execute():
            return await self._state_backend.list_tools(agent_id)

        try:
            # 只在这里进行一次同步转异步
            tools_map = asyncio.run(_execute())
        except Exception as e:
            logger.error(f"[TOOLS_LIST] 获取工具列表失败: {e}")
            return []

        result: List[Dict[str, Any]] = []
        for tool_name, tool_def in tools_map.items():
            try:
                if isinstance(tool_def, dict) and "function" in tool_def:
                    fn = tool_def["function"]
                    result.append({
                        "name": fn.get("name", tool_name),
                        "description": fn.get("description", ""),
                        "service_name": fn.get("service_name", ""),
                        "client_id": None,
                        "inputSchema": fn.get("parameters")
                    })
                else:
                    # Fallback best-effort mapping
                    result.append({
                        "name": tool_name,
                        "description": str(tool_def.get("description", "")) if isinstance(tool_def, dict) else "",
                        "service_name": tool_def.get("service_name", "") if isinstance(tool_def, dict) else "",
                        "client_id": None,
                        "inputSchema": tool_def.get("parameters") if isinstance(tool_def, dict) else None
                    })
            except Exception as e:
                logger.warning(f"[REGISTRY] Failed to map tool '{tool_name}': {e}")
        return result

    # === Tool Cache Operations (KV-backed) ===

    def _extract_tool_info_from_def(self, tool_def: Dict[str, Any], tool_name: str,
                                        service_name: str, agent_id: str) -> Dict[str, Any]:
            """
            Extract tool info from tool definition (helper for batch operations).

            Args:
                tool_def: Tool definition dict
                tool_name: Tool name
                service_name: Service name
                agent_id: Agent ID

            Returns:
                Tool info dict compatible with get_tool_info format
            """
            # Get Client ID
            client_id = self._registry._agent_client_service.get_service_client_id(agent_id, service_name) if service_name else None

            # Handle different tool definition formats
            if "function" in tool_def:
                function_data = tool_def["function"]
                return {
                    'name': tool_name,
                    'display_name': function_data.get('display_name', tool_name),
                    'original_name': function_data.get('name', tool_name),
                    'description': function_data.get('description', ''),
                    'inputSchema': function_data.get('parameters', {}),
                    'service_name': service_name,
                    'client_id': client_id
                }
            else:
                return {
                    'name': tool_name,
                    'display_name': tool_def.get('display_name', tool_name),
                    'original_name': tool_def.get('name', tool_name),
                    'description': tool_def.get('description', ''),
                    'inputSchema': tool_def.get('parameters', {}),
                    'service_name': service_name,
                    'client_id': client_id
                }

    @map_kv_exception
    async def set_tool_cache_async(self, agent_id: str, tool_name: str, tool_def: Dict[str, Any]) -> None:
        """
        Set a tool definition in py-key-value storage.

        Args:
            agent_id: Agent ID
            tool_name: Tool name (key)
            tool_def: Tool definition (value)

        Note:
            Writes directly to pyvk (single source of truth).

        Raises:
            CacheOperationError: If cache operation fails
            CacheConnectionError: If cache connection fails
            CacheValidationError: If data validation fails
        """
        # Write directly to pyvk
        await self._state_backend.set_tool(agent_id, tool_name, tool_def)
        logger.debug(f"Set tool: agent={agent_id}, tool={tool_name}")
    
    @map_kv_exception
    async def get_tool_cache_async(self, agent_id: str) -> Dict[str, Any]:
        """
        Get all tool definitions for an agent from py-key-value storage.

        Args:
            agent_id: Agent ID

        Returns:
            Dictionary mapping tool names to tool definitions

        Note:
            这是从 py-key-value 读取的异步版本。
            同步版本仍使用内存缓存（过渡期间保留）。

        Raises:
            CacheOperationError: If cache operation fails
            CacheConnectionError: If cache connection fails
            CacheValidationError: If data validation fails
        """
        # Delegate to KV-backed state backend
        tools = await self._state_backend.list_tools(agent_id)
        return tools
    
    @map_kv_exception
    async def delete_tool_cache_async(self, agent_id: str, tool_name: str) -> None:
        """
        Delete a tool definition from py-key-value storage.

        Args:
            agent_id: Agent ID
            tool_name: Tool name to delete

        Note:
            Deletes directly from pyvk (single source of truth).

        Raises:
            CacheOperationError: If cache operation fails
            CacheConnectionError: If cache connection fails
            CacheValidationError: If data validation fails
        """
        # Delete directly from pyvk
        await self._state_backend.delete_tool(agent_id, tool_name)

        logger.debug(f"Deleted tool cache: agent={agent_id}, tool={tool_name}")

    # === Async Service State Access Methods ===
    
    @map_kv_exception
    async def set_tool_to_service_mapping_async(self, agent_id: str, tool_name: str, service_name: str) -> None:
        """
        Set the tool-to-service mapping in py-key-value storage.

        Args:
            agent_id: Agent ID
            tool_name: Tool name
            service_name: Service name to map to

        Note:
            This method wraps the service_name in a dictionary before storage
            to satisfy py-key-value's type requirements (dict[str, Any]).
            This prevents beartype warnings and ensures type safety.
            The in-memory cache is also updated for backward compatibility.

        Raises:
            CacheOperationError: If cache operation fails
            CacheConnectionError: If cache connection fails
            CacheValidationError: If data validation fails
        """
        # Delegate to KV-backed state backend
        await self._state_backend.set_tool_service(agent_id, tool_name, service_name)

        # Also update in-memory cache for backward compatibility
        if agent_id not in self.tool_to_service:
            self.tool_to_service[agent_id] = {}
        self.tool_to_service[agent_id][tool_name] = service_name

        logger.debug(f"Set tool mapping: agent={agent_id}, tool={tool_name} -> service={service_name}")
    
    @map_kv_exception
    async def get_tool_to_service_mapping_async(self, agent_id: str, tool_name: str) -> Optional[str]:
        """
        Get the service name mapped to a tool from py-key-value storage.

        Args:
            agent_id: Agent ID
            tool_name: Tool name

        Returns:
            Service name or None if not found

        Note:
            This method unwraps the service_name from the dictionary format,
            maintaining backward compatibility with legacy unwrapped data.
            The async version reads from py-key-value, while the sync version
            still uses in-memory cache for backward compatibility.

        Raises:
            CacheOperationError: If cache operation fails
            CacheConnectionError: If cache connection fails
            CacheValidationError: If data validation fails
        """
        # Delegate to KV-backed state backend
        return await self._state_backend.get_tool_service(agent_id, tool_name)
    
    @map_kv_exception
    async def delete_tool_to_service_mapping_async(self, agent_id: str, tool_name: str) -> None:
        # Delegate to KV-backed state backend
        await self._state_backend.delete_tool_service(agent_id, tool_name)
