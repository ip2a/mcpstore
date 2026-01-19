"""
Lifecycle Manager - Responsible for service state management

Responsibilities:
1. Listen to ServiceCached events, initialize lifecycle state
2. Listen to ServiceConnected/ServiceConnectionFailed events, transition states
3. Publish ServiceStateChanged events
4. Manage state metadata
"""

import asyncio
import logging
import time
from datetime import datetime

from mcpstore.config.config_defaults import HealthCheckConfigDefaults
from mcpstore.core.events.event_bus import EventBus
from mcpstore.core.events.service_events import (
    ServiceCached,
    ServiceInitialized,
    ServiceConnected,
    ServiceConnectionFailed,
    ServiceStateChanged,
    ServiceBootstrapped,
    ServiceBootstrapFailed,
    ServiceReady,
)
from mcpstore.core.models.service import ServiceConnectionState, ServiceStateMetadata

logger = logging.getLogger(__name__)


class LifecycleManager:
    """
    Lifecycle Manager

    Responsibilities:
    1. Listen to ServiceCached events, initialize lifecycle state
    2. Listen to ServiceConnected/ServiceConnectionFailed events, transition states
    3. Publish ServiceStateChanged events
    4. Manage state metadata
    
    重要设计原则：
    - 使用 AgentLocks 保证与 CacheManager 的操作顺序一致
    - 只更新健康状态，不触碰工具状态（工具状态由 CacheManager 管理）
    """
    
    def __init__(
        self, 
        event_bus: EventBus, 
        registry: 'CoreRegistry', 
        lifecycle_config: 'ServiceLifecycleConfig' = None,
        agent_locks: 'AgentLocks' = None
    ):
        self._event_bus = event_bus
        self._registry = registry
        self._agent_locks = agent_locks
        
        # Configuration (thresholds/heartbeat intervals)
        if lifecycle_config is None:
            # 从 MCPStoreConfig 获取配置（有默认回退）
            from mcpstore.config.toml_config import get_lifecycle_config_with_defaults
            lifecycle_config = get_lifecycle_config_with_defaults()
            logger.info("LifecycleManager using lifecycle config defaults or loaded config")
        self._config = lifecycle_config
        defaults = HealthCheckConfigDefaults()
        self._warning_failure_threshold = getattr(
            lifecycle_config,
            "liveness_failure_threshold",
            getattr(lifecycle_config, "warning_failure_threshold", defaults.liveness_failure_threshold),
        )
        self._reconnecting_failure_threshold = getattr(
            lifecycle_config,
            "reconnecting_failure_threshold",
            getattr(lifecycle_config, "liveness_failure_threshold", defaults.liveness_failure_threshold + 1),
        )

        # Subscribe to events
        self._event_bus.subscribe(ServiceCached, self._on_service_cached, priority=90)
        self._event_bus.subscribe(ServiceConnected, self._on_service_connected, priority=40)
        self._event_bus.subscribe(ServiceConnectionFailed, self._on_service_connection_failed, priority=40)
        self._event_bus.subscribe(ServiceBootstrapped, self._on_service_bootstrapped, priority=70)
        self._event_bus.subscribe(ServiceBootstrapFailed, self._on_service_bootstrap_failed, priority=20)

        # [NEW] Subscribe to health check and timeout events
        from mcpstore.core.events.service_events import HealthCheckCompleted, ServiceTimeout, ReconnectionRequested
        self._event_bus.subscribe(HealthCheckCompleted, self._on_health_check_completed, priority=50)
        self._event_bus.subscribe(ServiceTimeout, self._on_service_timeout, priority=50)
        self._event_bus.subscribe(ReconnectionRequested, self._on_reconnection_requested, priority=30)

        logger.info("LifecycleManager initialized and subscribed to events")
        # 健康成功但无工具时的重连尝试上限，避免无工具服务产生无限循环
        self._max_tool_resync_attempts = 2
        # setup/bootstrap 链路的连接并发限制，避免启动时风暴
        self._bootstrap_semaphore = asyncio.Semaphore(5)

    async def _set_service_metadata_async(self, agent_id: str, service_name: str, metadata) -> None:
        """
        异步元数据设置：直接使用缓存层的异步API

        严格按照 Functional Core, Imperative Shell 原则：
        1. Imperative Shell: 直接使用异步API，避免同步/异步混用
        2. 通过正确的异步渠道进行元数据管理
        3. 避免复杂的线程池转换
        """
        try:
            # 直接使用缓存层的异步API设置元数据
            from mcpstore.core.cache.naming_service import NamingService
            global_name = NamingService.generate_service_global_name(service_name, agent_id)

            # 转换元数据为字典
            # ServiceStateMetadata 是 Pydantic BaseModel，使用 model_dump() 或 dict() 方法
            if hasattr(metadata, 'model_dump'):
                # Pydantic v2
                metadata_dict = metadata.model_dump(mode='json')
            elif hasattr(metadata, 'dict'):
                # Pydantic v1
                metadata_dict = metadata.dict()
            elif isinstance(metadata, dict):
                metadata_dict = metadata
            else:
                raise TypeError(
                    f"metadata must be a dict or Pydantic BaseModel, got: {type(metadata).__name__}"
                )

            # 通过 CacheLayerManager 异步保存元数据
            # 注意：必须使用 _cache_layer_manager，而不是 _cache_layer
            # _cache_layer 在 Redis 模式下是 RedisStore，没有 put_state 方法
            await self._registry._cache_layer_manager.put_state("service_metadata", global_name, metadata_dict)

            logger.debug(f"[LIFECYCLE] Service metadata set successfully: {global_name}")
        except Exception as e:
            logger.error(f"[LIFECYCLE] Failed to set service metadata for {agent_id}:{service_name}: {e}")
            raise RuntimeError(
                f"Failed to set service metadata: agent_id={agent_id}, service_name={service_name}, error={e}"
            ) from e

    async def _set_service_state_async(self, agent_id: str, service_name: str, state) -> None:
        """
        异步状态设置：只更新健康状态，不触碰工具状态
        
        重要设计原则（方案 C）：
        - LifecycleManager 只负责管理健康状态
        - 工具状态由 CacheManager 独占管理
        - 避免竞态条件导致工具状态被覆盖

        严格按照 Functional Core, Imperative Shell 原则：
        1. Imperative Shell: 直接使用异步API，避免同步/异步混用
        2. 通过正确的异步渠道进行状态管理
        3. 避免复杂的线程池转换
        """
        try:
            # 直接使用 StateManager 的异步API
            from mcpstore.core.cache.naming_service import NamingService
            global_name = NamingService.generate_service_global_name(service_name, agent_id)

            # 转换 ServiceConnectionState 为字符串
            if hasattr(state, 'value'):
                health_status = state.value
            else:
                health_status = str(state)

            # 使用缓存层状态管理器（cache/state_manager.py）
            cache_state_manager = getattr(self._registry, '_cache_state_manager', None)
            if cache_state_manager is None:
                raise RuntimeError(
                    "Cache layer StateManager not initialized. "
                    "Please ensure ServiceRegistry correctly initializes the _cache_state_manager attribute."
                )
            
            # 获取现有的服务状态
            existing_status = await cache_state_manager.get_service_status(global_name)
            
            if existing_status:
                # 关键修复（方案 C）：保留现有的工具状态，只更新健康状态
                # 工具状态由 CacheManager._update_service_status 独占管理
                tools_status = [
                    {
                        "tool_global_name": tool.tool_global_name,
                        "tool_original_name": tool.tool_original_name,
                        "status": tool.status
                    }
                    for tool in existing_status.tools
                ]
                
                await cache_state_manager.update_service_status(
                    global_name,
                    health_status,
                    tools_status
                )
                logger.debug(
                    f"[LIFECYCLE] Updated health status: {global_name} -> {health_status}, "
                    f"preserved tools count: {len(tools_status)}"
                )
            else:
                # 状态不存在时，不创建新状态
                # 状态应该由 CacheManager 在处理 ServiceConnected 事件时创建
                logger.warning(
                    f"[LIFECYCLE] Service state does not exist, skipping update: {global_name}. "
                    f"State will be created by CacheManager."
                )

        except Exception as e:
            logger.error(f"[LIFECYCLE] Failed to set service state for {agent_id}:{service_name}: {e}")
            raise
    
    async def _on_service_cached(self, event: ServiceCached):
        """
        Handle service cached event - initialize lifecycle state

        严格按照 Functional Core, Imperative Shell 原则：
        1. 纯异步操作，避免任何同步/异步混用
        2. 通过正确的异步API访问状态，而不是直接访问内部服务
        3. 确保事件发布和健康检查触发的可靠性
        """
        logger.info(f"[LIFECYCLE] Initializing lifecycle for: {event.service_name}")

        try:
            # 1. 纯异步检查现有元数据（遵循核心原则）
            existing_metadata = await self._registry.get_service_metadata_async(event.agent_id, event.service_name)

            service_config = None
            if existing_metadata and existing_metadata.service_config:
                # 保留现有配置信息
                service_config = existing_metadata.service_config
                logger.debug(f"[LIFECYCLE] Preserving existing service_config for: {event.service_name}")
            else:
                # 优先从服务实体获取配置，避免依赖外部 mcp.json
                try:
                    service_info = await self._registry.get_complete_service_info_async(event.agent_id, event.service_name)
                    if service_info and service_info.get("config"):
                        service_config = service_info["config"]
                        logger.debug(f"[LIFECYCLE] Loaded service_config from service entity for: {event.service_name}")
                except Exception as entity_error:
                    # 按要求：不兼容旧架构，直接抛出错误
                    raise RuntimeError(f"Unable to get service configuration from service entity: {event.service_name}") from entity_error

            # 2. 创建元数据（纯函数操作）
            metadata = ServiceStateMetadata(
                service_name=event.service_name,
                agent_id=event.agent_id,
                state_entered_time=datetime.now(),
                consecutive_failures=0,
                reconnect_attempts=0,
                next_retry_time=None,
                error_message=None,
                service_config=service_config
            )

            # 3. 通过正确的异步API保存元数据
            await self._set_service_metadata_async(event.agent_id, event.service_name, metadata)

            # 验证保存成功
            logger.debug(f"[LIFECYCLE] Metadata saved with config keys: {list(service_config.keys()) if service_config else 'None'}")
            logger.info(f"[LIFECYCLE] Lifecycle initialized: {event.service_name} -> STARTUP")

            # 4. 发布初始化完成事件（同步等待确保完成）
            initialized_event = ServiceInitialized(
                agent_id=event.agent_id,
                service_name=event.service_name,
                initial_state="startup"
            )
            await self._event_bus.publish(initialized_event, wait=True)
            logger.debug(f"[LIFECYCLE] ServiceInitialized event published for: {event.service_name}")

            # 5. 触发初始健康检查（关键修复：确保事件被正确发布）
            logger.info(f"[LIFECYCLE] Triggering initial health check for {event.service_name}")
            try:
                from mcpstore.core.events.service_events import HealthCheckRequested
                health_check_event = HealthCheckRequested(
                    agent_id=event.agent_id,
                    service_name=event.service_name,
                    check_type="initial"
                )
                await self._event_bus.publish(health_check_event, wait=False)
                logger.info(f"[LIFECYCLE] HealthCheckRequested event published for: {event.service_name}")
            except Exception as health_event_error:
                logger.error(f"[LIFECYCLE] Failed to publish HealthCheckRequested for {event.service_name}: {health_event_error}", exc_info=True)
                # 不抛出异常，允许生命周期初始化继续

        except Exception as e:
            logger.error(f"[LIFECYCLE] Failed to initialize lifecycle for {event.service_name}: {e}", exc_info=True)
            # 发布失败事件以便其他组件处理
            try:
                from mcpstore.core.events.service_events import ServiceOperationFailed
                error_event = ServiceOperationFailed(
                    agent_id=event.agent_id,
                    service_name=event.service_name,
                    operation="lifecycle_initialization",
                    error_message=str(e),
                    original_event=event
                )
                await self._event_bus.publish(error_event, wait=False)
            except Exception as publish_error:
                logger.error(f"[LIFECYCLE] Failed to publish error event for {event.service_name}: {publish_error}")

    async def _on_service_bootstrapped(self, event: ServiceBootstrapped):
        """
        处理 setup/bootstrap 重放完成事件：写入元数据并后台触发连接/健康收敛
        """
        logger.info(f"[LIFECYCLE] [BOOTSTRAP] Initializing lifecycle for: {event.service_name}")

        try:
            service_config = event.service_config or {}
            if not service_config:
                try:
                    service_info = await self._registry.get_complete_service_info_async(event.agent_id, event.service_name)
                    if service_info and service_info.get("config"):
                        service_config = service_info["config"]
                        logger.debug(f"[LIFECYCLE] [BOOTSTRAP] Loaded service_config from entity for: {event.service_name}")
                except Exception as entity_error:
                    logger.error(f"[LIFECYCLE] [BOOTSTRAP] Cannot load service_config: {entity_error}")
                    raise

            metadata = ServiceStateMetadata(
                service_name=event.service_name,
                agent_id=event.agent_id,
                state_entered_time=datetime.now(),
                consecutive_failures=0,
                reconnect_attempts=0,
                next_retry_time=None,
                error_message=None,
                service_config=service_config
            )

            await self._set_service_metadata_async(event.agent_id, event.service_name, metadata)
            await self._set_service_state_async(event.agent_id, event.service_name, ServiceConnectionState.STARTUP)

            async def _dispatch_connect_and_health():
                async with self._bootstrap_semaphore:
                    try:
                        init_event = ServiceInitialized(
                            agent_id=event.agent_id,
                            service_name=event.service_name,
                            initial_state="startup"
                        )
                        await self._event_bus.publish(init_event, wait=True)
                    except Exception as connect_err:
                        failed_event = ServiceBootstrapFailed(
                            agent_id=event.agent_id,
                            service_name=event.service_name,
                            error_message=str(connect_err),
                            source=event.source,
                            original_event=event
                        )
                        try:
                            await self._event_bus.publish(failed_event, wait=False)
                        except Exception:
                            logger.error(f"[LIFECYCLE] [BOOTSTRAP] Failed to publish ServiceBootstrapFailed for {event.service_name}")

            asyncio.create_task(_dispatch_connect_and_health())

        except Exception as e:
            logger.error(f"[LIFECYCLE] [BOOTSTRAP] Failed to initialize lifecycle for {event.service_name}: {e}", exc_info=True)
            try:
                failed_event = ServiceBootstrapFailed(
                    agent_id=event.agent_id,
                    service_name=event.service_name,
                    error_message=str(e),
                    source=event.source,
                    original_event=event
                )
                await self._event_bus.publish(failed_event, wait=False)
            except Exception as pub_err:
                logger.error(f"[LIFECYCLE] [BOOTSTRAP] Failed to publish ServiceBootstrapFailed: {pub_err}")

    async def _on_service_bootstrap_failed(self, event: ServiceBootstrapFailed):
        """记录 bootstrap 失败，保持非阻塞"""
        logger.error(f"[LIFECYCLE] [BOOTSTRAP] Service bootstrap failed: {event.service_name} error={event.error_message}")

    async def _on_service_connected(self, event: ServiceConnected):
        """
        Handle successful service connection - transition state to HEALTHY

        重要设计原则（方案 A + C）：
        - 使用 AgentLocks 保证与 CacheManager 的操作顺序一致
        - 只更新健康状态，不触碰工具状态

        严格按照 Functional Core, Imperative Shell 原则：
        1. 纯异步操作，使用正确的异步API
        2. 状态转换和元数据更新分离
        3. 错误处理和事件发布
        """
        logger.info(f"[LIFECYCLE] Service connected: {event.service_name}")

        try:
            # 方案 A：使用 AgentLocks 保证与 CacheManager 的操作顺序一致
            # CacheManager 先执行（priority=50），写入工具状态
            # LifecycleManager 后执行（priority=40），只更新健康状态
            if self._agent_locks:
                async with self._agent_locks.write(
                    event.agent_id, 
                    operation="lifecycle_on_service_connected"
                ):
                    await self._handle_service_connected_internal(event)
            else:
                # 没有锁时直接执行（向后兼容，但会记录警告）
                logger.warning(
                    f"[LIFECYCLE] AgentLocks not configured, potential race condition: {event.service_name}"
                )
                await self._handle_service_connected_internal(event)

        except Exception as e:
            logger.error(f"[LIFECYCLE] Failed to transition state for {event.service_name}: {e}", exc_info=True)
            # 发布错误事件
            try:
                from mcpstore.core.events.service_events import ServiceOperationFailed
                error_event = ServiceOperationFailed(
                    agent_id=event.agent_id,
                    service_name=event.service_name,
                    operation="state_transition",
                    error_message=str(e),
                    original_event=event
                )
                await self._event_bus.publish(error_event, wait=False)
            except Exception as publish_error:
                logger.error(f"[LIFECYCLE] Failed to publish error event for {event.service_name}: {publish_error}")

    async def _handle_service_connected_internal(self, event: ServiceConnected):
        """
        Handle service connection success internal logic (executed under lock protection)
        """
        # 1. 通过异步API转换状态到 HEALTHY
        # 方案 C：只更新健康状态，不触碰工具状态
        await self._set_service_state_async(
            agent_id=event.agent_id,
            service_name=event.service_name,
            state=ServiceConnectionState.HEALTHY
        )
        logger.debug(f"[LIFECYCLE] State transitioned to HEALTHY for: {event.service_name}")

        # 2. 获取并更新元数据（异步操作）
        metadata = await self._registry.get_service_metadata_async(event.agent_id, event.service_name)
        if metadata:
            # 更新失败计数和连接信息（纯函数操作）
            metadata.consecutive_failures = 0
            metadata.reconnect_attempts = 0
            metadata.error_message = None
            metadata.last_health_check = datetime.now()
            metadata.last_response_time = event.connection_time
            # 记录工具同步信息，连接成功后工具被写入时重置重试计数
            tools_count = None
            try:
                global_name = self._registry._naming.generate_service_global_name(
                    event.service_name,
                    event.agent_id
                )
                tools = await self._registry._relation_manager.get_service_tools(global_name)
                tools_count = len(tools)
            except Exception as tool_err:
                logger.debug(f"[LIFECYCLE] Skip tool sync metadata update for {event.service_name}: {tool_err}")

            # 无论工具获取是否成功，连接成功后重置工具重试计数并清空空工具标记
            metadata.tool_sync_attempts = 0
            metadata.tools_confirmed_empty = False
            if tools_count and tools_count > 0:
                metadata.last_tool_sync = datetime.now()

            # 保存更新后的元数据（异步API）
            await self._set_service_metadata_async(event.agent_id, event.service_name, metadata)
            logger.debug(f"[LIFECYCLE] Metadata updated for connected service: {event.service_name}")
        else:
            raise RuntimeError(
                f"Service metadata does not exist, data inconsistency: "
                f"service_name={event.service_name}, agent_id={event.agent_id}. "
                f"Metadata should be created when handling ServiceCached event."
            )

        # 3. 发布状态转换事件
        try:
            from mcpstore.core.events.service_events import ServiceStateChanged
            state_changed_event = ServiceStateChanged(
                agent_id=event.agent_id,
                service_name=event.service_name,
                old_state="startup",
                new_state="healthy",
                reason="connection_successful"
            )
            await self._event_bus.publish(state_changed_event, wait=False)
            logger.debug(f"[LIFECYCLE] ServiceStateChanged event published for: {event.service_name}")
        except Exception as event_error:
            logger.error(f"[LIFECYCLE] Failed to publish state change event for {event.service_name}: {event_error}")

        # 发布就绪事件：工具状态与健康元数据已落盘
        try:
            current_state = await self._registry.get_service_state_async(event.agent_id, event.service_name)
            health_value = getattr(current_state, "value", str(current_state)) if current_state else "unknown"
            ready_event = ServiceReady(
                agent_id=event.agent_id,
                service_name=event.service_name,
                tool_count=tools_count or 0,
                health_status=health_value
            )
            if self._event_bus.get_subscriber_count(ServiceReady) > 0:
                await self._event_bus.publish(ready_event, wait=False)
                logger.debug(f"[LIFECYCLE] ServiceReady event published for: {event.service_name}")
            else:
                logger.debug(f"[LIFECYCLE] No subscribers for ServiceReady, skip publish for: {event.service_name}")
        except Exception as ready_err:
            logger.error(f"[LIFECYCLE] Failed to publish ServiceReady for {event.service_name}: {ready_err}")
    
    async def _on_service_connection_failed(self, event: ServiceConnectionFailed):
        """
        Handle service connection failure - update metadata but let health check manage state

        严格按照 Functional Core, Imperative Shell 原则：
        1. 纯异步操作，使用正确的异步API
        2. 只更新元数据，不直接转换状态
        3. 让 HealthMonitor 通过健康检查处理状态转换
        """
        logger.warning(f"[LIFECYCLE] Service connection failed: {event.service_name} ({event.error_message})")

        try:
            # 1. 通过异步API获取现有元数据
            metadata = None
            try:
                metadata = await self._registry.get_service_metadata_async(event.agent_id, event.service_name)
            except Exception as metadata_error:
                logger.warning(f"[LIFECYCLE] Failed to get metadata for {event.service_name}: {metadata_error}")
                metadata = None

            # 2. 更新失败信息（纯函数操作）
            if metadata:
                metadata.consecutive_failures += 1
                metadata.error_message = event.error_message
                metadata.last_failure_time = datetime.now()
                # 使用已有字段记录重连计数，避免写入不存在的属性
                metadata.reconnect_attempts = event.retry_count

                # 保存更新后的元数据（异步API）
                await self._set_service_metadata_async(event.agent_id, event.service_name, metadata)
                logger.info(f"[LIFECYCLE] Updated failure metadata for {event.service_name}: {metadata.consecutive_failures} failures, retry_count={event.retry_count}")
            else:
                logger.warning(f"[LIFECYCLE] No metadata found for {event.service_name}, skipping failure update")

            # 3. 明确记录：不立即转换状态，让 HealthMonitor 处理；仅在初始连接失败或达到阈值时进入 CIRCUIT_OPEN
            logger.info(f"[LIFECYCLE] Connection failure handled, deferring state transition to health monitor")

            try:
                current_state = await self._registry.get_service_state_async(event.agent_id, event.service_name)
            except Exception:
                current_state = None

            # 仅在初次连接或明确达到重连阈值时切换，避免单次抖动直接进入重连
            should_enter_reconnecting = False
            if current_state in (ServiceConnectionState.STARTUP, None):
                should_enter_reconnecting = True
            elif metadata and metadata.consecutive_failures >= self._reconnecting_failure_threshold:
                should_enter_reconnecting = current_state not in (
                    ServiceConnectionState.CIRCUIT_OPEN,
                    ServiceConnectionState.DISCONNECTED,
                    ServiceConnectionState.DISCONNECTED,
                )

            if should_enter_reconnecting:
                await self._transition_state(
                    agent_id=event.agent_id,
                    service_name=event.service_name,
                    new_state=ServiceConnectionState.CIRCUIT_OPEN,
                    reason="connection_failed",
                    source="LifecycleManager"
                )

            # 4. 发布连接失败事件（可能触发其他组件的处理）
            try:
                # 这个事件可以用于通知外部监控系统
                from mcpstore.core.events.service_events import ServiceOperationFailed
                failure_event = ServiceOperationFailed(
                    agent_id=event.agent_id,
                    service_name=event.service_name,
                    operation="connection",
                    error_message=event.error_message,
                    original_event=event
                )
                await self._event_bus.publish(failure_event, wait=False)
                logger.debug(f"[LIFECYCLE] ServiceOperationFailed event published for connection failure: {event.service_name}")
            except Exception as publish_error:
                logger.error(f"[LIFECYCLE] Failed to publish failure event for {event.service_name}: {publish_error}")

        except Exception as e:
            logger.error(f"[LIFECYCLE] Failed to handle connection failure for {event.service_name}: {e}", exc_info=True)

    async def _on_health_check_completed(self, event: 'HealthCheckCompleted'):
        """
        Handle health check completion - transition service state based on health status

        严格按照 Functional Core, Imperative Shell 原则：
        1. 纯异步操作，使用正确的异步API
        2. 状态转换逻辑清晰分离
        3. 遵循阈值配置进行状态管理
        """
        logger.debug(f"[LIFECYCLE] Health check completed: {event.service_name} (success={event.success})")

        try:
            # 1. 通过异步API获取现有元数据
            metadata = await self._registry.get_service_metadata_async(event.agent_id, event.service_name)
            if metadata:
                # 更新健康检查信息（纯函数操作）
                metadata.last_health_check = datetime.now()
                metadata.last_response_time = event.response_time
                if hasattr(metadata, "window_error_rate"):
                    metadata.window_error_rate = event.window_error_rate
                if hasattr(metadata, "latency_p95"):
                    metadata.latency_p95 = event.latency_p95
                if hasattr(metadata, "latency_p99"):
                    metadata.latency_p99 = event.latency_p99
                if hasattr(metadata, "sample_size"):
                    metadata.sample_size = event.sample_size
                # 退避/硬超时/租约时间戳写入
                if hasattr(metadata, "next_retry_time"):
                    if getattr(event, "next_retry_time", None) is not None:
                        try:
                            metadata.next_retry_time = datetime.fromtimestamp(event.next_retry_time)
                        except Exception:
                            metadata.next_retry_time = None
                    elif event.retry_in is not None:
                        metadata.next_retry_time = datetime.fromtimestamp(time.time() + event.retry_in)
                if hasattr(metadata, "hard_deadline"):
                    if getattr(event, "hard_deadline", None) is not None:
                        try:
                            metadata.hard_deadline = datetime.fromtimestamp(event.hard_deadline)
                        except Exception:
                            metadata.hard_deadline = None
                    elif event.hard_timeout_in is not None:
                        metadata.hard_deadline = datetime.fromtimestamp(time.time() + event.hard_timeout_in)
                if hasattr(metadata, "lease_deadline"):
                    if getattr(event, "lease_deadline", None) is not None:
                        try:
                            metadata.lease_deadline = datetime.fromtimestamp(event.lease_deadline)
                        except Exception:
                            metadata.lease_deadline = None
                    elif event.lease_remaining is not None:
                        metadata.lease_deadline = datetime.fromtimestamp(time.time() + event.lease_remaining)

                if event.success:
                    metadata.consecutive_failures = 0
                    metadata.error_message = None
                else:
                    metadata.consecutive_failures += 1
                    metadata.error_message = event.error_message

                # 保存更新后的元数据（异步API）
                await self._set_service_metadata_async(event.agent_id, event.service_name, metadata)
                logger.debug(f"[LIFECYCLE] Updated health check metadata for: {event.service_name}")

            # 2. 通过异步API获取当前状态
            current_state = await self._registry.get_service_state_async(event.agent_id, event.service_name)
            failures = 0
            if metadata:
                failures = metadata.consecutive_failures

            # Success: 收敛到 HEALTHY/READY 或从 HALF_OPEN 恢复
            if event.success:
                await self._maybe_trigger_tool_resync(event.agent_id, event.service_name)
                suggested_state = event.suggested_state
                if isinstance(suggested_state, str):
                    try:
                        suggested_state = ServiceConnectionState(suggested_state)
                    except ValueError:
                        suggested_state = None
                target_state = suggested_state or ServiceConnectionState.HEALTHY
                if target_state == ServiceConnectionState.HEALTHY and current_state != ServiceConnectionState.HEALTHY:
                    await self._transition_state(
                        agent_id=event.agent_id,
                        service_name=event.service_name,
                        new_state=ServiceConnectionState.HEALTHY,
                        reason="health_check_success",
                        source="HealthMonitor"
                    )
                elif target_state == ServiceConnectionState.READY and current_state != ServiceConnectionState.READY:
                    # 就绪门槛：工具或依赖加载完成后才允许进入 READY
                    gate_passed = await self._ready_gate_passed(event.agent_id, event.service_name, metadata)
                    if not gate_passed:
                        logger.info(
                            f"[LIFECYCLE] Ready gate not satisfied, stay in STARTUP: {event.service_name} "
                            f"(tools_confirmed_empty={getattr(metadata, 'tools_confirmed_empty', None)}, "
                            f"last_tool_sync={getattr(metadata, 'last_tool_sync', None)})"
                        )
                        return
                    await self._transition_state(
                        agent_id=event.agent_id,
                        service_name=event.service_name,
                        new_state=ServiceConnectionState.READY,
                        reason="readiness_success",
                        source="HealthMonitor"
                    )
                elif current_state == ServiceConnectionState.HALF_OPEN:
                    # 半开试探成功，恢复健康
                    await self._transition_state(
                        agent_id=event.agent_id,
                        service_name=event.service_name,
                        new_state=ServiceConnectionState.HEALTHY,
                        reason="half_open_success",
                        source="HealthMonitor"
                    )
                return

            # Failure: 根据建议状态迁移
            suggested = event.suggested_state
            if isinstance(suggested, str):
                try:
                    suggested = ServiceConnectionState(suggested)
                except ValueError:
                    suggested = None
            if suggested == ServiceConnectionState.CIRCUIT_OPEN and current_state != ServiceConnectionState.CIRCUIT_OPEN:
                # 半开失败或运行期熔断，写入退避计数与下一次重试
                if metadata:
                    if hasattr(metadata, "reconnect_attempts"):
                        metadata.reconnect_attempts += 1
                    if hasattr(metadata, "next_retry_time") and event.retry_in is not None:
                        metadata.next_retry_time = datetime.fromtimestamp(time.time() + event.retry_in)
                    await self._set_service_metadata_async(event.agent_id, event.service_name, metadata)
                await self._transition_state(
                    agent_id=event.agent_id,
                    service_name=event.service_name,
                    new_state=ServiceConnectionState.CIRCUIT_OPEN,
                    reason="health_check_suggest_circuit_open",
                    source="HealthMonitor"
                )
                return
            if suggested == ServiceConnectionState.DEGRADED and current_state == ServiceConnectionState.HEALTHY:
                await self._transition_state(
                    agent_id=event.agent_id,
                    service_name=event.service_name,
                    new_state=ServiceConnectionState.DEGRADED,
                    reason="health_check_suggest_degraded",
                    source="HealthMonitor"
                )
                return
            if suggested == ServiceConnectionState.DISCONNECTED:
                await self._transition_state(
                    agent_id=event.agent_id,
                    service_name=event.service_name,
                    new_state=ServiceConnectionState.DISCONNECTED,
                    reason="health_check_hard_timeout",
                    source="HealthMonitor"
                )
                return
            # 若无建议但连续失败存在，可降级
            if suggested is None and current_state == ServiceConnectionState.HEALTHY and failures > 0:
                await self._transition_state(
                    agent_id=event.agent_id,
                    service_name=event.service_name,
                    new_state=ServiceConnectionState.DEGRADED,
                    reason="health_check_failures",
                    source="HealthMonitor"
                )
                return

        except Exception as e:
            logger.error(f"[LIFECYCLE] Failed to handle health check result for {event.service_name}: {e}", exc_info=True)

    async def _on_service_timeout(self, event: 'ServiceTimeout'):
        """
        Handle service timeout - transition state to DISCONNECTED
        """
        logger.warning(
            f"[LIFECYCLE] Service timeout: {event.service_name} "
            f"(type={event.timeout_type}, elapsed={event.elapsed_time:.1f}s)"
        )

        try:
            # Update metadata through async API
            metadata = await self._registry.get_service_metadata_async(event.agent_id, event.service_name)
            if metadata:
                metadata.error_message = f"Timeout: {event.timeout_type} ({event.elapsed_time:.1f}s)"
                if hasattr(metadata, "hard_deadline"):
                    try:
                        # 标记为当前时间的硬超时，用于外部观测
                        metadata.hard_deadline = datetime.now()
                    except Exception:
                        pass
                if hasattr(metadata, "next_retry_time"):
                    metadata.next_retry_time = None
                await self._set_service_metadata_async(event.agent_id, event.service_name, metadata)

            # Transition to DISCONNECTED state
            await self._transition_state(
                agent_id=event.agent_id,
                service_name=event.service_name,
                new_state=ServiceConnectionState.DISCONNECTED,
                reason=f"timeout_{event.timeout_type}",
                source="HealthMonitor"
            )

        except Exception as e:
            logger.error(f"[LIFECYCLE] Failed to handle timeout for {event.service_name}: {e}", exc_info=True)

    async def _ready_gate_passed(
        self,
        agent_id: str,
        service_name: str,
        metadata: 'ServiceStateMetadata | None'
    ) -> bool:
        """
        就绪门槛：避免在工具/依赖未加载完毕时进入 READY
        """
        try:
            # 配置允许跳过时直接放行
            service_config = getattr(metadata, "service_config", {}) if metadata else {}
            if service_config.get("skip_ready_gate"):
                return True

            # 如果明确标记“工具为空且确认”，视为无需等待
            if metadata and getattr(metadata, "tools_confirmed_empty", False):
                return True

            # 检查工具同步完成时间，存在即认为已跑过同步
            if metadata and getattr(metadata, "last_tool_sync", None):
                return True

            # 读取状态层判断是否已有工具
            cache_state_manager = getattr(self._registry, "_cache_state_manager", None)
            if cache_state_manager:
                from mcpstore.core.cache.naming_service import NamingService
                global_name = NamingService.generate_service_global_name(service_name, agent_id)
                status = await cache_state_manager.get_service_status(global_name)
                if status and status.tools:
                    return True

            # 未满足任何条件，继续等待
            return False
        except Exception as e:
            logger.warning(f"[LIFECYCLE] Ready gate check failed, allow transition by default: {e}")
            return True

    async def _on_reconnection_requested(self, event: 'ReconnectionRequested'):
        """
        Handle reconnection request - log event (actual reconnection handled by ConnectionManager)
        """
        logger.info(
            f"[LIFECYCLE] Reconnection requested: {event.service_name} "
            f"(retry={event.retry_count}, reason={event.reason})"
        )

        # Update reconnection attempt count in metadata
        try:
            metadata = await self._registry.get_service_metadata_async(event.agent_id, event.service_name)
            if metadata:
                metadata.reconnect_attempts = event.retry_count
                await self._set_service_metadata_async(event.agent_id, event.service_name, metadata)
        except Exception as e:
            logger.error(f"[LIFECYCLE] Failed to update reconnection metadata: {e}")

    async def _maybe_trigger_tool_resync(self, agent_id: str, service_name: str) -> None:
        """
        健康检查成功后如果工具列表为空，受控触发一次重连以补齐工具。
        - 使用工具关系是否存在来判断
        - 增加尝试上限，避免无工具服务产生无限循环
        """
        try:
            global_name = self._registry._naming.generate_service_global_name(service_name, agent_id)
            # 正在初始化/重连时不再触发额外重连，避免并发重复连接
            current_state = await self._registry.get_service_state_async(agent_id, service_name)
            if current_state in (
                ServiceConnectionState.STARTUP,
                ServiceConnectionState.CIRCUIT_OPEN,
            ):
                logger.debug(f"[LIFECYCLE] Skip tool resync while state={getattr(current_state, 'value', current_state)} for {service_name}")
                return

            tools = await self._registry._relation_manager.get_service_tools(global_name)
            tools_count = len(tools)

            metadata = await self._registry.get_service_metadata_async(agent_id, service_name)
            if metadata is None:
                logger.warning(f"[LIFECYCLE] Missing metadata for {service_name}, skip tool resync to avoid loop")
                return

            # 有工具：重置计数并退出
            if tools_count > 0:
                metadata.tool_sync_attempts = 0
                metadata.tools_confirmed_empty = False
                metadata.last_tool_sync = datetime.now()
                await self._set_service_metadata_async(agent_id, service_name, metadata)
                return

            # 工具为空：判断是否需要重连
            attempts = metadata.tool_sync_attempts
            if metadata.tools_confirmed_empty:
                logger.debug(f"[LIFECYCLE] Tools already confirmed empty for {service_name}, skip resync")
                return

            if attempts >= self._max_tool_resync_attempts:
                metadata.tools_confirmed_empty = True
                await self._set_service_metadata_async(agent_id, service_name, metadata)
                logger.info(
                    f"[LIFECYCLE] Skip tool resync for {service_name}, attempts={attempts} reach limit"
                )
                return

            # 触发重连拉取工具
            metadata.tool_sync_attempts = attempts + 1
            await self._set_service_metadata_async(agent_id, service_name, metadata)

            from mcpstore.core.events.service_events import ReconnectionRequested
            recon_event = ReconnectionRequested(
                agent_id=agent_id,
                service_name=service_name,
                retry_count=0,
                reason="health_success_missing_tools"
            )
            await self._event_bus.publish(recon_event, wait=False)
            logger.info(
                f"[LIFECYCLE] Trigger tool resync via reconnection: {service_name}, attempts={attempts + 1}"
            )
        except Exception as e:
            logger.error(f"[LIFECYCLE] Tool resync decision failed for {service_name}: {e}", exc_info=True)
    
    async def initialize_service(self, agent_id: str, service_name: str, service_config: dict) -> bool:
        """
        初始化服务（异步版）

        - 在事件循环内直接 await，不再通过同步包装器绕行
        - 生成/复用 client_id 后发布 ServiceAddRequested 事件
        """
        try:
            logger.info(f"[LIFECYCLE] initialize_service called: agent={agent_id}, service={service_name}")
            logger.debug(f"[LIFECYCLE] Service config: {service_config}")

            from mcpstore.core.utils.id_generator import ClientIDGenerator
            client_id = ClientIDGenerator.generate_deterministic_id(
                agent_id=agent_id,
                service_name=service_name,
                service_config=service_config,
                global_agent_store_id=agent_id
            )
            logger.debug(f"[LIFECYCLE] Generated client_id: {client_id}")

            # 复用已存在映射（避免重复写入）
            try:
                existing_client_id = await self._registry._agent_client_service.get_service_client_id_async(agent_id, service_name)
                if existing_client_id:
                    logger.debug(f"[LIFECYCLE] Found existing client_id mapping: {existing_client_id}")
                    client_id = existing_client_id
            except Exception as map_err:
                logger.warning(f"[LIFECYCLE] Failed to fetch existing client_id mapping: {map_err}")

            from mcpstore.core.events.service_events import ServiceAddRequested

            event = ServiceAddRequested(
                agent_id=agent_id,
                service_name=service_name,
                service_config=service_config,
                client_id=client_id,
                source="lifecycle_manager",
                wait_timeout=0
            )

            logger.info(f"[LIFECYCLE] Publishing ServiceAddRequested event for {service_name}")
            await self._event_bus.publish(event, wait=True)
            logger.info(f"[LIFECYCLE] Service {service_name} initialization triggered successfully")
            return True

        except Exception as e:
            logger.error(f"[LIFECYCLE] Failed to initialize service {service_name}: {e}", exc_info=True)
            return False

    def initialize_service_sync(self, agent_id: str, service_name: str, service_config: dict) -> bool:
        """
        同步包装器：在无事件循环的场景使用，内部通过 bridge 执行异步逻辑。
        """
        try:
            from mcpstore.core.bridge import get_async_bridge
            bridge = get_async_bridge()
            return bridge.run(
                self.initialize_service(agent_id, service_name, service_config),
                op_name="LifecycleManager.initialize_service"
            )
        except Exception as e:
            logger.error(f"[LIFECYCLE] Failed to initialize service (sync wrapper) {service_name}: {e}", exc_info=True)
            return False
    
    async def graceful_disconnect(self, agent_id: str, service_name: str, reason: str = "user_requested"):
        """Gracefully disconnect service (does not modify config/registry entities, only lifecycle disconnection).

        - Set state to DISCONNECTED → DISCONNECTED
        - Record disconnect reason in metadata
        - Upper layer (optional) cleans up tool display cache
        """
        try:
            # Update disconnect reason
            metadata = await self._registry.get_service_metadata_async(agent_id, service_name)
            if metadata:
                try:
                    metadata.disconnect_reason = reason
                    await self._set_service_metadata_async(agent_id, service_name, metadata)
                except Exception:
                    pass

            # First enter DISCONNECTED
            await self._transition_state(
                agent_id=agent_id,
                service_name=service_name,
                new_state=ServiceConnectionState.DISCONNECTED,
                reason=reason,
                source="LifecycleManager"
            )

            # Immediately converge to DISCONNECTED (don't wait for external callback)
            await self._transition_state(
                agent_id=agent_id,
                service_name=service_name,
                new_state=ServiceConnectionState.DISCONNECTED,
                reason=reason,
                source="LifecycleManager"
            )
        except Exception as e:
            logger.error(f"[LIFECYCLE] graceful_disconnect failed for {service_name}: {e}", exc_info=True)
    
    async def _transition_state(
        self,
        agent_id: str,
        service_name: str,
        new_state: ServiceConnectionState,
        reason: str,
        source: str
    ):
        """
        Execute state transition (single entry point)
        """
        # Get current state (async interface)
        try:
            old_state = await self._registry.get_service_state_async(agent_id, service_name)
        except Exception as e:
            logger.error(f"[LIFECYCLE] Failed to get current state for {service_name}: {e}")
            old_state = None

        if old_state == new_state:
            logger.debug(f"[LIFECYCLE] State unchanged: {service_name} already in {new_state.value}")
            return
        
        logger.info(
            f"[LIFECYCLE] State transition: {service_name} "
            f"{old_state.value if old_state else 'None'} -> {new_state.value} "
            f"(reason={reason}, source={source})"
        )
        
        # Update state (async interface)
        await self._registry.set_service_state_async(agent_id, service_name, new_state)
        
        # Update metadata (async get from pykv)
        try:
            metadata = await self._registry.get_service_metadata_async(agent_id, service_name)

            if metadata:
                if hasattr(metadata, 'state_entered_time'):
                    metadata.state_entered_time = datetime.now()
                try:
                    await self._registry.set_service_metadata_async(agent_id, service_name, metadata)
                except Exception as e:
                    logger.error(f"[LIFECYCLE] Failed to update metadata for {service_name}: {e}")
                    raise
        except Exception as e:
            logger.error(f"[LIFECYCLE] Error updating metadata for {service_name}: {e}")
            raise
        
        # Publish state change event
        state_changed_event = ServiceStateChanged(
            agent_id=agent_id,
            service_name=service_name,
            old_state=old_state.value if old_state else "none",
            new_state=new_state.value,
            reason=reason,
            source=source
        )
        await self._event_bus.publish(state_changed_event)

    async def handle_health_check_result(
        self,
        agent_id: str,
        service_name: str,
        success: bool,
        response_time: float,
        error_message: str = None
    ) -> None:
        """
        Handle health check result from service connection attempt.

        This method is called by orchestrator when a service connection attempt
        completes, allowing the LifecycleManager to transition service state
        based on the connection result.

        Args:
            agent_id: Agent ID that owns the service
            service_name: Service name
            success: Whether the connection/health check succeeded
            response_time: Response time of the health check
            error_message: Error message if the check failed
        """
        logger.info(
            f"[LIFECYCLE] Handle health check result: {service_name} "
            f"(success={success}, response_time={response_time:.3f}s, error={error_message})"
        )

        try:
            logger.info(f"[LIFECYCLE] Starting state transition logic for {service_name}")
            # Update metadata
            try:
                logger.info(f"[LIFECYCLE] Registry type: {type(self._registry)}")
                logger.info(f"[LIFECYCLE] Found get_service_metadata method: {hasattr(self._registry, 'get_service_metadata')}")

                # 使用统一的异步API
                metadata = await self._registry.get_service_metadata_async(agent_id, service_name)

                logger.info(f"[LIFECYCLE] Retrieved metadata for {service_name}: {metadata is not None}")
            except Exception as e:
                logger.error(f"[LIFECYCLE] Failed to get metadata for {service_name}: {e}")
                metadata = None

            # 简化元数据处理
            try:
                if metadata:
                    # 更新现有元数据
                    logger.info(f"[LIFECYCLE] Updating existing metadata for {service_name}")
                    if hasattr(metadata, 'last_health_check'):
                        metadata.last_health_check = datetime.now()
                    if hasattr(metadata, 'last_response_time'):
                        metadata.last_response_time = response_time

                    if success:
                        if hasattr(metadata, 'consecutive_failures'):
                            metadata.consecutive_failures = 0
                        if hasattr(metadata, 'error_message'):
                            metadata.error_message = None
                    else:
                        if hasattr(metadata, 'consecutive_failures'):
                            metadata.consecutive_failures = getattr(metadata, 'consecutive_failures', 0) + 1
                        if hasattr(metadata, 'error_message'):
                            metadata.error_message = error_message

                    try:
                        self._registry.set_service_metadata(agent_id, service_name, metadata)
                        logger.info(f"[LIFECYCLE] Updated metadata for {service_name}")
                    except Exception as e:
                        logger.warning(f"[LIFECYCLE] Failed to update metadata for {service_name}: {e}")
                else:
                    logger.info(f"[LIFECYCLE] No existing metadata found for {service_name}")
            except Exception as e:
                logger.warning(f"[LIFECYCLE] Error processing metadata for {service_name}: {e}")

            # Get current state
            try:
                # 使用统一的异步API
                current_state = await self._registry.get_service_state_async(agent_id, service_name)

                logger.info(f"[LIFECYCLE] Current state for {service_name}: {current_state}")
            except Exception as e:
                logger.error(f"[LIFECYCLE] Failed to get current state for {service_name}: {e}")
                current_state = None

            # Transition state based on result
            if success:
                # Success: transition to HEALTHY from any state
                if current_state != ServiceConnectionState.HEALTHY:
                    await self._transition_state(
                        agent_id=agent_id,
                        service_name=service_name,
                        new_state=ServiceConnectionState.HEALTHY,
                        reason="connection_success",
                        source="Orchestrator"
                    )
                    logger.info(f"[LIFECYCLE] Service {service_name} transitioned to HEALTHY (connection success)")
                else:
                    logger.debug(f"[LIFECYCLE] Service {service_name} already HEALTHY")
            else:
                # Failure: if no current state, assume STARTUP and transition to CIRCUIT_OPEN
                if current_state is None:
                    logger.info(f"[LIFECYCLE] Assuming current state is STARTUP for {service_name}")
                    current_state = ServiceConnectionState.STARTUP
                # Failure: determine target state based on current state and failure count
                failure_count = metadata.consecutive_failures if metadata else 1

                if current_state == ServiceConnectionState.STARTUP:
                    # First connection failure -> CIRCUIT_OPEN
                    new_state = ServiceConnectionState.CIRCUIT_OPEN
                    reason = "initial_connection_failed"
                    logger.info(f"[LIFECYCLE] Will transition {service_name} from STARTUP to CIRCUIT_OPEN (first failure)")
                elif failure_count >= self._reconnecting_failure_threshold:
                    # High failure count -> DISCONNECTED
                    new_state = ServiceConnectionState.DISCONNECTED
                    reason = "connection_unreachable"
                elif failure_count >= self._warning_failure_threshold:
                    # Medium failure count -> DEGRADED
                    new_state = ServiceConnectionState.DEGRADED
                    reason = "connection_warning"
                else:
                    # Low failure count -> CIRCUIT_OPEN
                    new_state = ServiceConnectionState.CIRCUIT_OPEN
                    reason = "connection_failed"

                if current_state != new_state:
                    try:
                        logger.info(f"[LIFECYCLE] About to call _transition_state for {service_name}: {current_state.value} -> {new_state.value}")
                        await self._transition_state(
                            agent_id=agent_id,
                            service_name=service_name,
                            new_state=new_state,
                            reason=reason,
                            source="Orchestrator"
                        )
                        logger.info(
                            f"[LIFECYCLE] Service {service_name} transitioned to {new_state.value} "
                            f"(reason={reason}, failures={failure_count})"
                        )
                    except Exception as e:
                        logger.error(f"[LIFECYCLE] Failed to transition {service_name} to {new_state.value}: {e}")
                else:
                    logger.debug(f"[LIFECYCLE] Service {service_name} already in {new_state.value}")

        except Exception as e:
            logger.error(
                f"[LIFECYCLE] Failed to handle health check result for {service_name}: {e}",
                exc_info=True
            )
