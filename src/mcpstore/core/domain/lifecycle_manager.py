"""
生命周期管理器 - 负责服务状态管理

职责:
1. 监听 ServiceCached 事件，初始化生命周期状态
2. 监听 ServiceConnected/ServiceConnectionFailed 事件，转换状态
3. 发布 ServiceStateChanged 事件
4. 管理状态元数据
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
    生命周期管理器
    
    职责:
    1. 监听 ServiceCached 事件，初始化生命周期状态
    2. 监听 ServiceConnected/ServiceConnectionFailed 事件，转换状态
    3. 发布 ServiceStateChanged 事件
    4. 管理状态元数据
    """
    
    def __init__(self, event_bus: EventBus, registry: 'CoreRegistry', lifecycle_config: 'ServiceLifecycleConfig' = None):
        self._event_bus = event_bus
        self._registry = registry
        # 配置（阈值/心跳间隔）
        if lifecycle_config is None:
            from mcpstore.core.lifecycle.config import ServiceLifecycleConfig
            lifecycle_config = ServiceLifecycleConfig()
        self._config = lifecycle_config

        # 订阅事件
        self._event_bus.subscribe(ServiceCached, self._on_service_cached, priority=90)
        self._event_bus.subscribe(ServiceConnected, self._on_service_connected, priority=40)
        self._event_bus.subscribe(ServiceConnectionFailed, self._on_service_connection_failed, priority=40)

        # 🆕 订阅健康检查和超时事件
        from mcpstore.core.events.service_events import HealthCheckCompleted, ServiceTimeout, ReconnectionRequested
        self._event_bus.subscribe(HealthCheckCompleted, self._on_health_check_completed, priority=50)
        self._event_bus.subscribe(ServiceTimeout, self._on_service_timeout, priority=50)
        self._event_bus.subscribe(ReconnectionRequested, self._on_reconnection_requested, priority=30)

        logger.info("LifecycleManager initialized and subscribed to events")
    
    async def _on_service_cached(self, event: ServiceCached):
        """
        处理服务已缓存事件 - 初始化生命周期状态
        """
        logger.info(f"[LIFECYCLE] Initializing lifecycle for: {event.service_name}")
        
        try:
            # 🔧 修复：检查是否已有 metadata（CacheManager 可能已创建）
            existing_metadata = self._registry.get_service_metadata(event.agent_id, event.service_name)
            
            if existing_metadata and existing_metadata.service_config:
                # 如果已有 metadata 且包含配置，保留原有配置
                service_config = existing_metadata.service_config
                logger.debug(f"[LIFECYCLE] Preserving existing service_config for: {event.service_name}")
            else:
                # 否则，尝试从客户端配置中读取
                client_config = self._registry.get_client_config_from_cache(event.client_id)
                service_config = client_config.get("mcpServers", {}).get(event.service_name, {}) if client_config else {}
                logger.debug(f"[LIFECYCLE] Loading service_config from client config for: {event.service_name}")
            
            # 创建或更新元数据（保留配置信息）
            metadata = ServiceStateMetadata(
                service_name=event.service_name,
                agent_id=event.agent_id,
                state_entered_time=datetime.now(),
                consecutive_failures=0,
                reconnect_attempts=0,
                next_retry_time=None,
                error_message=None,
                service_config=service_config  # 🔧 修复：使用正确的配置
            )
            
            self._registry.set_service_metadata(event.agent_id, event.service_name, metadata)
            
            # 验证配置是否正确保存
            logger.debug(f"[LIFECYCLE] Metadata saved with config keys: {list(service_config.keys()) if service_config else 'None'}")
            
            logger.info(f"[LIFECYCLE] Lifecycle initialized: {event.service_name} -> INITIALIZING")
            
            # 发布初始化完成事件
            initialized_event = ServiceInitialized(
                agent_id=event.agent_id,
                service_name=event.service_name,
                initial_state="initializing"
            )
            await self._event_bus.publish(initialized_event, wait=True)
            
        except Exception as e:
            logger.error(f"[LIFECYCLE] Failed to initialize lifecycle for {event.service_name}: {e}", exc_info=True)
    
    async def _on_service_connected(self, event: ServiceConnected):
        """
        处理服务连接成功 - 转换状态为 HEALTHY
        """
        logger.info(f"[LIFECYCLE] Service connected: {event.service_name}")
        
        try:
            await self._transition_state(
                agent_id=event.agent_id,
                service_name=event.service_name,
                new_state=ServiceConnectionState.HEALTHY,
                reason="connection_success",
                source="ConnectionManager"
            )
            
            # 重置失败计数
            metadata = self._registry.get_service_metadata(event.agent_id, event.service_name)
            if metadata:
                metadata.consecutive_failures = 0
                metadata.reconnect_attempts = 0
                metadata.error_message = None
                metadata.last_health_check = datetime.now()
                metadata.last_response_time = event.connection_time
                self._registry.set_service_metadata(event.agent_id, event.service_name, metadata)
            
        except Exception as e:
            logger.error(f"[LIFECYCLE] Failed to transition state for {event.service_name}: {e}", exc_info=True)
    
    async def _on_service_connection_failed(self, event: ServiceConnectionFailed):
        """
        处理服务连接失败 - 转换状态为 RECONNECTING
        """
        logger.warning(f"[LIFECYCLE] Service connection failed: {event.service_name} ({event.error_message})")
        
        try:
            # 更新元数据
            metadata = self._registry.get_service_metadata(event.agent_id, event.service_name)
            if metadata:
                metadata.consecutive_failures += 1
                metadata.error_message = event.error_message
                metadata.last_failure_time = datetime.now()
                self._registry.set_service_metadata(event.agent_id, event.service_name, metadata)
            
            # 根据当前状态决定目标状态
            current_state = self._registry.get_service_state(event.agent_id, event.service_name)
            
            if current_state == ServiceConnectionState.INITIALIZING:
                # 初次连接失败 -> RECONNECTING
                new_state = ServiceConnectionState.RECONNECTING
                reason = "initial_connection_failed"
            else:
                # 其他情况也转到 RECONNECTING
                new_state = ServiceConnectionState.RECONNECTING
                reason = "connection_failed"
            
            await self._transition_state(
                agent_id=event.agent_id,
                service_name=event.service_name,
                new_state=new_state,
                reason=reason,
                source="ConnectionManager"
            )
            
        except Exception as e:
            logger.error(f"[LIFECYCLE] Failed to handle connection failure for {event.service_name}: {e}", exc_info=True)

    async def _on_health_check_completed(self, event: 'HealthCheckCompleted'):
        """
        处理健康检查完成 - 根据健康状态转换服务状态
        """
        logger.debug(f"[LIFECYCLE] Health check completed: {event.service_name} (success={event.success})")

        try:
            # 更新元数据
            metadata = self._registry.get_service_metadata(event.agent_id, event.service_name)
            if metadata:
                metadata.last_health_check = datetime.now()
                metadata.last_response_time = event.response_time

                if event.success:
                    metadata.consecutive_failures = 0
                    metadata.error_message = None
                else:
                    metadata.consecutive_failures += 1
                    metadata.error_message = event.error_message

                self._registry.set_service_metadata(event.agent_id, event.service_name, metadata)

            # 基于失败计数与当前状态的转换规则（忽略 suggested_state）
            current_state = self._registry.get_service_state(event.agent_id, event.service_name)
            failures = 0
            if metadata:
                failures = metadata.consecutive_failures

            # 成功：从 INITIALIZING/WARNING 回到 HEALTHY；HEALTHY 保持
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

            # 失败：按阈值推进 WARNING/RECONNECTING
            warn_th = self._config.warning_failure_threshold
            rec_th = self._config.reconnecting_failure_threshold

            # 达到重连阈值：进入 RECONNECTING
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

            # 从 HEALTHY 进入 WARNING（首次失败）
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
        处理服务超时 - 转换状态为 UNREACHABLE
        """
        logger.warning(
            f"[LIFECYCLE] Service timeout: {event.service_name} "
            f"(type={event.timeout_type}, elapsed={event.elapsed_time:.1f}s)"
        )

        try:
            # 更新元数据
            metadata = self._registry.get_service_metadata(event.agent_id, event.service_name)
            if metadata:
                metadata.error_message = f"Timeout: {event.timeout_type} ({event.elapsed_time:.1f}s)"
                self._registry.set_service_metadata(event.agent_id, event.service_name, metadata)

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

        # 更新元数据中的重连尝试次数
        try:
            metadata = self._registry.get_service_metadata(event.agent_id, event.service_name)
            if metadata:
                metadata.reconnect_attempts = event.retry_count
                self._registry.set_service_metadata(event.agent_id, event.service_name, metadata)
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
            existing_client_id = self._registry.get_service_client_id(agent_id, service_name)
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
            metadata = self._registry.get_service_metadata(agent_id, service_name)
            if metadata:
                try:
                    metadata.disconnect_reason = reason
                    self._registry.set_service_metadata(agent_id, service_name, metadata)
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
        old_state = self._registry.get_service_state(agent_id, service_name)
        
        if old_state == new_state:
            logger.debug(f"[LIFECYCLE] State unchanged: {service_name} already in {new_state.value}")
            return
        
        logger.info(
            f"[LIFECYCLE] State transition: {service_name} "
            f"{old_state.value if old_state else 'None'} -> {new_state.value} "
            f"(reason={reason}, source={source})"
        )
        
        # 更新状态
        self._registry.set_service_state(agent_id, service_name, new_state)
        
        # 更新元数据
        metadata = self._registry.get_service_metadata(agent_id, service_name)
        if metadata:
            metadata.state_entered_time = datetime.now()
            self._registry.set_service_metadata(agent_id, service_name, metadata)
        
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

