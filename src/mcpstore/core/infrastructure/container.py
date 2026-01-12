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

        # 获取生命周期配置（自动处理异步上下文和fallback）
        from mcpstore.config.toml_config import get_lifecycle_config_with_defaults
        lifecycle_config = get_lifecycle_config_with_defaults()

        # 获取HTTP超时配置
        from mcpstore.config.config_defaults import StandaloneConfigDefaults
        http_timeout_seconds = float(StandaloneConfigDefaults().http_timeout_seconds)
        logger.debug(f"[CONTAINER] HTTP timeout configured: {http_timeout_seconds} seconds")

        self._lifecycle_manager = LifecycleManager(
            event_bus=self._event_bus,
            registry=self._registry,
            lifecycle_config=lifecycle_config,
            agent_locks=self._agent_locks
        )

        self._connection_manager = ConnectionManager(
            event_bus=self._event_bus,
            registry=self._registry,
            config_processor=self._config_processor,
            local_service_manager=self._local_service_manager,
            http_timeout_seconds=http_timeout_seconds
        )

        self._persistence_manager = PersistenceManager(
            event_bus=self._event_bus,
            config_manager=self._config_manager
        )

        self._health_monitor = HealthMonitor(
            event_bus=self._event_bus,
            registry=self._registry,
            lifecycle_config=lifecycle_config,
            global_agent_store_id=self._global_agent_store_id
        )

        # 创建重连调度器（使用相同的生命周期配置）
        self._reconnection_scheduler = ReconnectionScheduler(
            event_bus=self._event_bus,
            registry=self._registry,
            lifecycle_config=lifecycle_config,
            scan_interval=1.0,  # 扫描间隔固定1秒
        )

        # 创建应用服务
        self._service_app_service = ServiceApplicationService(
            event_bus=self._event_bus,
            registry=self._registry,
            lifecycle_manager=self._lifecycle_manager,
            global_agent_store_id=self._global_agent_store_id
        )

        # 事件诊断订阅：记录就绪/持久化阶段，便于调试与监控
        async def _on_service_persisting(event):
            logger.info(
                "[EVENT] ServicePersisting: agent=%s service=%s stage=%s tools=%s",
                event.agent_id, event.service_name, getattr(event, "stage", "cache"), getattr(event, "tool_count", 0)
            )

        async def _on_service_persisted(event):
            logger.info(
                "[EVENT] ServicePersisted: agent=%s service=%s stage=%s tools=%s",
                event.agent_id, event.service_name, getattr(event, "stage", "config"), getattr(event, "tool_count", 0)
            )

        async def _on_service_ready(event):
            logger.info(
                "[EVENT] ServiceReady: agent=%s service=%s health=%s tools=%s",
                event.agent_id, event.service_name, getattr(event, "health_status", ""), getattr(event, "tool_count", 0)
            )

        async def _on_tool_sync(event):
            logger.info(
                "[EVENT] ToolSync: agent=%s service=%s total=%s phase=%s",
                event.agent_id, event.service_name, getattr(event, "total_tools", 0),
                event.__class__.__name__
            )

        from mcpstore.core.events.service_events import (
            ServicePersisting,
            ServicePersisted,
            ServiceReady,
            ToolSyncStarted,
            ToolSyncCompleted,
        )
        self._event_bus.subscribe(ServicePersisting, _on_service_persisting, priority=1)
        self._event_bus.subscribe(ServicePersisted, _on_service_persisted, priority=1)
        self._event_bus.subscribe(ServiceReady, _on_service_ready, priority=1)
        self._event_bus.subscribe(ToolSyncStarted, _on_tool_sync, priority=1)
        self._event_bus.subscribe(ToolSyncCompleted, _on_tool_sync, priority=1)

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
