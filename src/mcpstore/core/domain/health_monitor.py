"""
健康检查管理器 - 负责服务健康监控

职责:
1. 监听 ServiceConnected 事件，启动定期健康检查
2. 定期检查服务健康状态
3. 发布 HealthCheckCompleted 事件
4. 检测服务超时
"""

import asyncio
import logging
import time
from typing import Dict, Tuple

from mcpstore.core.events.event_bus import EventBus
from mcpstore.core.events.service_events import (
    ServiceConnected, HealthCheckRequested, HealthCheckCompleted,
    ServiceTimeout, ServiceStateChanged
)
from mcpstore.core.lifecycle.config import ServiceLifecycleConfig
from mcpstore.core.models.service import ServiceConnectionState
from mcpstore.core.utils.mcp_client_helpers import temp_client_for_service

logger = logging.getLogger(__name__)


class HealthMonitor:
    """
    健康检查管理器

    职责:
    1. 监听 ServiceConnected 事件，启动定期健康检查
    2. 定期检查服务健康状态
    3. 发布 HealthCheckCompleted 事件
    4. 检测服务超时
    """

    def __init__(
        self,
        event_bus: EventBus,
        registry: 'CoreRegistry',
        lifecycle_config: 'ServiceLifecycleConfig',
        global_agent_store_id: str = "global_agent_store",
    ):
        self._event_bus = event_bus
        self._registry = registry
        self._config = lifecycle_config
        self._global_agent_store_id = global_agent_store_id

        # 从统一生命周期配置中读取参数
        self._check_interval = lifecycle_config.normal_heartbeat_interval
        self._warning_interval = lifecycle_config.warning_heartbeat_interval
        self._timeout_threshold = lifecycle_config.initialization_timeout
        self._ping_timeout = lifecycle_config.health_check_ping_timeout

        # 健康检查任务跟踪
        self._health_check_tasks: Dict[Tuple[str, str], asyncio.Task] = {}  # (agent_id, service_name) -> task
        self._is_running = False

        # 订阅事件
        self._event_bus.subscribe(ServiceConnected, self._on_service_connected, priority=30)
        self._event_bus.subscribe(HealthCheckRequested, self._on_health_check_requested, priority=100)
        self._event_bus.subscribe(ServiceStateChanged, self._on_state_changed, priority=20)

        logger.info(
            f"HealthMonitor initialized (bus={hex(id(self._event_bus))}, "
            f"normal_interval={self._check_interval}s, warning_interval={self._warning_interval}s, "
            f"timeout={self._timeout_threshold}s, ping_timeout={self._ping_timeout}s)"
        )

    async def start(self):
        """启动健康监控"""
        if self._is_running:
            logger.warning("HealthMonitor is already running")
            return

        self._is_running = True
        logger.info("HealthMonitor started")

    async def stop(self):
        """停止健康监控"""
        self._is_running = False

        # 取消所有健康检查任务
        for task in self._health_check_tasks.values():
            if not task.done():
                task.cancel()

        # 等待所有任务完成
        if self._health_check_tasks:
            await asyncio.gather(*self._health_check_tasks.values(), return_exceptions=True)

        self._health_check_tasks.clear()
        logger.info("HealthMonitor stopped")

    async def _on_service_connected(self, event: ServiceConnected):
        """
        处理服务连接成功 - 启动定期健康检查
        """
        logger.info(f"[HEALTH] Starting health check for: {event.service_name}")

        # 启动定期健康检查任务
        task_key = (event.agent_id, event.service_name)

        # 如果已有任务，先取消
        if task_key in self._health_check_tasks:
            old_task = self._health_check_tasks[task_key]
            if not old_task.done():
                old_task.cancel()

        # 创建新的健康检查任务
        task = asyncio.create_task(
            self._periodic_health_check(event.agent_id, event.service_name)
        )
        self._health_check_tasks[task_key] = task

    async def _on_health_check_requested(self, event: HealthCheckRequested):
        """
        处理健康检查请求 - 立即执行健康检查
        """
        # 统一使用全局命名空间读取状态（使用异步版本）
        global_name = await self._to_global_name_async(event.agent_id, event.service_name)
        current_state = await self._registry.get_service_state_async(self._global_agent_store_id, global_name)
        logger.info(f"[HEALTH] Manual health check requested: {event.service_name} (state={getattr(current_state,'value',str(current_state))}, bus={hex(id(self._event_bus))})")

        # 执行一次健康检查（关键路径使用同步派发，确保状态及时收敛）
        await self._execute_health_check(event.agent_id, event.service_name, wait=True)

    async def _on_state_changed(self, event: ServiceStateChanged):
        """
        处理状态变更 - 停止已断开服务的健康检查
        """
        # 如果服务进入终止/不可达状态，停止健康检查（使用统一小写枚举值）
        terminal_states = ["disconnected", "disconnecting", "unreachable"]
        if event.new_state in terminal_states:
            task_key = (event.agent_id, event.service_name)
            if task_key in self._health_check_tasks:
                task = self._health_check_tasks[task_key]
                if not task.done():
                    task.cancel()
                del self._health_check_tasks[task_key]
                logger.info(f"[HEALTH] Stopped health check for terminated service: {event.service_name}")

    async def _periodic_health_check(self, agent_id: str, service_name: str):
        """
        定期健康检查循环
        """
        logger.debug(f"[HEALTH] Periodic health check started: {service_name}")

        try:
            while self._is_running:
                # 根据当前状态选择检查间隔
                global_name = await self._to_global_name_async(agent_id, service_name)
                # 使用新的异步API获取服务状态
                current_state = await self._registry.get_service_state_async(self._global_agent_store_id, service_name)
                interval = self._warning_interval if current_state == ServiceConnectionState.WARNING else self._check_interval
                await asyncio.sleep(interval)

                # 执行健康检查
                await self._execute_health_check(agent_id, service_name)

        except asyncio.CancelledError:
            logger.debug(f"[HEALTH] Periodic health check cancelled: {service_name}")
        except Exception as e:
            logger.error(f"[HEALTH] Periodic health check error: {service_name} - {e}", exc_info=True)

    async def _execute_health_check(self, agent_id: str, service_name: str, wait: bool = False):
        """
        执行单次健康检查
        """
        start_time = time.time()

        try:
            # 如果服务已不存在，跳过检查，且停止周期任务
            global_name = await self._to_global_name_async(agent_id, service_name)
            # 使用异步 API 检查服务是否存在，避免在异步上下文中调用同步 API
            if not await self._registry.has_service_async(self._global_agent_store_id, global_name):
                logger.info(f"[HEALTH] Skip check for removed service: {service_name}")
                task_key = (agent_id, service_name)
                if task_key in self._health_check_tasks:
                    task = self._health_check_tasks.pop(task_key)
                    if not task.done():
                        task.cancel()
                return

            # 获取服务配置（新架构：从服务实体获取）
            # 从服务实体中获取服务配置，不再从 client_config 中获取
            service_entity = await self._registry._cache_service_manager.get_service(global_name)
            if service_entity is None:
                raise RuntimeError(
                    f"Service entity does not exist, cannot execute health check: service_name={service_name}, "
                    f"agent_id={agent_id}, global_name={global_name}"
                )
            
            service_config = service_entity.config
            if not service_config:
                raise RuntimeError(
                    f"Service configuration is empty, cannot execute health check: service_name={service_name}, "
                    f"agent_id={agent_id}, global_name={global_name}"
                )
            
            logger.debug(f"[HEALTH] Found service config for {service_name}: {list(service_config.keys())}")

            # 执行健康检查（使用临时 client + async with）
            try:
                # 设置超时并使用临时 client 进行健康检查
                ping_start = time.time()
                async with asyncio.timeout(self._ping_timeout):
                    async with temp_client_for_service(global_name, service_config) as client:
                        await client.ping()
                    response_time = time.time() - start_time

                    # 成功：仅上报成功与响应时间，不直接建议状态
                    logger.debug(f"[HEALTH] Check passed: {service_name} ({response_time:.2f}s)")

                    # 发布健康检查成功事件（手动检查使用同步派发）
                    # 注意：事件应该使用原始服务名称（Agent 视角），而非全局名称（Store 视角）
                    await self._publish_health_check_success(
                        agent_id, service_name, response_time, wait=wait
                    )
            except asyncio.TimeoutError:
                response_time = time.time() - start_time
                logger.warning(f"[HEALTH] Check timeout: {service_name}")
                # 注意：事件应该使用原始服务名称（Agent 视角），而非全局名称（Store 视角）
                await self._publish_health_check_failed(
                    agent_id, service_name, response_time, "Health check timeout", wait=wait
                )
            except Exception as e:
                response_time = time.time() - start_time
                error_message = str(e)
                logger.error(f"[HEALTH] Check failed: {service_name} - {error_message}")
                # 分类认证失败，并记录到元数据
                failure_reason = None
                try:
                    status_code = getattr(getattr(e, 'response', None), 'status_code', None)
                    if status_code in (401, 403):
                        failure_reason = 'auth_failed'
                    else:
                        lower_msg = error_message.lower()
                        if any(word in lower_msg for word in ['unauthorized', 'forbidden', 'invalid token', 'invalid api key']):
                            failure_reason = 'auth_failed'
                except Exception:
                    pass
                try:
                    metadata = await self._registry.get_service_metadata_async(self._global_agent_store_id, global_name)
                    if metadata:
                        metadata.failure_reason = failure_reason
                        metadata.error_message = error_message
                        await self._registry.set_service_metadata_async(self._global_agent_store_id, global_name, metadata)
                except Exception as e:
                    logger.error(f"[HEALTH] Failed to update metadata for {global_name}: {e}")
                    raise
                # 注意：事件应该使用原始服务名称（Agent 视角），而非全局名称（Store 视角）
                await self._publish_health_check_failed(
                    agent_id, service_name, response_time, error_message, wait=wait
                )
        except Exception as e:
            logger.error(f"[HEALTH] Execute health check error: {service_name} - {e}", exc_info=True)

    async def _publish_health_check_success(
        self,
        agent_id: str,
        service_name: str,
        response_time: float,
        wait: bool = False
    ):
        """发布健康检查成功事件"""
        event = HealthCheckCompleted(
            agent_id=agent_id,
            service_name=service_name,
            success=True,
            response_time=response_time,
            suggested_state=None
        )
        await self._event_bus.publish(event, wait=wait)

    async def _publish_health_check_failed(
        self,
        agent_id: str,
        service_name: str,
        response_time: float,
        error_message: str,
        wait: bool = False
    ):
        """发布健康检查失败事件"""
        event = HealthCheckCompleted(
            agent_id=agent_id,
            service_name=service_name,
            success=False,
            response_time=response_time,
            error_message=error_message,
            suggested_state=None
        )
        await self._event_bus.publish(event, wait=wait)

    async def _to_global_name_async(self, agent_id: str, service_name: str) -> str:
        """将本地服务名映射为全局服务名（异步版本，映射失败则返回原名）。"""
        try:
            mapping = await self._registry.get_global_name_from_agent_service_async(agent_id, service_name)
            return mapping or service_name
        except Exception:
            return service_name

    async def check_timeouts(self):
        """
        检查超时的服务（可由外部定期调用）
        """
        current_time = time.time()

        try:
            # 从缓存层获取所有服务并检查超时
            # 使用 _cache_layer_manager（CacheLayerManager），它有 get_all_entities_async 方法
            service_entities = await self._registry._cache_layer_manager.get_all_entities_async("services")

            for entity_key, entity_data in service_entities.items():
                if hasattr(entity_data, 'value'):
                    data = entity_data.value
                elif isinstance(entity_data, dict):
                    data = entity_data
                else:
                    continue

                agent_id = data.get('source_agent', 'unknown')
                service_name = data.get('service_original_name', entity_key)

                # 从缓存层获取服务元数据
                metadata = await self._registry.get_service_metadata_async(agent_id, service_name)
                if not metadata:
                    continue

                # 检查初始化超时
                state = await self._registry.get_service_state_async(agent_id, service_name)
                if state == ServiceConnectionState.INITIALIZING:
                    state_entered_time = await self._get_state_entered_time(metadata)
                    if state_entered_time:
                        elapsed = current_time - state_entered_time.timestamp()
                        if elapsed > self._timeout_threshold:
                            logger.warning(f"[HEALTH] Initialization timeout: {service_name} ({elapsed:.1f}s)")
                            await self._publish_timeout_event(
                                agent_id, service_name, "initialization", elapsed
                            )

        except Exception as e:
            logger.error(f"[HEALTH] Health check timeout failed: {e}", exc_info=True)

    async def _publish_timeout_event(
        self,
        agent_id: str,
        service_name: str,
        timeout_type: str,
        elapsed_time: float
    ):
        """发布超时事件"""
        event = ServiceTimeout(
            agent_id=agent_id,
            service_name=service_name,
            timeout_type=timeout_type,
            elapsed_time=elapsed_time
        )
        await self._event_bus.publish(event)

    async def _get_state_entered_time(self, metadata):
        """
        从元数据中获取状态进入时间

        Args:
            metadata: 服务元数据

        Returns:
            状态进入时间的datetime对象，如果不存在则返回None
        """
        if hasattr(metadata, 'state_entered_time'):
            return metadata.state_entered_time
        elif isinstance(metadata, dict):
            state_entered_time = metadata.get('state_entered_time')
            if state_entered_time:
                if isinstance(state_entered_time, str):
                    # 尝试解析ISO格式时间字符串
                    try:
                        from datetime import datetime
                        return datetime.fromisoformat(state_entered_time.replace('Z', '+00:00'))
                    except:
                        return None
                return state_entered_time
        return None

