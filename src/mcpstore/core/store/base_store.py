"""
Base MCPStore class
Contains core initialization logic and basic properties
"""

import logging
import threading
from typing import Dict, Optional
from weakref import WeakValueDictionary

from mcpstore.config.json_config import MCPConfig
from mcpstore.core.configuration.unified_config import UnifiedConfigManager
from mcpstore.core.context import MCPStoreContext
from mcpstore.core.orchestrator import MCPOrchestrator

logger = logging.getLogger(__name__)


class BaseMCPStore:
    """
    MCPStore - Intelligent Agent Tool Service Store
    Base class containing core initialization and properties
    """
    
    def __init__(self, orchestrator: MCPOrchestrator, config: MCPConfig,
                 tool_record_max_file_size: int = 30, tool_record_retention_days: int = 7):
        self.orchestrator = orchestrator
        self.config = config
        self.registry = orchestrator.registry
        self.client_manager = orchestrator.client_manager

        # [FIX] Add LocalServiceManager access attribute
        self.local_service_manager = orchestrator.local_service_manager
        self.session_manager = orchestrator.session_manager
        self.logger = logging.getLogger(__name__)

        # Tool recording configuration
        self.tool_record_max_file_size = tool_record_max_file_size
        self.tool_record_retention_days = tool_record_retention_days

        # Unified configuration manager (pass instance reference)
        self._unified_config = UnifiedConfigManager(mcp_config=config)


        # Set unified config to registry for JSON persistence
        self.registry.set_unified_config(self._unified_config)

        self._context_cache: Dict[str, MCPStoreContext] = {}
        self._store_context = self._create_store_context()

        # AgentProxy caching system for unified agent access
        self._agent_proxy_cache: WeakValueDictionary[str, 'AgentProxy'] = WeakValueDictionary()
        self._agent_cache_lock: threading.RLock = threading.RLock()

        # Data space manager (optional, only set when using data spaces)
        self._data_space_manager = None

        # [NEW] Cache manager

        # Cache manager
        from mcpstore.core.registry.cache_manager import ServiceCacheManager, CacheTransactionManager
        self.cache_manager = ServiceCacheManager(self.registry, self.orchestrator.lifecycle_manager)
        self.transaction_manager = CacheTransactionManager(self.registry)

        # Write locks: per-agent atomic write areas
        from mcpstore.core.registry.agent_locks import AgentLocks
        self.agent_locks = AgentLocks()

        # [已删除] SmartCacheQuery 接口
        # 原因: 功能冗余，可通过 registry 直接实现

        # 事件驱动架构: 初始化 ServiceContainer
        from mcpstore.core.infrastructure.container import ServiceContainer
        from mcpstore.core.configuration.config_processor import ConfigProcessor

        self.container = ServiceContainer(
            registry=self.registry,
            agent_locks=self.agent_locks,
            config_manager=self._unified_config,
            config_processor=ConfigProcessor,
            local_service_manager=self.local_service_manager,
            global_agent_store_id=self.client_manager.global_agent_store_id,
            enable_event_history=False  # Disable event history in production
        )

        # ToolSetManager 已废弃，工具可用性统一使用 StateManager
        # 工具状态存储在状态层: default:state:service_status

        # [UNIFIED] Point orchestrator.lifecycle_manager to container's lifecycle_manager
        try:
            self.orchestrator.lifecycle_manager = self.container.lifecycle_manager
        except Exception as e:
            logger.debug(f"Link lifecycle_manager failed: {e}")

        # [UNIFIED] Initialize content_manager after lifecycle_manager is set
        try:
            from mcpstore.core.lifecycle.content_manager import ServiceContentManager
            self.orchestrator.content_manager = ServiceContentManager(self.orchestrator)
            logger.info("ServiceContentManager initialization successful")
        except Exception as e:
            logger.warning(f"ServiceContentManager initialization failed: {e}")

        # Break circular dependency: pass container and context_factory to orchestrator
        # instead of letting orchestrator hold store reference (must be after container initialization)
        orchestrator.container = self.container
        orchestrator._context_factory = lambda: self.for_store()
        # Ensure sync manager can reference store for batch registration path
        try:
            orchestrator.store = self
        except Exception:
            pass

        logger.info("ServiceContainer initialized with event-driven architecture")

    def _create_store_context(self) -> MCPStoreContext:
        """Create store-level context"""
        return MCPStoreContext(self)

    def _get_or_create_agent_proxy(self, context: MCPStoreContext, agent_id: str) -> 'AgentProxy':
        """
        Get or create AgentProxy with unified caching.

        This method ensures that the same agent_id always returns the same AgentProxy
        instance across the entire MCPStore instance, providing true object identity
        and consistent state management.

        Args:
            context: The MCPStoreContext to use for the agent
            agent_id: Unique identifier for the agent

        Returns:
            AgentProxy: Cached or newly created AgentProxy instance
        """
        with self._agent_cache_lock:
            # Try to get from cache first
            cached_proxy = self._agent_proxy_cache.get(agent_id)
            if cached_proxy is not None:
                return cached_proxy

            # Create new AgentProxy and cache it
            from mcpstore.core.context.agent_proxy import AgentProxy
            agent_proxy = AgentProxy(context, agent_id)
            self._agent_proxy_cache[agent_id] = agent_proxy

            return agent_proxy

    def _clear_agent_proxy_cache(self, agent_id: Optional[str] = None) -> None:
        """
        Clear AgentProxy cache.

        Args:
            agent_id: Specific agent ID to clear, or None to clear all
        """
        with self._agent_cache_lock:
            if agent_id is None:
                self._agent_proxy_cache.clear()
            else:
                self._agent_proxy_cache.pop(agent_id, None)

    def _get_agent_proxy_cache_stats(self) -> Dict[str, int]:
        """
        Get AgentProxy cache statistics.

        Returns:
            Dictionary containing cache statistics
        """
        with self._agent_cache_lock:
            return {
                'total_cached_agents': len(self._agent_proxy_cache),
                'cache_lock_acquired': 1  # Simple indicator that lock is working
            }

    async def cleanup(self):
        """
        Cleanup resources on shutdown.
        
        This method handles proper cleanup of Redis clients and health check tasks.
        It follows the lifecycle management rules:
        - Only close system-created Redis clients
        - Do not close user-provided Redis clients
        - Stop health check tasks gracefully
        """
        logger.info("Starting MCPStore cleanup...")
        
        # Stop health check task if running
        health_check_task = getattr(self, "_health_check_task", None)
        if health_check_task:
            try:
                await health_check_task.stop()
                logger.debug("Health check task stopped")
            except Exception as e:
                logger.warning(f"Error stopping health check task: {e}")
        
        # Close system-created Redis client (but not user-provided)
        system_redis_client = getattr(self, "_system_created_redis_client", None)
        if system_redis_client:
            try:
                await system_redis_client.close()
                logger.debug("System-created Redis client closed")
            except Exception as e:
                logger.warning(f"Error closing Redis client: {e}")
        
        # Do NOT close user-provided Redis client
        user_redis_client = getattr(self, "_user_provided_redis_client", None)
        if user_redis_client:
            logger.debug("User-provided Redis client not closed (managed by user)")
        
        logger.info("MCPStore cleanup completed")
