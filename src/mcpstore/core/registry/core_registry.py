import os
import sys

sys.path.append(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
import logging
from datetime import datetime
from typing import Dict, Any, Optional, Tuple, List, Set, TYPE_CHECKING

from ..models.service import ServiceConnectionState, ServiceStateMetadata

from .atomic import atomic_write
from .exception_mapper import map_kv_exception
from .state_backend import KVRegistryStateBackend, RegistryStateBackend
from .kv_storage_adapter import KVStorageAdapter
from .agent_client_mapping_service import AgentClientMappingService
from .client_config_service import ClientConfigService
from .service_state_service import ServiceStateService
from .tool_management_service import ToolManagementService

if TYPE_CHECKING:
    from key_value.aio.protocols import AsyncKeyValue

logger = logging.getLogger(__name__)


class ServiceRegistry:
    """
    Manages the state of connected services and their tools, with agent_id isolation.

    agent_id as primary key, implementing complete isolation between store/agent/agent:
    - self.sessions: Dict[agent_id, Dict[service_name, session]] (in-memory only)
    - Other data stored via py-key-value AsyncKeyValue interface
    
    All operations must include agent_id, store level uses global_agent_store, agent level uses actual agent_id.
    """

    def __init__(self,
                 kv_store: Optional['AsyncKeyValue'] = None,
                 service_state_service: Optional[ServiceStateService] = None,
                 agent_client_service: Optional[AgentClientMappingService] = None,
                 client_config_service: Optional[ClientConfigService] = None,
                 tool_management_service: Optional[ToolManagementService] = None):
        """
        Initialize ServiceRegistry with dependency injection support.

        Args:
            kv_store: AsyncKeyValue instance for data storage. If None, uses MemoryStore.
                     Session data is always kept in memory regardless of kv_store type.
            service_state_service: Optional pre-initialized ServiceStateService (dependency injection)
            agent_client_service: Optional pre-initialized AgentClientMappingService (dependency injection)
            client_config_service: Optional pre-initialized ClientConfigService (dependency injection)
            tool_management_service: Optional pre-initialized ToolManagementService (dependency injection)

        Note:
            - When all services are provided: zero delegation factory mode
            - When no services provided: legacy backward compatibility mode
            - Sessions are stored in memory (not serializable)
            - All other data (tools, states, metadata, mappings) use kv_store
            - Maintains backward compatibility with existing code
        """
        # Import py-key-value here to avoid circular imports
        if kv_store is None:
            try:
                from key_value.aio.stores.memory import MemoryStore
                kv_store = MemoryStore()
                logger.debug("ServiceRegistry initialized with default MemoryStore")
            except ImportError:
                raise RuntimeError(
                    "py-key-value is not installed. Please install it with: "
                    "pip install py-key-value"
                )

        # Store the py-key-value instance
        self._kv_store: 'AsyncKeyValue' = kv_store
        # Initialize KV-backed state backend
        self._state_backend: RegistryStateBackend = KVRegistryStateBackend(self._kv_store)

        # Initialize KV storage adapter (extracted from God Object refactoring)
        self._kv_adapter = KVStorageAdapter(self._kv_store)

        # Initialize AsyncSyncHelper for sync-to-async conversion
        self._sync_helper: Optional[Any] = None  # Lazy initialization

        # Factory mode vs Legacy mode detection
        factory_mode = all([
            service_state_service is not None,
            agent_client_service is not None,
            client_config_service is not None
        ])

        if factory_mode:
            # Zero delegation factory mode - inject all dependencies
            logger.debug("ServiceRegistry initializing in factory mode (zero delegation)")
            self._service_state_service = service_state_service
            self._agent_client_service = agent_client_service
            self._client_config_service = client_config_service
            # ToolManagementService needs registry reference, create it after registry initialization
            self._tool_management_service = tool_management_service
        else:
            # Legacy backward compatibility mode - create services internally
            logger.debug("ServiceRegistry initializing in legacy mode (self-created services)")
            # Initialize service modules (God Object refactoring)
            self._agent_client_service = AgentClientMappingService(self._kv_store, self._state_backend, self._kv_adapter)
            self._client_config_service = ClientConfigService(self._kv_store, self._state_backend, self._kv_adapter)
            # Note: ServiceStateService needs access to _sync_helper which is lazy-initialized
            # We pass a lambda that will access it when needed
            self._service_state_service = ServiceStateService(self._kv_store, self._state_backend, self._kv_adapter,
                                                              lambda: self._ensure_sync_helper())
            self._tool_management_service = ToolManagementService(self._kv_store, self._state_backend, self._kv_adapter,
                                                                 self)

        # Create a no-op cache_backend for backward compatibility with @atomic_write decorator
        # The decorator expects begin(), commit(), rollback() methods
        class NoOpBackend:
            def begin(self): pass

            def commit(self): pass

            def rollback(self): pass

        self.cache_backend = NoOpBackend()

        # Initialize service mapping for dynamic proxy (zero delegation mode)
        if factory_mode:
            # ToolManagementService needs registry reference, create it now
            from .tool_management_service import ToolManagementService
            self._tool_management_service = ToolManagementService(
                self._kv_store,
                self._state_backend,
                self._kv_adapter,
                self
            )

            self._service_mapping = {
                'service_state': self._service_state_service,
                'agent_client': self._agent_client_service,
                'client_config': self._client_config_service,
                'tool_management': self._tool_management_service
            }
            logger.info("Dynamic proxy service mapping initialized for zero delegation mode")
        else:
            self._service_mapping = {}

        # Sessions remain in memory (not serializable)
        # agent_id -> {service_name: session}
        self.sessions: Dict[str, Dict[str, Any]] = {}

        # Legacy in-memory structures for backward compatibility
        # These will be gradually migrated to use _kv_store
        # agent_id -> {tool_name: tool_definition}
        self.tool_cache: Dict[str, Dict[str, Dict[str, Any]]] = {}
        # agent_id -> {tool_name: session}
        self.tool_to_session_map: Dict[str, Dict[str, Any]] = {}
        # agent_id -> {tool_name: service_name} (hard mapping)
        self.tool_to_service: Dict[str, Dict[str, str]] = {}
        # Long-lived connection service markers - agent_id:service_name
        self.long_lived_connections: Set[str] = set()

        # Lifecycle state support
        # agent_id -> {service_name: ServiceConnectionState}
        self.service_states: Dict[str, Dict[str, ServiceConnectionState]] = {}
        # agent_id -> {service_name: ServiceStateMetadata}
        self.service_metadata: Dict[str, Dict[str, ServiceStateMetadata]] = {}

        # Agent-Client mapping cache
        self.agent_clients: Dict[str, List[str]] = {}
        # Structure: {agent_id: [client_id1, client_id2, ...]}

        # Client configuration cache
        self.client_configs: Dict[str, Dict[str, Any]] = {}
        # Structure: {client_id: {"mcpServers": {...}}}

        # Service to Client reverse mapping
        self.service_to_client: Dict[str, Dict[str, str]] = {}
        # Structure: {agent_id: {service_name: client_id}}

        # Cache synchronization status
        from datetime import datetime
        self.cache_sync_status: Dict[str, datetime] = {}

        # Agent service mapping relationships
        # agent_id -> {local_name: global_name}
        self.agent_to_global_mappings: Dict[str, Dict[str, str]] = {}
        # global_name -> (agent_id, local_name)
        self.global_to_agent_mappings: Dict[str, Tuple[str, str]] = {}

        # State synchronization manager (lazy initialization)
        self._state_sync_manager = None

        # === Snapshot (A+B+D): immutable bundle and versioning ===
        # Current effective snapshot bundle (immutable structure); read paths only read this pointer,
        # publishing happens through atomic pointer exchange
        self._tools_snapshot_bundle: Optional[Dict[str, Any]] = None
        self._tools_snapshot_version: int = 0
        # Snapshot dirty flag: set to True when cache changes (add/remove/clear)
        self._tools_snapshot_dirty: bool = True

        # Initialize AsyncSyncHelper for sync-to-async conversion
        # This allows synchronous methods to call async KV store operations
        self._sync_helper: Optional[Any] = None  # Lazy initialization

        logger.debug(
            f"ServiceRegistry initialized (id={id(self)}) with py-key-value backend, snapshot_version={self._tools_snapshot_version}")

    def _ensure_sync_helper(self):
        """Ensure AsyncSyncHelper is initialized (lazy initialization)."""
        if self._sync_helper is None:
            from mcpstore.core.utils.async_sync_helper import AsyncSyncHelper
            self._sync_helper = AsyncSyncHelper()
            logger.debug("AsyncSyncHelper initialized for ServiceRegistry")
        return self._sync_helper

    def _sync_to_kv(self, coro, operation_name: str = "KV operation"):
        """
        Synchronously execute an async KV store operation.
        
        This method bridges synchronous code (like add_service) with async KV operations.
        It uses AsyncSyncHelper to handle event loop management intelligently.
        
        Args:
            coro: Coroutine to execute
            operation_name: Description of the operation (for logging)
        
        Note:
            Failures are logged but don't raise exceptions to avoid breaking
            the main flow. The in-memory cache remains the source of truth.
        """
        try:
            logger.debug(f"[KV_SYNC] Starting sync: {operation_name}")
            helper = self._ensure_sync_helper()
            helper.run_async(coro, timeout=5.0)
            logger.debug(f"[KV_SYNC] Successfully synced: {operation_name}")
        except Exception as e:
            # Log cache operation failure with context
            logger.warning(
                f"Cache operation failed: {operation_name}. "
                f"Error: {type(e).__name__}: {e}. "
                f"In-memory cache remains consistent."
            )
            # Don't raise - memory cache is still updated, KV sync is best-effort

    def _get_collection(self, agent_id: str, data_type: str) -> str:
        """Generate Collection name for py-key-value storage.
        
        This method implements the Collection mapping strategy for organizing data
        in py-key-value. It supports both Store mode (using global_agent_store_id)
        and Agent mode (using actual agent_id).
        
        Args:
            agent_id: Agent ID (Store mode uses 'global_agent_store', Agent mode uses actual agent_id)
            data_type: Data type identifier (tools | states | metadata | clients | mappings)
        
        Returns:
            Collection name in format: agent:{agent_id}:{data_type}
        
        Examples:
            >>> _get_collection("global_agent_store", "tools")
            "agent:global_agent_store:tools"
            
            >>> _get_collection("agent_001", "states")
            "agent:agent_001:states"
            
            >>> _get_collection("my_agent", "metadata")
            "agent:my_agent:metadata"
        
        Note:
            This method is idempotent - calling it multiple times with the same
            parameters will always return the same Collection name.
            
        Validates:
            - Requirements 3.1: Agent isolation semantics preservation
            - Requirements 11.1: Store mode Agent isolation
            - Requirements 15.1: Collection naming conventions
            - Requirements 15.3: Collection generation logic
        """
        return f"agent:{agent_id}:{data_type}"

    def _wrap_scalar_value(self, value: Any) -> Dict[str, Any]:
        """
        Wrap a scalar value in a dictionary for py-key-value storage.
        
        This method ensures type safety when storing scalar values in py-key-value,
        which expects dict[str, Any] for its put() method. By wrapping scalars in
        a dictionary, we prevent beartype warnings and maintain type correctness.
        
        Args:
            value: Scalar value (str, int, bool, float, None) or dict
        
        Returns:
            Dictionary with structure: {"value": value}
            If input is already a dict, returns it unchanged.
        
        Examples:
            >>> _wrap_scalar_value("mcpstore")
            {"value": "mcpstore"}
            
            >>> _wrap_scalar_value(42)
            {"value": 42}
            
            >>> _wrap_scalar_value(True)
            {"value": True}
            
            >>> _wrap_scalar_value(None)
            {"value": None}
            
            >>> _wrap_scalar_value({"already": "wrapped"})
            {"already": "wrapped"}
        
        Note:
            - Uses consistent key "value" for all scalar types
            - Passes through dict values unchanged
            - Handles None as a valid scalar
            - Logs warnings for unexpected non-scalar types
        
        Validates:
            - Requirements 1.1: Wrapping service names before storage
            - Requirements 1.3: Wrapping scalar values in dictionaries
            - Requirements 2.1: Using standard key name consistently
        """
        # Pass through dicts unchanged
        if isinstance(value, dict):
            return value

        # Validate scalar types and log warnings for unexpected types
        if not isinstance(value, (str, int, float, bool, type(None))):
            logger.warning(
                f"Wrapping non-scalar type {type(value).__name__}. "
                f"Consider using dict directly for complex types."
            )

        return {"value": value}

    def _unwrap_scalar_value(self, wrapped: Any) -> Any:
        """
        Unwrap a scalar value from dictionary storage format.
        
        This method extracts scalar values from the wrapped dictionary format,
        maintaining backward compatibility with legacy unwrapped data. It handles
        both the new wrapped format ({"value": x}) and legacy format (x).
        
        Args:
            wrapped: Value from py-key-value (dict or legacy scalar)
        
        Returns:
            Unwrapped scalar value
        
        Examples:
            >>> _unwrap_scalar_value({"value": "mcpstore"})
            "mcpstore"
            
            >>> _unwrap_scalar_value("mcpstore")  # Legacy format
            "mcpstore"
            
            >>> _unwrap_scalar_value(None)
            None
            
            >>> _unwrap_scalar_value({"value": 42})
            42
        
        Note:
            - Handles both new wrapped format and legacy unwrapped format
            - Logs migration info when encountering legacy format
            - Returns None if wrapped is None
            - Provides backward compatibility for existing cached data
        
        Validates:
            - Requirements 1.2: Extracting service names from dictionaries
            - Requirements 1.5: Maintaining backward compatibility
            - Requirements 3.1: Reading both old and new formats
            - Requirements 3.3: Logging migration for observability
        """
        if wrapped is None:
            return None

        # New wrapped format
        if isinstance(wrapped, dict) and "value" in wrapped:
            return wrapped["value"]

        # Legacy format - return as-is with migration logging
        logger.info(
            f"Migrating legacy unwrapped value: {type(wrapped).__name__}. "
            f"Value will be wrapped on next write."
        )
        return wrapped

    def _is_wrapped_value(self, value: Any) -> bool:
        """
        Check if a value is in wrapped format.
        
        This method determines whether a value has been wrapped using the
        _wrap_scalar_value method by checking for the standard "value" key
        in a dictionary structure.
        
        Args:
            value: Value to check
        
        Returns:
            True if value is a dict with "value" key, False otherwise
        
        Examples:
            >>> _is_wrapped_value({"value": "mcpstore"})
            True
            
            >>> _is_wrapped_value("mcpstore")
            False
            
            >>> _is_wrapped_value({"other": "data"})
            False
            
            >>> _is_wrapped_value(None)
            False
        
        Note:
            - Used for validation and debugging
            - Helps identify legacy vs. new format data
            - Simple check: dict with "value" key
        
        Validates:
            - Requirements 2.1: Consistent wrapping key usage
            - Requirements 2.2: Same wrapping pattern for all types
        """
        return isinstance(value, dict) and "value" in value

    def configure_cache_backend(self, cache_config: Dict[str, Any]) -> None:
        """
        Configure the cache backend for the registry.
        
        This method allows runtime configuration of the cache backend, particularly
        for Redis configuration. It creates the appropriate kv_store based on the
        configuration and replaces the current _kv_store.
        
        Args:
            cache_config: Cache configuration dictionary with structure:
                {
                    "backend": "redis" | "memory",
                    "redis": {
                        "url": str,
                        "password": str,
                        "namespace": str,
                        "socket_timeout": float,
                        "healthcheck_interval": int,
                        "max_connections": int
                    },
                    "mode": "local" | "hybrid" | "shared"
                }
        
        Note:
            This method is called by setup_manager during initialization when
            Redis backend is configured.
        """
        backend_type = cache_config.get("backend", "memory")

        if backend_type == "redis":
            # Build Redis kv_store
            from mcpstore.core.registry.kv_store_factory import _build_kv_store

            redis_config = cache_config.get("redis", {})

            # Build configuration for kv_store_factory with defaults for None values
            factory_config = {
                "type": "redis",
                "url": redis_config.get("url"),
                "password": redis_config.get("password"),
                "namespace": redis_config.get("namespace"),
                # Enable wrappers by default
                "enable_statistics": True,
                "enable_size_limit": True,
                "max_item_size": 1024 * 1024,  # 1MB default
            }

            # Only add optional numeric parameters if they are provided (not None)
            # This allows the factory to use its own defaults
            if redis_config.get("socket_timeout") is not None:
                factory_config["socket_timeout"] = redis_config.get("socket_timeout")
            if redis_config.get("healthcheck_interval") is not None:
                factory_config["healthcheck_interval"] = redis_config.get("healthcheck_interval")
            if redis_config.get("max_connections") is not None:
                factory_config["max_connections"] = redis_config.get("max_connections")

            # Build the kv_store with wrappers
            self._kv_store = _build_kv_store(factory_config)
            logger.info(f"Configured Redis cache backend with namespace: {redis_config.get('namespace')}")
        else:
            # Memory backend (already initialized in __init__)
            logger.debug("Using default Memory cache backend")

    # === Async Tool Cache Access Methods ===

    # === Async Service Metadata Access Methods ===

    # === Async Tool Mapping Access Methods ===

    def _ensure_state_sync_manager(self):
        """确保状态同步管理器已初始化"""
        if self._state_sync_manager is None:
            from mcpstore.core.sync.shared_client_state_sync import SharedClientStateSyncManager
            self._state_sync_manager = SharedClientStateSyncManager(self)
            logger.debug("[REGISTRY] state_sync_manager initialized")

    def clear(self, agent_id: str):
        """
        清空指定 agent_id 的所有注册服务和工具。
        只影响该 agent_id 下的服务、工具、会话，不影响其它 agent。
        """
        service_names = set(self.sessions.get(agent_id, {}).keys())
        service_names.update(self.service_states.get(agent_id, {}).keys())
        for service_name in list(service_names):
            try:
                self.remove_service(agent_id, service_name)
            except Exception as e:
                logger.warning(f"Failed to remove service {service_name} during clear for agent {agent_id}: {e}")

        self.sessions.pop(agent_id, None)
        self.tool_cache.pop(agent_id, None)
        self.tool_to_session_map.pop(agent_id, None)
        self.tool_to_service.pop(agent_id, None)

        #  清理新增的缓存字段
        self.service_states.pop(agent_id, None)
        self.service_metadata.pop(agent_id, None)
        self.service_to_client.pop(agent_id, None)

        # 清理Agent-Client映射和相关Client配置
        client_ids = self.agent_clients.pop(agent_id, [])
        for client_id in client_ids:
            # 检查client是否被其他agent使用
            is_used_by_others = any(
                client_id in clients for other_agent, clients in self.agent_clients.items()
                if other_agent != agent_id
            )
            if not is_used_by_others:
                self.client_configs.pop(client_id, None)

    def _remove_service_tools(self, agent_id: str, service_name: str):
        tools_to_remove = [
            tool_name
            for tool_name, mapped_service in self.tool_to_service.get(agent_id, {}).items()
            if mapped_service == service_name
        ]
        for tool_name in tools_to_remove:
            if tool_name in self.tool_cache.get(agent_id, {}):
                del self.tool_cache[agent_id][tool_name]
            if tool_name in self.tool_to_session_map.get(agent_id, {}):
                del self.tool_to_session_map[agent_id][tool_name]
            if agent_id in self.tool_to_service and tool_name in self.tool_to_service[agent_id]:
                del self.tool_to_service[agent_id][tool_name]
            self._sync_to_kv(
                self.delete_tool_cache_async(agent_id, tool_name),
                f"tool_cache:{agent_id}:{tool_name}"
            )
            self._sync_to_kv(
                self.delete_tool_to_service_mapping_async(agent_id, tool_name),
                f"tool_mapping:{agent_id}:{tool_name}"
            )
        if tools_to_remove:
            self._tools_snapshot_dirty = True

    @atomic_write(agent_id_param="agent_id", use_lock=True)
    def add_service(self, agent_id: str, name: str, session: Any = None, tools: List[Tuple[str, Dict[str, Any]]] = None,
                    service_config: Dict[str, Any] = None, state: 'ServiceConnectionState' = None,
                    preserve_mappings: bool = False) -> List[str]:
        """
        为指定 agent_id 注册服务及其工具（支持所有状态的服务）
        - agent_id: store/agent 的唯一标识
        - name: 服务名
        - session: 服务会话对象（可选，失败的服务为None）
        - tools: [(tool_name, tool_def)]（可选，失败的服务为空列表）
        - service_config: 服务配置信息
        - state: 服务状态（可选，如果不提供则根据session判断）
        - preserve_mappings: 是否保留现有的Agent-Client映射关系（优雅修复用）
        返回实际注册的工具名列表。
        """
        #  新增：支持所有状态的服务注册
        tools = tools or []
        service_config = service_config or {}

        # 初始化数据结构
        if agent_id not in self.sessions:
            self.sessions[agent_id] = {}
        if agent_id not in self.tool_cache:
            self.tool_cache[agent_id] = {}
        if agent_id not in self.tool_to_session_map:
            self.tool_to_session_map[agent_id] = {}
        if agent_id not in self.tool_to_service:
            self.tool_to_service[agent_id] = {}
        if agent_id not in self.service_states:
            self.service_states[agent_id] = {}
        if agent_id not in self.service_metadata:
            self.service_metadata[agent_id] = {}

        # 确定服务状态
        if state is None:
            if session is not None and len(tools) > 0:
                from mcpstore.core.models.service import ServiceConnectionState
                state = ServiceConnectionState.HEALTHY
            elif session is not None:
                from mcpstore.core.models.service import ServiceConnectionState
                state = ServiceConnectionState.WARNING  # 有连接但无工具
            else:
                from mcpstore.core.models.service import ServiceConnectionState
                state = ServiceConnectionState.DISCONNECTED  # 连接失败

        #  优雅修复：智能处理现有服务
        if name in self.sessions[agent_id]:
            if preserve_mappings:
                # 保留映射关系，只清理工具缓存
                logger.debug(f"[ADD_SERVICE] exists keep_mappings=True clear_tools_only name={name}")
                self.clear_service_tools_only(agent_id, name)
            else:
                # 传统逻辑：完全移除服务
                logger.debug(
                    f"Re-registering service: {name} for agent {agent_id}. Removing old service before overwriting.")
                self.remove_service(agent_id, name)

        # 存储服务信息（即使连接失败也存储）
        self.sessions[agent_id][name] = session  # 失败的服务session为None
        self.service_states[agent_id][name] = state

        # Sync service state to KV store
        self._sync_to_kv(
            self.set_service_state_async(agent_id, name, state),
            f"service_state:{agent_id}:{name}"
        )

        # 关键：存储完整的服务配置和元数据
        if name not in self.service_metadata[agent_id]:
            from mcpstore.core.models.service import ServiceStateMetadata
            from datetime import datetime
            metadata = ServiceStateMetadata(
                service_name=name,
                agent_id=agent_id,
                state_entered_time=datetime.now(),
                service_config=service_config,  # 存储完整配置
                consecutive_failures=0 if session else 1,
                error_message=None if session else "Connection failed"
            )
            self.service_metadata[agent_id][name] = metadata

            # Sync metadata to KV store
            self._sync_to_kv(
                self.set_service_metadata_async(agent_id, name, metadata),
                f"service_metadata:{agent_id}:{name}"
            )
        else:
            #  修复：如果metadata已存在，也要更新service_config
            # 这确保了配置信息始终是最新的
            existing_metadata = self.service_metadata[agent_id][name]
            if service_config:  # 只在提供了新配置时更新
                existing_metadata.service_config = service_config
                logger.debug(f"[ADD_SERVICE] Updated service_config for existing service: {name}")

                # Sync updated metadata to KV store
                self._sync_to_kv(
                    self.set_service_metadata_async(agent_id, name, existing_metadata),
                    f"service_metadata:{agent_id}:{name}"
                )

        added_tool_names = []
        for tool_name, tool_definition in tools:
            # Use new tool ownership determination logic
            # 检查工具定义中的服务归属
            tool_service_name = None
            if "function" in tool_definition:
                tool_service_name = tool_definition["function"].get("service_name")
            else:
                tool_service_name = tool_definition.get("service_name")

            # 验证工具是否属于当前服务
            if tool_service_name and tool_service_name != name:
                logger.warning(
                    f"Tool '{tool_name}' belongs to service '{tool_service_name}', not '{name}'. Skipping this tool.")
                continue

            # 检查工具名冲突
            if tool_name in self.tool_cache[agent_id]:
                existing_session = self.tool_to_session_map[agent_id].get(tool_name)
                if existing_session is not session:
                    logger.warning(
                        f"Tool name conflict: '{tool_name}' from {name} for agent {agent_id} conflicts with existing tool. Skipping this tool.")
                    continue

            # 存储工具到内存
            self.tool_cache[agent_id][tool_name] = tool_definition
            self.tool_to_session_map[agent_id][tool_name] = session

            # Map tool to service in memory
            if agent_id not in self.tool_to_service:
                self.tool_to_service[agent_id] = {}
            self.tool_to_service[agent_id][tool_name] = name

            # Sync tool definition to KV store
            self._sync_to_kv(
                self.set_tool_cache_async(agent_id, tool_name, tool_definition),
                f"tool_cache:{agent_id}:{tool_name}"
            )

            # Sync tool-to-service mapping to KV store
            self._sync_to_kv(
                self.set_tool_to_service_mapping_async(agent_id, tool_name, name),
                f"tool_mapping:{agent_id}:{tool_name}"
            )

            # Mark snapshot as dirty
            self._tools_snapshot_dirty = True
            added_tool_names.append(tool_name)

        logger.debug(f"Service added: {name} ({state.value}, {len(tools)} tools) for agent {agent_id}")
        return added_tool_names

    def add_failed_service(self, agent_id: str, name: str, service_config: Dict[str, Any],
                           error_message: str, state: 'ServiceConnectionState' = None):
        """
        注册失败的服务到缓存
        """
        if state is None:
            from mcpstore.core.models.service import ServiceConnectionState
            state = ServiceConnectionState.DISCONNECTED

        added_tools = self.add_service(
            agent_id=agent_id,
            name=name,
            session=None,
            tools=[],
            service_config=service_config,
            state=state
        )

        # 更新错误信息
        if agent_id in self.service_metadata and name in self.service_metadata[agent_id]:
            self.service_metadata[agent_id][name].error_message = error_message

        return added_tools

    @atomic_write(agent_id_param="agent_id", use_lock=True)
    def replace_service_tools(self, agent_id: str, service_name: str, session: Any, remote_tools: List[Any]) -> Dict[
        str, int]:
        """
        规范化并原子替换某服务的工具缓存：
        - 强制键名使用带前缀全名: {service}_{original}
        - 强制 schema 写入 function.parameters（将 inputSchema 统一转换）
        - 设置 function.display_name=original_name, function.service_name=service_name
        - 保留现有的 Agent-Client 映射与 service 配置与状态

        Returns:
            Dict: {"replaced": int, "invalid": int}
        """
        replaced_count = 0
        invalid_count = 0

        try:
            # 仅清理工具，不动映射
            self.clear_service_tools_only(agent_id, service_name)

            processed: List[Tuple[str, Dict[str, Any]]] = []

            def _get(original: Any, key: str, default: Any = None) -> Any:
                # 支持对象或字典两种形态读取
                if isinstance(original, dict):
                    return original.get(key, default)
                return getattr(original, key, default)

            for tool in remote_tools or []:
                try:
                    original_name = _get(tool, 'name')
                    if not original_name or not isinstance(original_name, str):
                        invalid_count += 1
                        continue

                    # 归一 schema: 优先 inputSchema → parameters
                    schema = _get(tool, 'inputSchema')
                    if schema is None and isinstance(tool, dict):
                        # 兼容 function.parameters 已存在的情况
                        fn = tool.get('function')
                        if isinstance(fn, dict):
                            schema = fn.get('parameters')

                    description = _get(tool, 'description', '')

                    full_name = f"{service_name}_{original_name}"
                    tool_def: Dict[str, Any] = {
                        'type': 'function',
                        'function': {
                            'name': original_name,
                            'description': description or '',
                            'parameters': schema or {},
                            'display_name': original_name,
                            'service_name': service_name,
                        }
                    }
                    processed.append((full_name, tool_def))
                except Exception:
                    invalid_count += 1
                    continue

            # 使用现有状态与配置
            current_state = self.get_service_state(agent_id, service_name)
            service_config = self.get_service_config_from_cache(agent_id, service_name)

            self.add_service(
                agent_id=agent_id,
                name=service_name,
                session=session,
                tools=processed,
                service_config=service_config or {},
                state=current_state,
                preserve_mappings=True
            )
            replaced_count = len(processed)

            # 标脏快照，由读侧或上层触发重建
            try:
                if hasattr(self, 'mark_tools_snapshot_dirty'):
                    self.mark_tools_snapshot_dirty()
            except Exception:
                pass

            return {"replaced": replaced_count, "invalid": invalid_count}
        except Exception as e:
            logger.error(f"[REGISTRY] replace_service_tools failed: agent={agent_id} service={service_name} err={e}")
            return {"replaced": replaced_count, "invalid": invalid_count + 1}

    @atomic_write(agent_id_param="agent_id", use_lock=True)
    def remove_service(self, agent_id: str, name: str) -> Optional[Any]:
        """
        移除指定 agent_id 下的服务及其所有工具。
        只影响该 agent_id，不影响其它 agent。
        """
        session = self.sessions.get(agent_id, {}).pop(name, None)
        if not session:
            logger.debug(f"Service {name} has no active session for agent {agent_id}. Cleaning up cache data only.")
            self._remove_service_tools(agent_id, name)
            self._cleanup_service_cache_data(agent_id, name)
            return None

        self._remove_service_tools(agent_id, name)

        self._cleanup_service_cache_data(agent_id, name)

        # 标记并重建快照（强一致）
        try:
            if hasattr(self, 'tools_changed'):
                # 尝试使用全局agent（若无，则用当前agent作为兜底）
                gid = getattr(self, '_main_agent_id', None) or agent_id
                self.tools_changed(global_agent_id=gid, aggressive=True)
        except Exception:
            try:
                self.mark_tools_snapshot_dirty()
            except Exception:
                pass
        logger.debug(f"Service removed: {name} for agent {agent_id}")
        return session

    @atomic_write(agent_id_param="agent_id", use_lock=True)
    def clear_service_tools_only(self, agent_id: str, service_name: str):
        """
        只清理服务的工具缓存，保留Agent-Client映射关系

        这是优雅修复方案的核心方法：
        - 清理工具缓存和工具-会话映射
        - 保留Agent-Client映射
        - 保留Client配置
        - 保留Service-Client映射
        """
        try:
            logger.debug(
                f"[REGISTRY.CLEAR_TOOLS_ONLY] begin agent={agent_id} service={service_name} tool_cache_size={len(self.tool_cache.get(agent_id, {}))}")
            # 获取现有会话
            existing_session = self.sessions.get(agent_id, {}).get(service_name)
            if not existing_session:
                logger.debug(f"[CLEAR_TOOLS] no_session service={service_name} skip=True")
                return

            # 只清理工具相关的缓存
            tools_to_remove = [
                tool_name for tool_name, owner_session
                in self.tool_to_session_map.get(agent_id, {}).items()
                if owner_session is existing_session
            ]

            for tool_name in tools_to_remove:
                # 清理工具缓存
                if agent_id in self.tool_cache and tool_name in self.tool_cache[agent_id]:
                    del self.tool_cache[agent_id][tool_name]
                # 清理工具-会话映射
                if agent_id in self.tool_to_session_map and tool_name in self.tool_to_session_map[agent_id]:
                    del self.tool_to_session_map[agent_id][tool_name]
                # 清理工具-服务硬映射 (in-memory)
                if agent_id in self.tool_to_service and tool_name in self.tool_to_service[agent_id]:
                    del self.tool_to_service[agent_id][tool_name]
            # Mark snapshot as dirty
            self._tools_snapshot_dirty = True

            # 清理会话（会被新会话替换）
            if agent_id in self.sessions and service_name in self.sessions[agent_id]:
                del self.sessions[agent_id][service_name]

            logger.debug(
                f"[CLEAR_TOOLS] cleared_tools service={service_name} count={len(tools_to_remove)} keep_mappings=True")

        except Exception as e:
            logger.error(f"Failed to clear service tools for {service_name}: {e}")
        # 强一致：工具清理后立即触发快照更新
        try:
            gid = getattr(self, '_main_agent_id', None) or agent_id
            if hasattr(self, 'tools_changed'):
                self.tools_changed(global_agent_id=gid, aggressive=True)
        except Exception:
            try:
                self.mark_tools_snapshot_dirty()
            except Exception:
                pass

    def _cleanup_service_cache_data(self, agent_id: str, service_name: str):
        """清理服务相关的缓存数据"""
        # 清理服务状态和元数据
        if agent_id in self.service_states:
            self.service_states[agent_id].pop(service_name, None)
        if agent_id in self.service_metadata:
            self.service_metadata[agent_id].pop(service_name, None)

        self._sync_to_kv(
            self._service_state_service.delete_service_state_async(agent_id, service_name),
            f"service_state:{agent_id}:{service_name}"
        )
        self._sync_to_kv(
            self._service_state_service.delete_service_metadata_async(agent_id, service_name),
            f"service_metadata:{agent_id}:{service_name}"
        )

        # 清理Service-Client映射
        client_id = self._agent_client_service.get_service_client_id(agent_id, service_name)
        if client_id:
            self._agent_client_service.remove_service_client_mapping(agent_id, service_name)

            # 检查client是否还有其他服务
            client_config = self._client_config_service.get_client_config_from_cache(client_id)
            if client_config:
                remaining_services = client_config.get("mcpServers", {})
                if service_name in remaining_services:
                    del remaining_services[service_name]

                # 如果client没有其他服务，移除client
                if not remaining_services:
                    self._client_config_service.remove_client_config(client_id)
                    self._agent_client_service.remove_agent_client_mapping(agent_id, client_id)

    def get_session(self, agent_id: str, name: str) -> Optional[Any]:
        """
        Get service session for an agent (synchronous, in-memory only).
        
        Args:
            agent_id: Agent ID
            name: Service name
            
        Returns:
            Session object or None if not found
            
        Note:
            Sessions are ALWAYS stored in memory, never in py-key-value storage,
            because MCP Session objects are not serializable.
            This is a synchronous method and will remain so.
            
        Validates:
            - Requirements 3.2: Session 对象的序列化问题
            - Requirements 15.4: Session 数据隔离
        """
        return self.sessions.get(agent_id, {}).get(name)

    def set_session(self, agent_id: str, service_name: str, session: Any) -> None:
        """
        Set service session for an agent (synchronous, in-memory only).
        
        Args:
            agent_id: Agent ID
            service_name: Service name
            session: Session object to store
            
        Note:
            Sessions are ALWAYS stored in memory, never in py-key-value storage,
            because MCP Session objects are not serializable.
            This method includes defensive checks to prevent accidental serialization.
            
        Raises:
            SessionSerializationError: If session contains non-serializable references
            
        Validates:
            - Requirements 3.2: Session 对象的序列化问题
            - Requirements 4.1: ServiceRegistry 的数据存储层
        """
        # Import exception mapper for validation
        from .exception_mapper import validate_session_serializable

        # Defensive check: validate session doesn't contain non-serializable references
        validate_session_serializable(session, agent_id, service_name)

        # Store in memory
        if agent_id not in self.sessions:
            self.sessions[agent_id] = {}
        self.sessions[agent_id][service_name] = session

        logger.debug(f"Set session: agent={agent_id}, service={service_name}")

    def get_session_for_tool(self, agent_id: str, tool_name: str) -> Optional[Any]:
        """
        获取指定 agent_id 下工具对应的服务会话。
        """
        return self.tool_to_session_map.get(agent_id, {}).get(tool_name)

    def get_all_tools(self, agent_id: str) -> List[Dict[str, Any]]:
        """
        获取指定 agent_id 下所有工具的定义。
        """
        all_tools = []
        for tool_name, tool_def in self.tool_cache.get(agent_id, {}).items():
            session = self.tool_to_session_map.get(agent_id, {}).get(tool_name)
            service_name = None
            for name, sess in self.sessions.get(agent_id, {}).items():
                if sess is session:
                    service_name = name
                    break
            tool_with_service = tool_def.copy()
            if "function" not in tool_with_service and isinstance(tool_with_service, dict):
                tool_with_service = {
                    "type": "function",
                    "function": tool_with_service
                }
            if "function" in tool_with_service:
                function_data = tool_with_service["function"]
                if service_name:
                    original_description = function_data.get("description", "")
                    if not original_description.endswith(f" (来自服务: {service_name})"):
                        function_data["description"] = f"{original_description} (来自服务: {service_name})"
                function_data["service_info"] = {"service_name": service_name}
            all_tools.append(tool_with_service)
        logger.debug(
            f"Retrieved {len(all_tools)} tools from {len(self.get_all_service_names(agent_id))} services for agent {agent_id}")
        return all_tools

    def get_all_tool_info(self, agent_id: str) -> List[Dict[str, Any]]:
        """
        获取指定 agent_id 下所有工具的详细信息。
        """
        tools_info = []
        for tool_name in self.tool_cache.get(agent_id, {}).keys():
            session = self.tool_to_session_map.get(agent_id, {}).get(tool_name)
            service_name = None
            for name, sess in self.sessions.get(agent_id, {}).items():
                if sess is session:
                    service_name = name
                    break
            detailed_tool = self._get_detailed_tool_info(agent_id, tool_name)
            if detailed_tool:
                detailed_tool["service_name"] = service_name
                tools_info.append(detailed_tool)
        return tools_info

    def get_connected_services(self, agent_id: str) -> List[Dict[str, Any]]:
        """
        获取指定 agent_id 下所有已连接服务的信息。
        """
        services = []
        for name in self.get_all_service_names(agent_id):
            tools = self.get_tools_for_service(agent_id, name)
            services.append({
                "name": name,
                "tool_count": len(tools)
            })
        return services

    def get_tools_for_service(self, agent_id: str, name: str) -> List[str]:
        """
        获取指定 agent_id 下某服务的所有工具名。
         修复：改为从service_to_client映射和tool_cache获取，而不是依赖sessions
        """
        logger.info(f"[REGISTRY] get_tools service={name} agent_id={agent_id}")

        #  修复：首先检查服务是否存在
        if not self.has_service(agent_id, name):
            logger.warning(f"[REGISTRY] service_not_exists service={name}")
            return []

        #  优先：使用工具→服务硬映射
        tools = []
        tool_cache = self.tool_cache.get(agent_id, {})
        tool_to_session = self.tool_to_session_map.get(agent_id, {})
        tool_to_service = self.tool_to_service.get(agent_id, {})

        # 获取该服务的session（如果存在）
        service_session = self.sessions.get(agent_id, {}).get(name)

        logger.debug(
            f"[REGISTRY] tool_cache_size={len(tool_cache)} tool_to_session_size={len(tool_to_session)} tool_to_service_size={len(tool_to_service)}")

        for tool_name in tool_cache.keys():
            mapped_service = tool_to_service.get(tool_name)
            if mapped_service == name:
                tools.append(tool_name)
                continue
            # 次选：当硬映射缺失时，使用会话匹配（避免历史数据缺口）
            tool_session = tool_to_session.get(tool_name)
            if service_session and tool_session is service_session:
                tools.append(tool_name)

        logger.debug(f"[REGISTRY] found_tools service={name} count={len(tools)} list={tools}")
        return tools

    def _extract_description_from_schema(self, prop_info):
        """从 schema 中提取描述信息"""
        if isinstance(prop_info, dict):
            # 优先查找 description 字段
            if 'description' in prop_info:
                return prop_info['description']
            # 其次查找 title 字段
            elif 'title' in prop_info:
                return prop_info['title']
            # 检查是否有 anyOf 或 allOf 结构
            elif 'anyOf' in prop_info:
                for item in prop_info['anyOf']:
                    if isinstance(item, dict) and 'description' in item:
                        return item['description']
            elif 'allOf' in prop_info:
                for item in prop_info['allOf']:
                    if isinstance(item, dict) and 'description' in item:
                        return item['description']

        return "无描述"

    def _extract_type_from_schema(self, prop_info):
        """从 schema 中提取类型信息"""
        if isinstance(prop_info, dict):
            if 'type' in prop_info:
                return prop_info['type']
            elif 'anyOf' in prop_info:
                # 处理 Union 类型
                types = []
                for item in prop_info['anyOf']:
                    if isinstance(item, dict) and 'type' in item:
                        types.append(item['type'])
                return '|'.join(types) if types else '未知'
            elif 'allOf' in prop_info:
                # 处理 intersection 类型
                for item in prop_info['allOf']:
                    if isinstance(item, dict) and 'type' in item:
                        return item['type']

        return "未知"

    def get_tool_info(self, agent_id: str, tool_name: str) -> Dict[str, Any]:
        """
        获取指定 agent_id 下某工具的详细信息，返回格式化的工具信息。
        """
        tool_def = self.tool_cache.get(agent_id, {}).get(tool_name)
        if not tool_def:
            return None

        session = self.tool_to_session_map.get(agent_id, {}).get(tool_name)
        service_name = None
        if session:
            for name, sess in self.sessions.get(agent_id, {}).items():
                if sess is session:
                    service_name = name
                    break

        # 获取 Client ID
        client_id = self._agent_client_service.get_service_client_id(agent_id, service_name) if service_name else None

        # 处理不同的工具定义格式
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

    def _get_detailed_tool_info(self, agent_id: str, tool_name: str) -> Dict[str, Any]:
        """
        获取指定 agent_id 下某工具的详细信息。
        """
        tool_def = self.tool_cache.get(agent_id, {}).get(tool_name)
        if not tool_def:
            return {}
        session = self.tool_to_session_map.get(agent_id, {}).get(tool_name)
        service_name = None
        if session:
            for name, sess in self.sessions.get(agent_id, {}).items():
                if sess is session:
                    service_name = name
                    break

        if "function" in tool_def:
            function_data = tool_def["function"]
            tool_info = {
                "name": tool_name,  # 这是存储的键名（显示名称）
                "display_name": function_data.get("display_name", tool_name),  # 用户友好的显示名称
                "description": function_data.get("description", ""),
                "service_name": service_name,
                "inputSchema": function_data.get("parameters", {}),
                "original_name": function_data.get("name", tool_name)  # FastMCP 原始名称
            }
        else:
            tool_info = {
                "name": tool_name,
                "display_name": tool_def.get("display_name", tool_name),
                "description": tool_def.get("description", ""),
                "service_name": service_name,
                "inputSchema": tool_def.get("parameters", {}),
                "original_name": tool_def.get("name", tool_name)
            }
        return tool_info

    def get_service_details(self, agent_id: str, name: str) -> Dict[str, Any]:
        """
        获取指定 agent_id 下某服务的详细信息。
        """
        if name not in self.sessions.get(agent_id, {}):
            return {}

        logger.info(f"Getting service details for: {name} (agent_id={agent_id})")
        session = self.sessions.get(agent_id, {}).get(name)

        # 只在调试特定问题时打印详细日志
        logger.debug(
            f"get_service_details: agent_id={agent_id}, name={name}, session_id={id(session) if session else None}")

        tools = self.get_tools_for_service(agent_id, name)
        # service_health已废弃，使用None作为默认值
        last_heartbeat = None
        detailed_tools = []
        for tool_name in tools:
            detailed_tool = self._get_detailed_tool_info(agent_id, tool_name)
            if detailed_tool:
                detailed_tools.append(detailed_tool)
        # TODO: 添加Resources和Prompts信息收集
        # 当前版本暂时返回空值，后续版本将实现完整的资源和提示词统计

        return {
            "name": name,
            "tools": detailed_tools,
            "tool_count": len(tools),
            "tool_names": [tool["name"] for tool in detailed_tools],

            # 新增：Resources相关字段
            "resource_count": 0,  # TODO: 实现资源数量统计
            "resource_names": [],  # TODO: 实现资源名称列表
            "resource_template_count": 0,  # TODO: 实现资源模板数量统计
            "resource_template_names": [],  # TODO: 实现资源模板名称列表

            # 新增：Prompts相关字段
            "prompt_count": 0,  # TODO: 实现提示词数量统计
            "prompt_names": [],  # TODO: 实现提示词名称列表

            # 新增：能力标识
            "capabilities": ["tools"],  # TODO: 根据实际支持的功能动态更新

            # 现有字段
            "last_heartbeat": str(last_heartbeat) if last_heartbeat else "N/A",
            "connected": name in self.sessions.get(agent_id, {})
        }

    def get_services_for_agent(self, agent_id: str) -> List[str]:
        """
        获取指定 agent_id 下所有已注册服务名（别名方法）
        """
        return self.get_all_service_names(agent_id)

    def get_service_info(self, agent_id: str, service_name: str) -> Optional['ServiceInfo']:
        """
        获取指定服务的基本信息

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            ServiceInfo对象或None
        """
        try:
            # 检查服务是否存在
            if service_name not in self.sessions.get(agent_id, {}):
                return None

            # 获取服务状态
            state = self.get_service_state(agent_id, service_name)

            # 获取工具数量
            tools = self.get_tools_for_service(agent_id, service_name)
            tool_count = len(tools)

            # 获取服务元数据
            metadata = self.get_service_metadata(agent_id, service_name)

            # 构造ServiceInfo对象
            from mcpstore.core.models.service import ServiceInfo, TransportType
            from datetime import datetime

            # 尝试从元数据中获取配置信息
            service_config = metadata.service_config if metadata else {}

            # 推断传输类型
            transport_type = TransportType.STREAMABLE_HTTP  # 默认
            if 'url' in service_config:
                transport_type = TransportType.STREAMABLE_HTTP
            elif 'command' in service_config:
                transport_type = TransportType.STDIO

            service_info = ServiceInfo(
                name=service_name,
                transport_type=transport_type,
                status=state,
                tool_count=tool_count,
                url=service_config.get('url', ''),
                command=service_config.get('command'),
                args=service_config.get('args'),
                working_dir=service_config.get('working_dir'),
                env=service_config.get('env'),
                keep_alive=service_config.get('keep_alive', False),
                package_name=service_config.get('package_name'),
                last_heartbeat=metadata.last_ping_time if metadata else None,
                last_state_change=metadata.state_entered_time if metadata else datetime.now(),
                state_metadata=metadata,
                config=service_config  # [REFACTOR] 添加完整的config字段
            )

            return service_info

        except Exception as e:
            logger.debug(f"获取服务信息时出现异常: {e}")
            return None

    def get_service_config(self, agent_id: str, name: str) -> Optional[Dict[str, Any]]:
        """获取服务配置"""
        try:
            # 1) 服务不存在：直接返回 None
            if not self.has_service(agent_id, name):
                logger.debug(f"[REGISTRY] get_service_config: service_not_exists agent={agent_id} name={name}")
                return None

            # 2) 优先：从元数据缓存读取（单一真源）
            metadata = self.get_service_metadata(agent_id, name)
            if metadata and isinstance(metadata.service_config, dict) and metadata.service_config:
                logger.debug(f"[REGISTRY] get_service_config: from_metadata agent={agent_id} name={name}")
                return metadata.service_config

            # 3) 备用：从 Client 配置映射读取
            client_id = self.service_to_client.get(agent_id, {}).get(name)
            if client_id:
                client_cfg = self.client_configs.get(client_id, {}) or {}
                svc_cfg = (client_cfg.get("mcpServers", {}) or {}).get(name)
                if isinstance(svc_cfg, dict) and svc_cfg:
                    logger.debug(
                        f"[REGISTRY] get_service_config: from_client_configs agent={agent_id} name={name} client_id={client_id}")
                    return svc_cfg

            # 4) 未找到：返回 None，不依赖 Web 层
            logger.debug(f"[REGISTRY] get_service_config: not_found agent={agent_id} name={name}")
            return None

        except Exception as e:
            logger.warning(f"[REGISTRY] get_service_config error: {e}")
            return None

    
    
    
    
    
    
    
    
    
    
    
    def __getattr__(self, name: str):
        """
        动态方法代理 - 零委托模式的核心实现

        当访问不存在的方法时，自动查找并调用对应的服务方法
        这样就无需编写任何委托代码，实现了真正的零委托
        """
        if not hasattr(self, '_service_mapping') or not self._service_mapping:
            # Legacy mode - 可能是在升级过程中访问
            raise AttributeError(f"'{type(self).__name__}' object has no attribute '{name}' (legacy mode)")

        # 在所有服务中查找方法
        for service_name, service in self._service_mapping.items():
            if hasattr(service, name):
                method = getattr(service, name)
                logger.debug(f"[REGISTRY] Method '{name}' dynamically proxied to {service_name} service")
                return method

        # 如果没有找到，提供清晰的错误信息
        available_methods = []
        for service_name, service in self._service_mapping.items():
            methods = [f"{service_name}.{m}" for m in dir(service) if not m.startswith('_') and callable(getattr(service, m))]
            available_methods.extend(methods[:5])  # 每个服务只显示前5个方法

        raise AttributeError(
            f"Method '{name}' not found in any service. "
            f"Available methods (sample): {available_methods[:15]}..."
        )

    def mark_as_long_lived(self, agent_id: str, service_name: str):
        """标记服务为长连接服务"""
        service_key = f"{agent_id}:{service_name}"
        self.long_lived_connections.add(service_key)
        logger.debug(f"Marked service '{service_name}' as long-lived for agent '{agent_id}'")

    def is_long_lived_service(self, agent_id: str, service_name: str) -> bool:
        """检查服务是否为长连接服务"""
        service_key = f"{agent_id}:{service_name}"
        return service_key in self.long_lived_connections

    def get_long_lived_services(self, agent_id: str) -> List[str]:
        """获取指定Agent的所有长连接服务"""
        prefix = f"{agent_id}:"
        return [
            key[len(prefix):] for key in self.long_lived_connections
            if key.startswith(prefix)
        ]

    def remove_service_lifecycle_data(self, agent_id: str, service_name: str):
        """移除服务的生命周期数据"""
        if agent_id in self.service_states:
            self.service_states[agent_id].pop(service_name, None)
        if agent_id in self.service_metadata:
            self.service_metadata[agent_id].pop(service_name, None)
        logger.debug(f"Removed lifecycle data for service {service_name} (agent {agent_id})")

    def get_all_service_states(self, agent_id: str) -> Dict[str, ServiceConnectionState]:
        """获取指定Agent的所有服务状态"""
        return self.service_states.get(agent_id, {}).copy()

    def clear_agent_lifecycle_data(self, agent_id: str):
        """清除指定Agent的所有生命周期数据"""
        self.service_states.pop(agent_id, None)
        self.service_metadata.pop(agent_id, None)
        logger.info(f"Cleared lifecycle data for agent {agent_id}")

    def should_cache_aggressively(self, agent_id: str, service_name: str) -> bool:
        """
        判断是否应该激进缓存
        长连接服务可以更激进地缓存，因为连接稳定
        """
        return self.is_long_lived_service(agent_id, service_name)

    # ===  新增：Agent-Client 映射管理 ===

    # ===  新增：Service-Client 映射管理 ===

    @map_kv_exception
    async def set_service_client_mapping_async(self, agent_id: str, service_name: str, client_id: str) -> None:
        await self._state_backend.set_service_client(agent_id, service_name, client_id)

    @map_kv_exception
    async def delete_service_client_mapping_async(self, agent_id: str, service_name: str) -> None:
        await self._state_backend.delete_service_client(agent_id, service_name)

    def get_repository(self):
        """Return a Repository-style thin facade bound to this registry.
        Avoids circular import by importing locally.
        """
        try:
            from .repository import CacheRepository  # type: ignore
        except Exception as e:
            raise RuntimeError(f"CacheRepository unavailable: {e}")
        return CacheRepository(self)

    # ===  新增：Agent 服务映射管理 ===

    def add_agent_service_mapping(self, agent_id: str, local_name: str, global_name: str):
        """
        建立 Agent 服务映射关系

        Args:
            agent_id: Agent ID
            local_name: Agent 中的本地服务名
            global_name: Store 中的全局服务名（带后缀）
        """
        # 建立 agent -> global 映射
        if agent_id not in self.agent_to_global_mappings:
            self.agent_to_global_mappings[agent_id] = {}
        self.agent_to_global_mappings[agent_id][local_name] = global_name

        # 建立 global -> agent 映射
        self.global_to_agent_mappings[global_name] = (agent_id, local_name)

        logger.debug(f" [AGENT_MAPPING] Added mapping: {agent_id}:{local_name} ↔ {global_name}")

    def get_global_name_from_agent_service(self, agent_id: str, local_name: str) -> Optional[str]:
        """获取 Agent 服务对应的全局名称"""
        return self.agent_to_global_mappings.get(agent_id, {}).get(local_name)

    def get_agent_service_from_global_name(self, global_name: str) -> Optional[Tuple[str, str]]:
        """获取全局服务名对应的 Agent 服务信息"""
        return self.global_to_agent_mappings.get(global_name)

    def get_agent_services(self, agent_id: str) -> List[str]:
        """获取 Agent 的所有服务（全局名称）"""
        return list(self.agent_to_global_mappings.get(agent_id, {}).values())

    def is_agent_service(self, global_name: str) -> bool:
        """判断是否为 Agent 服务"""
        return global_name in self.global_to_agent_mappings

    def remove_agent_service_mapping(self, agent_id: str, local_name: str):
        """移除 Agent 服务映射"""
        if agent_id in self.agent_to_global_mappings:
            global_name = self.agent_to_global_mappings[agent_id].pop(local_name, None)
            if global_name:
                self.global_to_agent_mappings.pop(global_name, None)
                logger.debug(f" [AGENT_MAPPING] Removed mapping: {agent_id}:{local_name} ↔ {global_name}")

    # ===  新增：完整的服务信息获取 ===

    def get_service_summary(self, agent_id: str, service_name: str) -> Dict[str, Any]:
        """
        获取服务完整摘要信息

        Returns:
            {
                "name": "weather",
                "state": "healthy",
                "tool_count": 5,
                "tools": ["get_weather", "get_forecast"],
                "has_session": True,
                "last_heartbeat": "2024-01-01T12:00:00",
                "error_message": None,
                "config": {"url": "http://weather.com"}
            }
        """
        if not self.has_service(agent_id, service_name):
            logger.debug(f"Service not found: {service_name} for agent {agent_id}")
            return {}

        state = self.get_service_state(agent_id, service_name)
        metadata = self.get_service_metadata(agent_id, service_name)
        tools = self.get_tools_for_service(agent_id, service_name)
        session = self.get_session(agent_id, service_name)

        # 安全的时间格式化
        def safe_isoformat(dt):
            if dt is None:
                return None
            if hasattr(dt, 'isoformat'):
                return dt.isoformat()
            elif isinstance(dt, str):
                return dt
            else:
                return str(dt)

        return {
            "name": service_name,
            "state": state.value if state else "unknown",
            "tool_count": len(tools),
            "tools": tools,
            "has_session": session is not None,
            "last_heartbeat": safe_isoformat(metadata.last_ping_time if metadata else None),
            "error_message": metadata.error_message if metadata else None,
            "config": metadata.service_config if metadata else {},
            "consecutive_failures": metadata.consecutive_failures if metadata else 0,
            "state_entered_time": safe_isoformat(metadata.state_entered_time if metadata else None),
            # 修复：添加state_metadata字段，用于判断服务是否激活
            "state_metadata": metadata
        }

    def get_complete_service_info(self, agent_id: str, service_name: str) -> Dict[str, Any]:
        """获取服务的完整信息（包括 Client 信息）"""
        # 基础服务信息
        base_info = self.get_service_summary(agent_id, service_name)

        # Client 信息
        client_id = self._agent_client_service.get_service_client_id(agent_id, service_name)
        client_config = self._client_config_service.get_client_config_from_cache(client_id) if client_id else {}

        # 合并信息
        complete_info = {
            **base_info,
            "client_id": client_id,
            "client_config": client_config,
            "agent_id": agent_id
        }

        return complete_info

    def get_all_services_complete_info(self, agent_id: str) -> List[Dict[str, Any]]:
        """获取 Agent 下所有服务的完整信息"""
        service_names = self.get_all_service_names(agent_id)
        return [
            self.get_complete_service_info(agent_id, service_name)
            for service_name in service_names
        ]

    # ===  新增：便捷查询方法 ===

    def get_services_by_state(self, agent_id: str, states: List['ServiceConnectionState']) -> List[str]:
        """
        按状态筛选服务

        Args:
            states: [ServiceConnectionState.HEALTHY, ServiceConnectionState.WARNING]

        Returns:
            ["service1", "service2"]
        """
        services = []
        for service_name, state in self.service_states.get(agent_id, {}).items():
            if state in states:
                services.append(service_name)
        return services

    def get_healthy_services(self, agent_id: str) -> List[str]:
        """获取健康的服务列表"""
        from mcpstore.core.models.service import ServiceConnectionState
        return self.get_services_by_state(agent_id, [
            ServiceConnectionState.HEALTHY,
            ServiceConnectionState.WARNING
        ])

    def get_failed_services(self, agent_id: str) -> List[str]:
        """获取失败的服务列表"""
        from mcpstore.core.models.service import ServiceConnectionState
        return self.get_services_by_state(agent_id, [
            ServiceConnectionState.UNREACHABLE,
            ServiceConnectionState.DISCONNECTED
        ])

    def get_services_with_tools(self, agent_id: str) -> List[str]:
        """获取有工具的服务列表"""
        services_with_tools = []
        for service_name in self.get_all_service_names(agent_id):
            tools = self.get_tools_for_service(agent_id, service_name)
            if tools:
                services_with_tools.append(service_name)
        return services_with_tools

    # ===  新增：缓存同步管理 ===

    def sync_to_client_manager(self, client_manager):
        """将缓存数据同步到 ClientManager（简化版本）"""
        try:
            # 这里可以实现具体的同步逻辑
            # 目前作为占位符，实际同步由cache_manager处理
            logger.debug("[REGISTRY] sync_to_client_manager called")

        except Exception as e:
            logger.error(f"Failed to sync registry to ClientManager: {e}")
            raise

    #  [REFACTOR] 移除重复的方法定义 - 使用上面统一的方法

    def get_service_config_from_cache(self, agent_id: str, service_name: str) -> Optional[Dict[str, Any]]:
        """从缓存获取服务配置（缓存优先架构的核心方法）"""
        metadata = self.get_service_metadata(agent_id, service_name)
        if metadata and metadata.service_config:
            return metadata.service_config

        # 如果缓存中没有配置，说明系统有问题，应该报错
        logger.error(f"Service configuration not found in cache for {service_name} in agent {agent_id}")
        logger.error("This indicates a system issue - all services should have config in cache")
        return None

    # === Hot-Swapping Backend Methods ===

    async def switch_backend(self, new_backend: 'AsyncKeyValue') -> None:
        """
        Runtime backend switching with automatic data migration.
        
        This method implements hot-swapping of the cache backend, allowing
        the system to switch from MemoryStore to RedisStore (or vice versa)
        without losing data.
        
        Args:
            new_backend: New py-key-value backend to switch to
            
        Process:
            1. Export all data from old backend
            2. Switch backend reference
            3. Import data to new backend
            4. Verify data integrity
            5. Rollback on failure
            
        Limitations:
            - Session data is NOT migrated (always stays in memory)
            - Brief write blocking during switch
            
        Validates:
            - Requirements 12.1: 运行时后端切换
            - Requirements 12.2: 数据迁移机制
            - Requirements 12.3: Session 数据特殊处理
            
        Example:
            >>> # Switch from Memory to Redis
            >>> redis_store = RedisStore(url="redis://localhost:6379/0")
            >>> await registry.switch_backend(redis_store)
            
            >>> # Switch from Redis to Memory
            >>> memory_store = MemoryStore()
            >>> await registry.switch_backend(memory_store)
        """
        logger.info(
            f"[HOT_SWAP] Starting backend switch from {type(self._kv_store).__name__} to {type(new_backend).__name__}")

        # 1. Export all data from old backend
        old_backend = self._kv_store

        try:
            logger.info("[HOT_SWAP] Step 1: Exporting data from old backend...")
            exported_data = await self._export_all_data()
            logger.info(f"[HOT_SWAP] Exported {len(exported_data)} agents' data")

            # 2. Switch backend reference
            logger.info("[HOT_SWAP] Step 2: Switching backend reference...")
            self._kv_store = new_backend

            # 3. Import data to new backend
            logger.info("[HOT_SWAP] Step 3: Importing data to new backend...")
            await self._import_all_data(exported_data)
            logger.info("[HOT_SWAP] Data import completed")

            # 4. Verify data integrity
            logger.info("[HOT_SWAP] Step 4: Verifying data integrity...")
            await self._verify_data_integrity(exported_data)
            logger.info("[HOT_SWAP] Data integrity verified")

            logger.info(f"[HOT_SWAP] Backend switched successfully to {type(new_backend).__name__}")

        except Exception as e:
            # 5. Rollback to old backend on failure
            logger.error(f"[HOT_SWAP] Backend switch failed: {e}")
            logger.info("[HOT_SWAP] Rolling back to old backend...")
            self._kv_store = old_backend
            logger.info("[HOT_SWAP] Rollback completed")
            raise RuntimeError(f"Backend switch failed and rolled back: {e}")

    async def _export_all_data(self) -> Dict[str, Any]:
        """
        Export all cached data (excluding Sessions) from current backend.
        
        This method exports all data types from py-key-value storage:
        - Tool cache
        - Service states
        - Service metadata
        - Tool-to-service mappings
        - Agent-client mappings (from in-memory cache)
        - Client configs (from in-memory cache)
        
        Returns:
            Dict mapping agent_id to their data:
            {
                "agent_001": {
                    "tools": {"tool1": {...}, "tool2": {...}},
                    "states": {"service1": "HEALTHY", ...},
                    "metadata": {"service1": {...}, ...},
                    "mappings": {"tool1": "service1", ...}
                },
                ...
            }
            
        Note:
            - Sessions are NOT exported (not serializable)
            - Uses batch operations for efficiency
            
        Validates:
            - Requirements 12.2: 数据导出/导入辅助方法
        """
        logger.debug("[EXPORT] Starting data export...")
        exported = {}

        # Get all agent IDs from in-memory structures
        # We need to check multiple sources to get all agents
        agent_ids = set()
        agent_ids.update(self.tool_cache.keys())
        agent_ids.update(self.service_states.keys())
        agent_ids.update(self.service_metadata.keys())
        agent_ids.update(self.tool_to_service.keys())
        agent_ids.update(self.agent_clients.keys())

        logger.debug(f"[EXPORT] Found {len(agent_ids)} agents to export")

        for agent_id in agent_ids:
            agent_data = {}

            # Export tool cache
            try:
                tools_collection = self._get_collection(agent_id, "tools")
                agent_data["tools"] = await self._export_collection(tools_collection)
                logger.debug(f"[EXPORT] Agent {agent_id}: exported {len(agent_data['tools'])} tools")
            except Exception as e:
                logger.warning(f"[EXPORT] Failed to export tools for {agent_id}: {e}")
                agent_data["tools"] = {}

            # Export service states
            try:
                states_collection = self._get_collection(agent_id, "states")
                agent_data["states"] = await self._export_collection(states_collection)
                logger.debug(f"[EXPORT] Agent {agent_id}: exported {len(agent_data['states'])} states")
            except Exception as e:
                logger.warning(f"[EXPORT] Failed to export states for {agent_id}: {e}")
                agent_data["states"] = {}

            # Export service metadata
            try:
                metadata_collection = self._get_collection(agent_id, "metadata")
                agent_data["metadata"] = await self._export_collection(metadata_collection)
                logger.debug(f"[EXPORT] Agent {agent_id}: exported {len(agent_data['metadata'])} metadata")
            except Exception as e:
                logger.warning(f"[EXPORT] Failed to export metadata for {agent_id}: {e}")
                agent_data["metadata"] = {}

            # Export tool-to-service mappings
            try:
                mappings_collection = self._get_collection(agent_id, "mappings")
                agent_data["mappings"] = await self._export_collection(mappings_collection)
                logger.debug(f"[EXPORT] Agent {agent_id}: exported {len(agent_data['mappings'])} mappings")
            except Exception as e:
                logger.warning(f"[EXPORT] Failed to export mappings for {agent_id}: {e}")
                agent_data["mappings"] = {}

            exported[agent_id] = agent_data

        # Also export in-memory structures that aren't in py-key-value
        # (These are needed for complete state restoration)
        exported["_meta"] = {
            "agent_clients": dict(self.agent_clients),
            "client_configs": dict(self.client_configs),
            "service_to_client": dict(self.service_to_client),
            "agent_to_global_mappings": dict(self.agent_to_global_mappings),
            "global_to_agent_mappings": dict(self.global_to_agent_mappings),
            "long_lived_connections": list(self.long_lived_connections)
        }

        logger.info(
            f"[EXPORT] Export completed: {len(exported) - 1} agents, {sum(len(d.get('tools', {})) for d in exported.values() if isinstance(d, dict) and 'tools' in d)} total tools")
        return exported

    async def _export_collection(self, collection: str) -> Dict[str, Any]:
        """
        Export all key-value pairs from a collection.
        
        Args:
            collection: Collection name to export
            
        Returns:
            Dict mapping keys to values in the collection
        """
        try:
            # Use batch operations if available
            if hasattr(self._kv_store, 'keys') and hasattr(self._kv_store, 'get_many'):
                keys = await self._kv_store.keys(collection=collection)
                if not keys:
                    return {}
                values = await self._kv_store.get_many(keys, collection=collection)
                return dict(zip(keys, values))
            else:
                # Fallback: not supported, return empty
                logger.warning(f"[EXPORT] Store does not support batch operations for collection {collection}")
                return {}
        except Exception as e:
            logger.error(f"[EXPORT] Failed to export collection {collection}: {e}")
            return {}

    async def _import_all_data(self, data: Dict[str, Any]) -> None:
        """
        Import all data to the new backend.
        
        Args:
            data: Exported data structure from _export_all_data()
            
        Process:
            - Imports all data types to py-key-value
            - Restores in-memory structures
            - Uses batch operations for efficiency
            
        Validates:
            - Requirements 12.2: 数据导出/导入辅助方法
        """
        logger.debug("[IMPORT] Starting data import...")

        # Import meta data first (in-memory structures)
        if "_meta" in data:
            meta = data["_meta"]
            self.agent_clients = dict(meta.get("agent_clients", {}))
            self.client_configs = dict(meta.get("client_configs", {}))
            self.service_to_client = dict(meta.get("service_to_client", {}))
            self.agent_to_global_mappings = dict(meta.get("agent_to_global_mappings", {}))
            self.global_to_agent_mappings = dict(meta.get("global_to_agent_mappings", {}))
            self.long_lived_connections = set(meta.get("long_lived_connections", []))
            logger.debug("[IMPORT] Restored in-memory structures")

        # Import agent data
        agent_count = 0
        for agent_id, agent_data in data.items():
            if agent_id == "_meta":
                continue

            if not isinstance(agent_data, dict):
                continue

            # Import tools
            if "tools" in agent_data and agent_data["tools"]:
                tools_collection = self._get_collection(agent_id, "tools")
                await self._import_collection(tools_collection, agent_data["tools"])
                # Also update in-memory cache
                if agent_id not in self.tool_cache:
                    self.tool_cache[agent_id] = {}
                self.tool_cache[agent_id].update(agent_data["tools"])
                logger.debug(f"[IMPORT] Agent {agent_id}: imported {len(agent_data['tools'])} tools")

            # Import states
            if "states" in agent_data and agent_data["states"]:
                states_collection = self._get_collection(agent_id, "states")
                await self._import_collection(states_collection, agent_data["states"])
                # Also update in-memory cache
                if agent_id not in self.service_states:
                    self.service_states[agent_id] = {}
                for service_name, state_data in agent_data["states"].items():
                    if isinstance(state_data, dict):
                        state_value = state_data.get("state")
                    else:
                        state_value = state_data
                    if isinstance(state_value, str):
                        self.service_states[agent_id][service_name] = ServiceConnectionState(state_value)
                logger.debug(f"[IMPORT] Agent {agent_id}: imported {len(agent_data['states'])} states")

            # Import metadata
            if "metadata" in agent_data and agent_data["metadata"]:
                metadata_collection = self._get_collection(agent_id, "metadata")
                await self._import_collection(metadata_collection, agent_data["metadata"])
                # Also update in-memory cache
                if agent_id not in self.service_metadata:
                    self.service_metadata[agent_id] = {}
                for service_name, metadata_data in agent_data["metadata"].items():
                    if isinstance(metadata_data, dict):
                        # Reconstruct ServiceStateMetadata
                        from datetime import datetime
                        state_entered_time = metadata_data.get("state_entered_time")
                        if isinstance(state_entered_time, str):
                            state_entered_time = datetime.fromisoformat(state_entered_time)
                        last_ping_time = metadata_data.get("last_ping_time")
                        if isinstance(last_ping_time, str):
                            last_ping_time = datetime.fromisoformat(last_ping_time)
                        self.service_metadata[agent_id][service_name] = ServiceStateMetadata(
                            service_name=metadata_data.get("service_name", service_name),
                            agent_id=metadata_data.get("agent_id", agent_id),
                            state_entered_time=state_entered_time or datetime.now(),
                            service_config=metadata_data.get("service_config", {}),
                            consecutive_failures=metadata_data.get("consecutive_failures", 0),
                            error_message=metadata_data.get("error_message"),
                            last_ping_time=last_ping_time
                        )
                logger.debug(f"[IMPORT] Agent {agent_id}: imported {len(agent_data['metadata'])} metadata")

            # Import mappings
            if "mappings" in agent_data and agent_data["mappings"]:
                mappings_collection = self._get_collection(agent_id, "mappings")
                await self._import_collection(mappings_collection, agent_data["mappings"])
                # Also update in-memory cache
                if agent_id not in self.tool_to_service:
                    self.tool_to_service[agent_id] = {}
                self.tool_to_service[agent_id].update(agent_data["mappings"])
                logger.debug(f"[IMPORT] Agent {agent_id}: imported {len(agent_data['mappings'])} mappings")

            agent_count += 1

        logger.info(f"[IMPORT] Import completed: {agent_count} agents")

    async def _import_collection(self, collection: str, data: Dict[str, Any]) -> None:
        """
        Import key-value pairs to a collection.
        
        Args:
            collection: Collection name to import to
            data: Dict mapping keys to values
        """
        try:
            # Use batch operations if available
            if hasattr(self._kv_store, 'put_many'):
                keys = list(data.keys())
                values = list(data.values())
                await self._kv_store.put_many(keys, values, collection=collection)
            else:
                # Fallback: put one by one
                for key, value in data.items():
                    await self._kv_store.put(key, value, collection=collection)
        except Exception as e:
            logger.error(f"[IMPORT] Failed to import collection {collection}: {e}")
            raise

    async def _verify_data_integrity(self, exported_data: Dict[str, Any]) -> None:
        """
        Verify that imported data matches exported data.
        
        Args:
            exported_data: Original exported data to compare against
            
        Raises:
            RuntimeError: If data integrity check fails
            
        Validates:
            - Requirements 12.2: 验证数据完整性
        """
        logger.debug("[VERIFY] Starting data integrity verification...")

        errors = []

        for agent_id, agent_data in exported_data.items():
            if agent_id == "_meta":
                continue

            if not isinstance(agent_data, dict):
                continue

            # Verify tools
            if "tools" in agent_data:
                tools_collection = self._get_collection(agent_id, "tools")
                imported_tools = await self._export_collection(tools_collection)

                # Check counts
                if len(imported_tools) != len(agent_data["tools"]):
                    errors.append(
                        f"Agent {agent_id}: tool count mismatch (expected {len(agent_data['tools'])}, got {len(imported_tools)})")

                # Check keys
                missing_keys = set(agent_data["tools"].keys()) - set(imported_tools.keys())
                if missing_keys:
                    errors.append(f"Agent {agent_id}: missing tools: {missing_keys}")

            # Verify states
            if "states" in agent_data:
                states_collection = self._get_collection(agent_id, "states")
                imported_states = await self._export_collection(states_collection)

                if len(imported_states) != len(agent_data["states"]):
                    errors.append(
                        f"Agent {agent_id}: state count mismatch (expected {len(agent_data['states'])}, got {len(imported_states)})")

            # Verify metadata
            if "metadata" in agent_data:
                metadata_collection = self._get_collection(agent_id, "metadata")
                imported_metadata = await self._export_collection(metadata_collection)

                if len(imported_metadata) != len(agent_data["metadata"]):
                    errors.append(
                        f"Agent {agent_id}: metadata count mismatch (expected {len(agent_data['metadata'])}, got {len(imported_metadata)})")

            # Verify mappings
            if "mappings" in agent_data:
                mappings_collection = self._get_collection(agent_id, "mappings")
                imported_mappings = await self._export_collection(mappings_collection)

                if len(imported_mappings) != len(agent_data["mappings"]):
                    errors.append(
                        f"Agent {agent_id}: mapping count mismatch (expected {len(agent_data['mappings'])}, got {len(imported_mappings)})")

        if errors:
            error_msg = "\n".join(errors)
            logger.error(f"[VERIFY] Data integrity check failed:\n{error_msg}")
            raise RuntimeError(f"Data integrity verification failed:\n{error_msg}")

        logger.info("[VERIFY] Data integrity verified successfully")
