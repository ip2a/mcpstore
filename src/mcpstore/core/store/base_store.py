"""
基础 MCPStore 类
包含核心初始化逻辑和基础属性
"""

import logging
from typing import Dict

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
        # Link back so orchestrator can access store-level facilities (locks, config)
        try:
            setattr(self.orchestrator, 'store', self)
        except Exception:
            logger.debug("Orchestrator linking to store failed; proceeding without back-reference")
        #  修复：添加LocalServiceManager访问属性
        self.local_service_manager = orchestrator.local_service_manager
        self.session_manager = orchestrator.session_manager
        self.logger = logging.getLogger(__name__)

        # Tool recording configuration
        self.tool_record_max_file_size = tool_record_max_file_size
        self.tool_record_retention_days = tool_record_retention_days

        # Unified configuration manager
        self._unified_config = UnifiedConfigManager(
            mcp_config_path=config.json_path,
            client_services_path=None  # single-source mode: do not use shard files
        )

        self._context_cache: Dict[str, MCPStoreContext] = {}
        self._store_context = self._create_store_context()

        # Data space manager (optional, only set when using data spaces)
        self._data_space_manager = None

        #  新增：缓存管理器
        
        # 认证配置管理器
        from mcpstore.core.auth.manager import AuthConfigManager
        self._auth_config_manager = AuthConfigManager()
        
        # 市场管理器
        from mcpstore.core.market.manager import MarketManager
        self._market_manager = MarketManager()
        
        # 缓存管理器
        from mcpstore.core.registry.cache_manager import ServiceCacheManager, CacheTransactionManager
        self.cache_manager = ServiceCacheManager(self.registry, self.orchestrator.lifecycle_manager)
        self.transaction_manager = CacheTransactionManager(self.registry)

        # 写锁：per-agent 原子写区
        from mcpstore.core.registry.agent_locks import AgentLocks
        self.agent_locks = AgentLocks()

        #  新增：智能查询接口
        from mcpstore.core.registry.smart_query import SmartCacheQuery
        self.query = SmartCacheQuery(self.registry)

        # 🆕 事件驱动架构：初始化 ServiceContainer
        from mcpstore.core.infrastructure.container import ServiceContainer
        from mcpstore.core.configuration.config_processor import ConfigProcessor

        self.container = ServiceContainer(
            registry=self.registry,
            agent_locks=self.agent_locks,
            config_manager=self._unified_config,
            config_processor=ConfigProcessor,
            local_service_manager=self.local_service_manager,
            global_agent_store_id=self.client_manager.global_agent_store_id,
            enable_event_history=False  # 生产环境关闭事件历史
        )

        logger.info("ServiceContainer initialized with event-driven architecture")

    def _create_store_context(self) -> MCPStoreContext:
        """Create store-level context"""
        return MCPStoreContext(self)
