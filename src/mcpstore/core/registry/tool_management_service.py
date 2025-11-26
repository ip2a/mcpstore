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
        
        # Tool cache
        # agent_id -> {tool_name: tool_definition}
        self.tool_cache: Dict[str, Dict[str, Dict[str, Any]]] = {}
        
        # agent_id -> {tool_name: session}
        self.tool_to_session_map: Dict[str, Dict[str, Any]] = {}
        
        # agent_id -> {tool_name: service_name} (hard mapping)
        self.tool_to_service: Dict[str, Dict[str, str]] = {}
        
        # Snapshot data
        self._tools_snapshot_bundle: Optional[Dict[str, Any]] = None
        self._tools_snapshot_version: int = 0
        self._tools_snapshot_dirty: bool = True
    
    # === Tool List and Info Methods ===
    
    def list_tools(self, agent_id: str) -> List[Dict[str, Any]]:
        """Return a list-like snapshot of tools for the given agent_id.

        The registry stores raw tool definitions; this method converts them
        into a minimal, stable structure compatible with ToolInfo fields.
        We avoid importing pydantic models here to keep registry free of heavy deps.
        """
        tools_map = self.tool_cache.get(agent_id, {})
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
# === Snapshot building and publishing API ===
    def get_tools_snapshot_bundle(self) -> Optional[Dict[str, Any]]:
        """
        Return current published tools snapshot bundle (read-only pointer).
        Structure (example):
        {
            "tools": {
                "services": { "weather": [ToolItem, ...], ... },
                "tools_by_fullname": { "weather_get": ToolItem, ... }
            },
            "mappings": {
                "agent_to_global": { agent_id: { local: global } },
                "global_to_agent": { global: (agent_id, local) }
            },
            "meta": { "version": int, "created_at": float }
        }
        """
        bundle = self._tools_snapshot_bundle
        try:
            if bundle:
                meta = bundle.get("meta", {}) if isinstance(bundle, dict) else {}
                tools_section = bundle.get("tools", {}) if isinstance(bundle, dict) else {}
                services_index = tools_section.get("services", {}) if isinstance(tools_section, dict) else {}
                logger.debug(f"[SNAPSHOT] get_bundle ok (registry_id={id(self)}) version={meta.get('version')} services={len(services_index)}")
            else:
                logger.debug(f"[SNAPSHOT] get_bundle none (registry_id={id(self)})")
        except Exception as e:
            logger.debug(f"[SNAPSHOT] get_bundle log_error: {e}")
        return bundle
    def rebuild_tools_snapshot(self, global_agent_id: str) -> Dict[str, Any]:
            """
            Rebuild immutable tools snapshot bundle and publish using atomic pointer swap (Copy-On-Write).
            Build global source-of-truth snapshot based only on cache under global_agent_id;
            Agent views are projected by upper layer based on mappings.

            Note:
                This method is synchronous for backward compatibility, but internally uses
                async batch operations for better performance with py-key-value.

            Validates:
                - Requirements 3.3: Snapshot mechanism compatibility
                - Requirements 4.3: Snapshot API backward compatibility
            """
            import asyncio
            from time import time

            logger.debug(f"[SNAPSHOT] rebuild start (registry_id={id(self)}) agent={global_agent_id} current_version={self._tools_snapshot_version}")

            # Run async rebuild logic in sync context
            try:
                # Try to get the current event loop
                loop = asyncio.get_event_loop()
                if loop.is_running():
                    # If loop is already running, create a new task
                    # This handles the case where we're called from async context
                    import concurrent.futures
                    with concurrent.futures.ThreadPoolExecutor() as executor:
                        future = executor.submit(
                            asyncio.run,
                            self._rebuild_tools_snapshot_async(global_agent_id)
                        )
                        new_bundle = future.result()
                else:
                    # Loop exists but not running, use it
                    new_bundle = loop.run_until_complete(
                        self._rebuild_tools_snapshot_async(global_agent_id)
                    )
            except RuntimeError:
                # No event loop, create one
                new_bundle = asyncio.run(
                    self._rebuild_tools_snapshot_async(global_agent_id)
                )

            # Atomic publish (pointer swap)
            self._tools_snapshot_bundle = new_bundle
            self._tools_snapshot_version += 1

            try:
                services_index = new_bundle.get("tools", {}).get("services", {})
                total_tools = sum(len(v) for v in services_index.values())
            except Exception:
                total_tools = 0

            logger.debug(f"Tools bundle published: v{self._tools_snapshot_version}, services={len(services_index)}")
            logger.info(f"[SNAPSHOT] rebuild done (registry_id={id(self)}) version={self._tools_snapshot_version} services={len(services_index)} tools_total={total_tools}")

            # Clear dirty flag after rebuild completion
            self._tools_snapshot_dirty = False
            return new_bundle
    def mark_tools_snapshot_dirty(self) -> None:
            """Mark tools snapshot as dirty, indicating readers should rebuild on next access."""
            try:
                self._tools_snapshot_dirty = True
                logger.debug(f"[SNAPSHOT] marked dirty (registry_id={id(self)})")
            except Exception:
                # 防御性：不影响主流程
                pass
    def is_tools_snapshot_dirty(self) -> bool:
            """返回当前工具快照是否为脏。"""
            return bool(getattr(self, "_tools_snapshot_dirty", False))
    def tools_changed(self, global_agent_id: str, aggressive: bool = True) -> None:
            """统一触发器：声明工具/服务集合发生变化。

            当前阶段：直接标脏并立即重建，确保强一致；
            后续阶段（TODO）：可在此处加入去抖/限频的调度逻辑。
            """
            try:
                self.mark_tools_snapshot_dirty()
            except Exception:
                pass
            if aggressive:
                try:
                    self.rebuild_tools_snapshot(global_agent_id)
                except Exception:
                    # 防御性：不要影响上层流程
                    pass
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
            client_id = self._registry.get_service_client_id(agent_id, service_name) if service_name else None

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
            This method also updates the in-memory cache for backward compatibility.

        Raises:
            CacheOperationError: If cache operation fails
            CacheConnectionError: If cache connection fails
            CacheValidationError: If data validation fails
        """
        # Delegate to KV-backed state backend
        await self._state_backend.set_tool(agent_id, tool_name, tool_def)

        # Also update in-memory cache for backward compatibility
        if agent_id not in self.tool_cache:
            self.tool_cache[agent_id] = {}
        self.tool_cache[agent_id][tool_name] = tool_def

        # Mark snapshot as dirty
        self._tools_snapshot_dirty = True

        logger.debug(f"Set tool cache: agent={agent_id}, tool={tool_name}")
    
    @map_kv_exception
    async def get_tool_cache_async(self, agent_id: str) -> Dict[str, Any]:
        """
        Get all tool definitions for an agent from py-key-value storage.

        Args:
            agent_id: Agent ID

        Returns:
            Dictionary mapping tool names to tool definitions

        Note:
            This is the async version that reads from py-key-value.
            For backward compatibility, the sync version still uses in-memory cache.

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
            This method also updates the in-memory cache for backward compatibility.

        Raises:
            CacheOperationError: If cache operation fails
            CacheConnectionError: If cache connection fails
            CacheValidationError: If data validation fails
        """
        # Delegate to KV-backed state backend
        await self._state_backend.delete_tool(agent_id, tool_name)

        # Also update in-memory cache for backward compatibility
        if agent_id in self.tool_cache and tool_name in self.tool_cache[agent_id]:
            del self.tool_cache[agent_id][tool_name]

        # Mark snapshot as dirty
        self._tools_snapshot_dirty = True

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
    
    async def _rebuild_tools_snapshot_async(self, global_agent_id: str) -> Dict[str, Any]:
        """
        Internal async implementation of snapshot rebuild using batch operations.
        
        This method uses py-key-value's get_many for efficient batch retrieval of tool data.
        
        Args:
            global_agent_id: The global agent ID to build snapshot for
        
        Returns:
            The new snapshot bundle
        
        Note:
            Uses batch operations (get_many) for better performance with py-key-value.
            Maintains Copy-on-Write semantics by creating a new immutable bundle.
        """
        from time import time
        
        # Build global tools index
        services_index: Dict[str, List[Dict[str, Any]]] = {}
        tools_by_fullname: Dict[str, Dict[str, Any]] = {}
        
        # Iterate through all service names under global_agent_id
        service_names = self._registry.get_all_service_names(global_agent_id)
        
        # Use batch operations to get all tool data at once
        collection = self._registry._kv_adapter.get_collection(global_agent_id, "tools")
        
        # Get all tool definitions using batch operation
        all_tools_data = {}
        try:
            all_tools_data = await self._state_backend.list_tools(global_agent_id)
            logger.debug(f"[SNAPSHOT] Batch loaded {len(all_tools_data)} tools from backend")
        except Exception as e:
            logger.warning(f"[SNAPSHOT] Backend list_tools failed, falling back to in-memory: {e}")
            all_tools_data = self.tool_cache.get(global_agent_id, {})
            logger.debug(f"[SNAPSHOT] Fallback to in-memory cache, {len(all_tools_data)} tools")
        
        for service_name in service_names:
            # Get tool name list for this service
            tool_names = self._registry.get_tools_for_service(global_agent_id, service_name)
            if not tool_names:
                services_index[service_name] = []
                continue
            
            items: List[Dict[str, Any]] = []
            for tool_name in tool_names:
                # Use batch-loaded data instead of individual get_tool_info calls
                tool_def = all_tools_data.get(tool_name)
                if not tool_def:
                    # Fallback to get_tool_info if not in batch data
                    info = self._registry.get_tool_info(global_agent_id, tool_name)
                else:
                    # Extract info from tool definition
                    info = self._extract_tool_info_from_def(tool_def, tool_name, service_name, global_agent_id)
                
                if not info:
                    continue
                
                # Normalize to snapshot entry
                # Unified: Use prefixed full name for stable external keys (info.name / tool_name)
                # Display: display_name provided as pure name to frontend
                full_name = info.get("name", tool_name)
                item = {
                    "name": full_name,
                    "display_name": info.get("display_name", info.get("original_name", full_name.split(f"{service_name}_", 1)[-1] if isinstance(full_name, str) else full_name)),
                    "description": info.get("description", ""),
                    "service_name": service_name,
                    "client_id": info.get("client_id"),
                    "inputSchema": info.get("inputSchema", {}),
                    "original_name": info.get("original_name", info.get("name", tool_name))
                }
                items.append(item)
                tools_by_fullname[full_name] = item
            services_index[service_name] = items
        
        # Copy mapping snapshot (read-only)
        agent_to_global = {aid: dict(mapping) for aid, mapping in self._registry.agent_to_global_mappings.items()}
        global_to_agent = dict(self._registry.global_to_agent_mappings)
        
        new_bundle: Dict[str, Any] = {
            "tools": {
                "services": services_index,
                "tools_by_fullname": tools_by_fullname
            },
            "mappings": {
                "agent_to_global": agent_to_global,
                "global_to_agent": global_to_agent
            },
            "meta": {
                "version": self._tools_snapshot_version + 1,
                "created_at": time()
            }
        }
        
        return new_bundle
