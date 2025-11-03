"""
依赖注入容器 - 管理所有组件的创建和依赖关系

职责:
1. 创建和管理所有组件的生命周期
2. 处理组件之间的依赖关系
3. 提供统一的访问接口
"""

import logging
from typing import TYPE_CHECKING

from mcpstore.core.application.service_application_service import ServiceApplicationService
from mcpstore.core.domain.cache_manager import CacheManager
from mcpstore.core.domain.connection_manager import ConnectionManager
from mcpstore.core.domain.health_monitor import HealthMonitor
from mcpstore.core.domain.lifecycle_manager import LifecycleManager
from mcpstore.core.domain.persistence_manager import PersistenceManager
from mcpstore.core.domain.reconnection_scheduler import ReconnectionScheduler
from mcpstore.core.events.event_bus import EventBus

if TYPE_CHECKING:
    from mcpstore.core.registry.core_registry import CoreRegistry
    from mcpstore.core.registry.agent_locks import AgentLocks
    from mcpstore.core.configuration.unified_config import UnifiedConfigManager
    from mcpstore.core.configuration.config_processor import ConfigProcessor
    from mcpstore.core.integration.local_service_adapter import LocalServiceManagerAdapter

logger = logging.getLogger(__name__)


class ServiceContainer:
    """
    服务容器 - 依赖注入容器
    
    负责创建和管理所有组件的生命周期
    """
    
    def __init__(
        self,
        registry: 'CoreRegistry',
        agent_locks: 'AgentLocks',
        config_manager: 'UnifiedConfigManager',
        config_processor: 'ConfigProcessor',
        local_service_manager: 'LocalServiceManagerAdapter',
        global_agent_store_id: str,
        enable_event_history: bool = False
    ):
        self._registry = registry
        self._agent_locks = agent_locks
        self._config_manager = config_manager
        self._config_processor = config_processor
        self._local_service_manager = local_service_manager
        self._global_agent_store_id = global_agent_store_id
        
        # 创建事件总线（核心）
        # 事件总线：启用可选的 handler 超时（安全兜底）
        self._event_bus = EventBus(enable_history=enable_event_history, handler_timeout=None)
        
        # 创建领域服务
        self._cache_manager = CacheManager(
            event_bus=self._event_bus,
            registry=self._registry,
            agent_locks=self._agent_locks
        )
        
        self._lifecycle_manager = LifecycleManager(
            event_bus=self._event_bus,
            registry=self._registry
        )
        
        self._connection_manager = ConnectionManager(
            event_bus=self._event_bus,
            registry=self._registry,
            config_processor=self._config_processor,
            local_service_manager=self._local_service_manager
        )
        
        self._persistence_manager = PersistenceManager(
            event_bus=self._event_bus,
            config_manager=self._config_manager
        )

        # 🆕 创建健康监控管理器（统一从 ServiceLifecycleConfig 读取配置）
        from mcpstore.core.lifecycle.config import ServiceLifecycleConfig
        lifecycle_config = ServiceLifecycleConfig()

        self._health_monitor = HealthMonitor(
            event_bus=self._event_bus,
            registry=self._registry,
            check_interval=lifecycle_config.normal_heartbeat_interval,
            timeout_threshold=lifecycle_config.initialization_timeout,
            ping_timeout=lifecycle_config.health_check_ping_timeout,
            warning_interval=lifecycle_config.warning_heartbeat_interval,
            global_agent_store_id=self._global_agent_store_id
        )

        # 🆕 创建重连调度器（统一从 ServiceLifecycleConfig 读取配置）
        self._reconnection_scheduler = ReconnectionScheduler(
            event_bus=self._event_bus,
            registry=self._registry,
            scan_interval=1.0,  # 扫描间隔固定1秒
            base_delay=lifecycle_config.base_reconnect_delay,
            max_delay=lifecycle_config.max_reconnect_delay,
            max_retries=lifecycle_config.max_reconnect_attempts
        )

        # 创建应用服务
        self._service_app_service = ServiceApplicationService(
            event_bus=self._event_bus,
            registry=self._registry,
            global_agent_store_id=self._global_agent_store_id
        )

        logger.info("ServiceContainer initialized with all components (including health monitor and reconnection scheduler)")
    
    @property
    def event_bus(self) -> EventBus:
        """获取事件总线"""
        return self._event_bus
    
    @property
    def service_application_service(self) -> ServiceApplicationService:
        """获取服务应用服务"""
        return self._service_app_service
    
    @property
    def cache_manager(self) -> CacheManager:
        """获取缓存管理器"""
        return self._cache_manager
    
    @property
    def lifecycle_manager(self) -> LifecycleManager:
        """获取生命周期管理器"""
        return self._lifecycle_manager
    
    @property
    def connection_manager(self) -> ConnectionManager:
        """获取连接管理器"""
        return self._connection_manager
    
    @property
    def persistence_manager(self) -> PersistenceManager:
        """获取持久化管理器"""
        return self._persistence_manager

    @property
    def health_monitor(self) -> HealthMonitor:
        """获取健康监控管理器"""
        return self._health_monitor

    @property
    def reconnection_scheduler(self) -> ReconnectionScheduler:
        """获取重连调度器"""
        return self._reconnection_scheduler

    async def start(self):
        """启动所有需要后台运行的组件"""
        logger.info("Starting ServiceContainer components...")

        # 启动健康监控
        await self._health_monitor.start()

        # 启动重连调度器
        await self._reconnection_scheduler.start()

        logger.info("ServiceContainer components started")

    async def stop(self):
        """停止所有组件"""
        logger.info("Stopping ServiceContainer components...")

        # 停止健康监控
        await self._health_monitor.stop()

        # 停止重连调度器
        await self._reconnection_scheduler.stop()

        logger.info("ServiceContainer components stopped")

