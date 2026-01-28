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
        enable_event_history: bool = False,
        event_store=None,
        event_syncer=None,
        health_enabled: bool = True,
        reconnection_enabled: bool = True,
        enable_event_log: bool = True,
        is_only_db: bool = False,
        event_lease_ttl: float = 30.0,
    ):
        self._registry = registry
        self._agent_locks = agent_locks
        self._config_manager = config_manager
        self._config_processor = config_processor
        self._local_service_manager = local_service_manager
        self._global_agent_store_id = global_agent_store_id
        self._event_store = event_store
        self._event_syncer = event_syncer
        self._health_enabled = health_enabled
        self._reconnection_enabled = reconnection_enabled
        self._enable_event_log = enable_event_log
        self._is_only_db = is_only_db
        self._event_lease_ttl = event_lease_ttl
        
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
            config_manager=self._config_manager,
            enable_file_persistence=False,
        )

        self._health_monitor = HealthMonitor(
            event_bus=self._event_bus,
            registry=self._registry,
            lifecycle_config=lifecycle_config,
            global_agent_store_id=self._global_agent_store_id,
            event_store=self._event_store,
            enable_event_log=self._enable_event_log,
            is_only_db=self._is_only_db,
            lease_ttl=self._event_lease_ttl,
        )

        # 创建重连调度器（使用相同的生命周期配置）
        self._reconnection_scheduler = ReconnectionScheduler(
            event_bus=self._event_bus,
            registry=self._registry,
            lifecycle_config=lifecycle_config,
            scan_interval=1.0,  # 扫描间隔固定1秒
            event_store=self._event_store,
            enable_event_log=self._enable_event_log,
            is_only_db=self._is_only_db,
            lease_ttl=self._event_lease_ttl,
        )

        # 创建应用服务
        self._service_app_service = ServiceApplicationService(
            event_bus=self._event_bus,
            registry=self._registry,
            lifecycle_manager=self._lifecycle_manager,
            global_agent_store_id=self._global_agent_store_id,
            event_store=self._event_store,
            enable_event_log=self._enable_event_log,
            is_only_db=self._is_only_db,
        )

        # 事件请求监听（控制平面执行）
        from mcpstore.core.events.service_events import ServiceRestartRequested, ServiceResetRequested

        async def _on_restart_requested(event):
            try:
                await self._service_app_service.restart_service(
                    service_name=event.service_name,
                    agent_id=event.agent_id,
                    wait_timeout=0.0,
                    from_event=True,
                    source=event.source,
                )
            except Exception as e:
                logger.error(f"[EVENT] ServiceRestartRequested handling failed: {e}", exc_info=True)

        self._event_bus.subscribe(ServiceRestartRequested, _on_restart_requested, priority=90)

        async def _on_reset_requested(event):
            try:
                await self._service_app_service.reset_service(
                    agent_id=event.agent_id,
                    service_name=event.service_name,
                    wait_timeout=0.0,
                    from_event=True,
                    source=event.source,
                )
            except Exception as e:
                logger.error(f"[EVENT] ServiceResetRequested handling failed: {e}", exc_info=True)

        self._event_bus.subscribe(ServiceResetRequested, _on_reset_requested, priority=90)

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
            ServiceReady,
            ToolSyncStarted,
            ToolSyncCompleted,
        )
        self._event_bus.subscribe(ServicePersisting, _on_service_persisting, priority=1)
        # ServicePersisted 已停用文件持久化，不再订阅
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

        if self._event_syncer:
            try:
                await self._event_syncer.start()
                logger.info("EventSyncer started")
            except Exception as sync_err:
                logger.error(f"EventSyncer start failed: {sync_err}", exc_info=True)

        # 启动健康监控
        if self._health_enabled:
            await self._health_monitor.start()
        else:
            logger.info("HealthMonitor disabled (mode=only_db)")

        # 启动重连调度器
        if self._reconnection_enabled:
            await self._reconnection_scheduler.start()
        else:
            logger.info("ReconnectionScheduler disabled (mode=only_db)")

        logger.info("ServiceContainer components started")

    async def stop(self):
        """停止所有组件"""
        logger.info("Stopping ServiceContainer components...")

        if self._event_syncer:
            try:
                await self._event_syncer.stop()
            except Exception:
                pass

        # 停止健康监控
        if self._health_enabled:
            await self._health_monitor.stop()

        # 停止重连调度器
        if self._reconnection_enabled:
            await self._reconnection_scheduler.stop()

        logger.info("ServiceContainer components stopped")

    def attach_event_syncer(self, event_syncer):
        """在容器创建后注入事件同步器。"""
        self._event_syncer = event_syncer
        try:
            # 将 event_syncer 透传给应用服务，用于写后同步消费
            self._service_app_service._event_syncer = event_syncer
        except Exception:
            pass
