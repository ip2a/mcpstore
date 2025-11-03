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
        check_interval: float = 30.0,  # 正常健康检查间隔（秒）
        timeout_threshold: float = 300.0,  # 初始化超时（秒）
        ping_timeout: float = 10.0,  # ping 调用超时（秒）
        warning_interval: float = 10.0,  # 警告状态下的检查间隔（秒）
        global_agent_store_id: str = "global_agent_store",
    ):
        self._event_bus = event_bus
        self._registry = registry
        self._check_interval = check_interval
        self._warning_interval = warning_interval
        self._timeout_threshold = timeout_threshold
        self._ping_timeout = ping_timeout
        self._global_agent_store_id = global_agent_store_id

        # 健康检查任务跟踪
        self._health_check_tasks: Dict[Tuple[str, str], asyncio.Task] = {}  # (agent_id, service_name) -> task
        self._is_running = False

        # 订阅事件
        self._event_bus.subscribe(ServiceConnected, self._on_service_connected, priority=30)
        self._event_bus.subscribe(HealthCheckRequested, self._on_health_check_requested, priority=100)
        self._event_bus.subscribe(ServiceStateChanged, self._on_state_changed, priority=20)

        logger.info(
            f"HealthMonitor initialized (bus={hex(id(self._event_bus))}, normal_interval={check_interval}s, warning_interval={warning_interval}s, "
            f"timeout={timeout_threshold}s, ping_timeout={ping_timeout}s)"
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
        # 统一使用全局命名空间读取状态
        global_name = self._to_global_name(event.agent_id, event.service_name)
        current_state = self._registry.get_service_state(self._global_agent_store_id, global_name)
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
                global_name = self._to_global_name(agent_id, service_name)
                current_state = self._registry.get_service_state(self._global_agent_store_id, global_name)
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
            global_name = self._to_global_name(agent_id, service_name)
            if not self._registry.has_service(self._global_agent_store_id, global_name):
                logger.info(f"[HEALTH] Skip check for removed service: {service_name}")
                task_key = (agent_id, service_name)
                if task_key in self._health_check_tasks:
                    task = self._health_check_tasks.pop(task_key)
                    if not task.done():
                        task.cancel()
                return

            # 获取服务配置（优先缓存）
            service_config = self._registry.get_service_config_from_cache(self._global_agent_store_id, global_name) \
                or self._registry.get_service_config(self._global_agent_store_id, global_name)
            if not service_config:
                logger.warning(f"[HEALTH] No service config found: {service_name} (skip without state change)")
                return

            # 执行健康检查（使用临时 client + async with）
            try:
                # 设置超时并使用临时 client 进行健康检查
                async with asyncio.timeout(self._ping_timeout):
                    async with temp_client_for_service(global_name, service_config) as client:
                        await client.ping()
                    response_time = time.time() - start_time

                    # 成功：仅上报成功与响应时间，不直接建议状态
                    logger.debug(f"[HEALTH] Check passed: {service_name} ({response_time:.2f}s)")

                    # 发布健康检查成功事件（手动检查使用同步派发）
                    await self._publish_health_check_success(
                        agent_id, global_name, response_time, wait=wait
                    )
            except asyncio.TimeoutError:
                response_time = time.time() - start_time
                logger.warning(f"[HEALTH] Check timeout: {service_name}")
                await self._publish_health_check_failed(
                    agent_id, global_name, response_time, "Health check timeout", wait=wait
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
                    metadata = self._registry.get_service_metadata(self._global_agent_store_id, global_name)
                    if metadata:
                        metadata.failure_reason = failure_reason
                        metadata.error_message = error_message
                        self._registry.set_service_metadata(self._global_agent_store_id, global_name, metadata)
                except Exception:
                    pass
                await self._publish_health_check_failed(
                    agent_id, global_name, response_time, error_message, wait=wait
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

    def _to_global_name(self, agent_id: str, service_name: str) -> str:
        """将本地服务名映射为全局服务名（映射失败则返回原名）。"""
        try:
            mapping = self._registry.get_global_name_from_agent_service(agent_id, service_name)
            return mapping or service_name
        except Exception:
            return service_name

    async def check_timeouts(self):
        """
        检查超时的服务（可由外部定期调用）
        """
        current_time = time.time()

        # 遍历所有服务，检查超时
        for agent_id in self._registry.service_states.keys():
            service_names = self._registry.get_all_service_names(agent_id)

            for service_name in service_names:
                metadata = self._registry.get_service_metadata(agent_id, service_name)
                if not metadata:
                    continue

                # 检查初始化超时
                if metadata.state == ServiceConnectionState.INITIALIZING:
                    elapsed = current_time - metadata.state_entered_time.timestamp()
                    if elapsed > self._timeout_threshold:
                        logger.warning(f"[HEALTH] Initialization timeout: {service_name} ({elapsed:.1f}s)")
                        await self._publish_timeout_event(
                            agent_id, service_name, "initialization", elapsed
                        )

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

