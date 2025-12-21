"""
Lifecycle Manager - Responsible for service state management

Responsibilities:
1. Listen to ServiceCached events, initialize lifecycle state
2. Listen to ServiceConnected/ServiceConnectionFailed events, transition states
3. Publish ServiceStateChanged events
4. Manage state metadata
"""

import logging
from datetime import datetime

from mcpstore.core.events.event_bus import EventBus
from mcpstore.core.events.service_events import (
    ServiceCached, ServiceInitialized, ServiceConnected,
    ServiceConnectionFailed, ServiceStateChanged
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
            logger.info(f"LifecycleManager using config from {'MCPStoreConfig' if hasattr(lifecycle_config, 'warning_failure_threshold') else 'defaults'}")
        self._config = lifecycle_config

        # Subscribe to events
        self._event_bus.subscribe(ServiceCached, self._on_service_cached, priority=90)
        self._event_bus.subscribe(ServiceConnected, self._on_service_connected, priority=40)
        self._event_bus.subscribe(ServiceConnectionFailed, self._on_service_connection_failed, priority=40)

        # [NEW] Subscribe to health check and timeout events
        from mcpstore.core.events.service_events import HealthCheckCompleted, ServiceTimeout, ReconnectionRequested
        self._event_bus.subscribe(HealthCheckCompleted, self._on_health_check_completed, priority=50)
        self._event_bus.subscribe(ServiceTimeout, self._on_service_timeout, priority=50)
        self._event_bus.subscribe(ReconnectionRequested, self._on_reconnection_requested, priority=30)

        logger.info("LifecycleManager initialized and subscribed to events")

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
                    f"metadata 必须是字典或 Pydantic BaseModel，实际类型: {type(metadata).__name__}"
                )

            # 通过 CacheLayerManager 异步保存元数据
            # 注意：必须使用 _cache_layer_manager，而不是 _cache_layer
            # _cache_layer 在 Redis 模式下是 RedisStore，没有 put_state 方法
            await self._registry._cache_layer_manager.put_state("service_metadata", global_name, metadata_dict)

            logger.debug(f"[LIFECYCLE] Service metadata set successfully: {global_name}")
        except Exception as e:
            logger.error(f"[LIFECYCLE] Failed to set service metadata for {agent_id}:{service_name}: {e}")
            raise RuntimeError(
                f"设置服务元数据失败: agent_id={agent_id}, service_name={service_name}, error={e}"
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
                    "缓存层 StateManager 未初始化。"
                    "请确保 ServiceRegistry 正确初始化了 _cache_state_manager 属性。"
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
                    f"[LIFECYCLE] 更新健康状态: {global_name} -> {health_status}, "
                    f"保留工具数量: {len(tools_status)}"
                )
            else:
                # 状态不存在时，不创建新状态
                # 状态应该由 CacheManager 在处理 ServiceConnected 事件时创建
                logger.warning(
                    f"[LIFECYCLE] 服务状态不存在，跳过更新: {global_name}. "
                    f"状态将由 CacheManager 创建。"
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
                # 从客户端配置读取（通过正确的异步API）
                try:
                    client_config = await self._registry.get_client_config_from_cache_async(event.client_id)
                    service_config = client_config.get("mcpServers", {}).get(event.service_name, {}) if client_config else {}
                    logger.debug(f"[LIFECYCLE] Loading service_config from client config for: {event.service_name}")
                except Exception as config_error:
                    logger.warning(f"[LIFECYCLE] Failed to load client config for {event.service_name}: {config_error}")
                    service_config = {}

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
            logger.info(f"[LIFECYCLE] Lifecycle initialized: {event.service_name} -> INITIALIZING")

            # 4. 发布初始化完成事件（同步等待确保完成）
            initialized_event = ServiceInitialized(
                agent_id=event.agent_id,
                service_name=event.service_name,
                initial_state="initializing"
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
                    f"[LIFECYCLE] AgentLocks 未配置，可能存在竞态条件: {event.service_name}"
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
        处理服务连接成功的内部逻辑（在锁保护下执行）
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

            # 保存更新后的元数据（异步API）
            await self._set_service_metadata_async(event.agent_id, event.service_name, metadata)
            logger.debug(f"[LIFECYCLE] Metadata updated for connected service: {event.service_name}")
        else:
            raise RuntimeError(
                f"服务元数据不存在，数据不一致: "
                f"service_name={event.service_name}, agent_id={event.agent_id}. "
                f"元数据应该在 ServiceCached 事件处理时创建。"
            )

        # 3. 发布状态转换事件
        try:
            from mcpstore.core.events.service_events import ServiceStateChanged
            state_changed_event = ServiceStateChanged(
                agent_id=event.agent_id,
                service_name=event.service_name,
                old_state="initializing",
                new_state="healthy",
                reason="connection_successful"
            )
            await self._event_bus.publish(state_changed_event, wait=False)
            logger.debug(f"[LIFECYCLE] ServiceStateChanged event published for: {event.service_name}")
        except Exception as event_error:
            logger.error(f"[LIFECYCLE] Failed to publish state change event for {event.service_name}: {event_error}")
    
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
                metadata.retry_count = event.retry_count

                # 保存更新后的元数据（异步API）
                await self._set_service_metadata_async(event.agent_id, event.service_name, metadata)
                logger.info(f"[LIFECYCLE] Updated failure metadata for {event.service_name}: {metadata.consecutive_failures} failures, retry_count={event.retry_count}")
            else:
                logger.warning(f"[LIFECYCLE] No metadata found for {event.service_name}, skipping failure update")

            # 3. 明确记录：不立即转换状态，让 HealthMonitor 处理
            logger.info(f"[LIFECYCLE] Connection failure handled, deferring state transition to health monitor")

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

            # Success: return from INITIALIZING/WARNING to HEALTHY; HEALTHY stays
            if event.success:
                if current_state in (ServiceConnectionState.INITIALIZING, ServiceConnectionState.WARNING):
                    await self._transition_state(
                        agent_id=event.agent_id,
                        service_name=event.service_name,
                        new_state=ServiceConnectionState.HEALTHY,
                        reason="health_check_success",
                        source="HealthMonitor"
                    )
                return

            # Failure: advance to WARNING/RECONNECTING based on thresholds
            warn_th = self._config.warning_failure_threshold
            rec_th = self._config.reconnecting_failure_threshold

            # Reached reconnection threshold: enter RECONNECTING
            if failures >= rec_th:
                if current_state != ServiceConnectionState.RECONNECTING:
                    await self._transition_state(
                        agent_id=event.agent_id,
                        service_name=event.service_name,
                        new_state=ServiceConnectionState.RECONNECTING,
                        reason="health_check_consecutive_failures",
                        source="HealthMonitor"
                    )
                return

            # Enter WARNING from HEALTHY (first failure)
            if current_state == ServiceConnectionState.HEALTHY and failures >= warn_th:
                await self._transition_state(
                    agent_id=event.agent_id,
                    service_name=event.service_name,
                    new_state=ServiceConnectionState.WARNING,
                    reason="health_check_first_failure",
                    source="HealthMonitor"
                )
                return

        except Exception as e:
            logger.error(f"[LIFECYCLE] Failed to handle health check result for {event.service_name}: {e}", exc_info=True)

    async def _on_service_timeout(self, event: 'ServiceTimeout'):
        """
        Handle service timeout - transition state to UNREACHABLE
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
                await self._set_service_metadata_async(event.agent_id, event.service_name, metadata)

            # 转换到 UNREACHABLE 状态
            await self._transition_state(
                agent_id=event.agent_id,
                service_name=event.service_name,
                new_state=ServiceConnectionState.UNREACHABLE,
                reason=f"timeout_{event.timeout_type}",
                source="HealthMonitor"
            )

        except Exception as e:
            logger.error(f"[LIFECYCLE] Failed to handle timeout for {event.service_name}: {e}", exc_info=True)

    async def _on_reconnection_requested(self, event: 'ReconnectionRequested'):
        """
        处理重连请求 - 记录日志（实际重连由 ConnectionManager 处理）
        """
        logger.info(
            f"[LIFECYCLE] Reconnection requested: {event.service_name} "
            f"(retry={event.retry_count}, reason={event.reason})"
        )

        # Update metadata中的重连尝试次数
        try:
            metadata = await self._registry.get_service_metadata_async(event.agent_id, event.service_name)
            if metadata:
                metadata.reconnect_attempts = event.retry_count
                await self._set_service_metadata_async(event.agent_id, event.service_name, metadata)
        except Exception as e:
            logger.error(f"[LIFECYCLE] Failed to update reconnection metadata: {e}")
    
    def initialize_service(self, agent_id: str, service_name: str, service_config: dict) -> bool:
        """
        初始化服务 - 触发完整的事件流程
        
        这是添加服务的主入口，确保所有必要的事件被触发。
        
        Args:
            agent_id: Agent ID
            service_name: 服务名称
            service_config: 服务配置
            
        Returns:
            bool: 是否成功初始化
        """
        try:
            logger.info(f"[LIFECYCLE] initialize_service called: agent={agent_id}, service={service_name}")
            logger.debug(f"[LIFECYCLE] Service config: {service_config}")
            
            # 生成 client_id
            from mcpstore.core.utils.id_generator import ClientIDGenerator
            client_id = ClientIDGenerator.generate_deterministic_id(
                agent_id=agent_id,
                service_name=service_name,
                service_config=service_config,
                global_agent_store_id=agent_id  # 使用 agent_id 作为 global ID
            )
            logger.debug(f"[LIFECYCLE] Generated client_id: {client_id}")
            
            # 检查是否已存在映射
            existing_client_id = self._registry._agent_client_service.get_service_client_id(agent_id, service_name)
            if existing_client_id:
                logger.debug(f"[LIFECYCLE] Found existing client_id mapping: {existing_client_id}")
                client_id = existing_client_id
            
            # 发布 ServiceAddRequested 事件，触发完整流程
            from mcpstore.core.events.service_events import ServiceAddRequested
            import asyncio
            
            event = ServiceAddRequested(
                agent_id=agent_id,
                service_name=service_name,
                service_config=service_config,
                client_id=client_id,
                source="lifecycle_manager",
                wait_timeout=0
            )
            
            logger.info(f"[LIFECYCLE] Publishing ServiceAddRequested event for {service_name}")
            
            # 同步发布事件（在当前事件循环中）
            try:
                loop = asyncio.get_event_loop()
                if loop.is_running():
                    # 如果事件循环正在运行，创建任务
                    task = asyncio.create_task(self._event_bus.publish(event, wait=True))
                    # 不等待任务完成，让它在后台运行
                    logger.debug(f"[LIFECYCLE] Event published as background task")
                else:
                    # 如果事件循环未运行，同步运行
                    loop.run_until_complete(self._event_bus.publish(event, wait=True))
                    logger.debug(f"[LIFECYCLE] Event published synchronously")
            except RuntimeError as e:
                # 处理没有事件循环的情况
                logger.warning(f"[LIFECYCLE] No event loop available, creating new one: {e}")
                new_loop = asyncio.new_event_loop()
                asyncio.set_event_loop(new_loop)
                try:
                    new_loop.run_until_complete(self._event_bus.publish(event, wait=True))
                    logger.debug(f"[LIFECYCLE] Event published in new event loop")
                finally:
                    new_loop.close()
            
            logger.info(f"[LIFECYCLE] Service {service_name} initialization triggered successfully")
            return True
            
        except Exception as e:
            logger.error(f"[LIFECYCLE] Failed to initialize service {service_name}: {e}", exc_info=True)
            return False
    
    async def graceful_disconnect(self, agent_id: str, service_name: str, reason: str = "user_requested"):
        """优雅断开服务连接（不修改配置/注册表实体，仅生命周期断链）。

        - 将状态置为 DISCONNECTING → DISCONNECTED
        - 记录断开原因到 metadata
        - 由上层（可选）清理工具展示缓存
        """
        try:
            # 更新断开原因
            metadata = await self._registry.get_service_metadata_async(agent_id, service_name)
            if metadata:
                try:
                    metadata.disconnect_reason = reason
                    await self._set_service_metadata_async(agent_id, service_name, metadata)
                except Exception:
                    pass

            # 先进入 DISCONNECTING
            await self._transition_state(
                agent_id=agent_id,
                service_name=service_name,
                new_state=ServiceConnectionState.DISCONNECTING,
                reason=reason,
                source="LifecycleManager"
            )

            # 立即收敛为 DISCONNECTED（不等待外部回调）
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
        执行状态转换（唯一入口）
        """
        # 获取当前状态（异步接口）
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
        
        # 更新状态（异步接口）
        await self._registry.set_service_state_async(agent_id, service_name, new_state)
        
        # Update metadata（从 pykv 异步获取）
        try:
            metadata = await self._registry.get_service_metadata_async(agent_id, service_name)

            if metadata:
                if hasattr(metadata, 'state_entered_time'):
                    metadata.state_entered_time = datetime.now()
                try:
                    self._registry.set_service_metadata(agent_id, service_name, metadata)
                except Exception as e:
                    logger.error(f"[LIFECYCLE] Failed to update metadata for {service_name}: {e}")
                    raise
        except Exception as e:
            logger.error(f"[LIFECYCLE] Error updating metadata for {service_name}: {e}")
            raise
        
        # 发布状态变化事件
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
                # Failure: if no current state, assume INITIALIZING and transition to RECONNECTING
                if current_state is None:
                    logger.info(f"[LIFECYCLE] Assuming current state is INITIALIZING for {service_name}")
                    current_state = ServiceConnectionState.INITIALIZING
                # Failure: determine target state based on current state and failure count
                failure_count = metadata.consecutive_failures if metadata else 1

                if current_state == ServiceConnectionState.INITIALIZING:
                    # First connection failure -> RECONNECTING
                    new_state = ServiceConnectionState.RECONNECTING
                    reason = "initial_connection_failed"
                    logger.info(f"[LIFECYCLE] Will transition {service_name} from INITIALIZING to RECONNECTING (first failure)")
                elif failure_count >= self._config.reconnecting_failure_threshold:
                    # High failure count -> UNREACHABLE
                    new_state = ServiceConnectionState.UNREACHABLE
                    reason = "connection_unreachable"
                elif failure_count >= self._config.warning_failure_threshold:
                    # Medium failure count -> WARNING
                    new_state = ServiceConnectionState.WARNING
                    reason = "connection_warning"
                else:
                    # Low failure count -> RECONNECTING
                    new_state = ServiceConnectionState.RECONNECTING
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
