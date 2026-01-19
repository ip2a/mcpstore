"""
重连调度器 - 负责自动重连管理

职责:
1. 定期扫描 CIRCUIT_OPEN 状态的服务
2. 检查是否到达重连时间
3. 发布 ReconnectionRequested 事件
4. 管理重连延迟策略（指数退避）
"""

import asyncio
import logging
import time
from datetime import datetime, timedelta
from typing import Dict, Optional, List

from mcpstore.config.config_dataclasses import ServiceLifecycleConfig
from mcpstore.core.events.event_bus import EventBus
from mcpstore.core.events.service_events import (
    ServiceStateChanged, ReconnectionRequested, ReconnectionScheduled,
    ServiceConnectionFailed
)
from mcpstore.core.models.service import ServiceConnectionState

logger = logging.getLogger(__name__)


class ReconnectionScheduler:
    """
    重连调度器
    
    职责:
    1. 定期扫描 CIRCUIT_OPEN 状态的服务
    2. 检查是否到达重连时间
    3. 发布 ReconnectionRequested 事件
    4. 管理重连延迟策略（指数退避）
    """
    
    def __init__(
        self, 
        event_bus: EventBus, 
        registry: 'CoreRegistry',
        lifecycle_config: 'ServiceLifecycleConfig',
        scan_interval: float = 1.0,  # 默认1秒扫描一次
    ):
        self._event_bus = event_bus
        self._registry = registry
        self._config = lifecycle_config
        self._scan_interval = scan_interval
        # 从统一配置读取重连相关参数
        self._base_delay = lifecycle_config.backoff_base
        self._max_delay = lifecycle_config.backoff_max
        self._max_retries = lifecycle_config.max_reconnect_attempts
        self._empty_scan_log_interval = 30.0
        self._last_empty_scan_log = 0.0
        
        # 调度器状态
        self._is_running = False
        self._scheduler_task: Optional[asyncio.Task] = None
        
        # 订阅事件
        self._event_bus.subscribe(ServiceStateChanged, self._on_state_changed, priority=20)
        self._event_bus.subscribe(ServiceConnectionFailed, self._on_connection_failed, priority=50)
        
        logger.info(f"ReconnectionScheduler initialized (scan_interval={scan_interval}s)")
    
    async def start(self):
        """启动重连调度器"""
        if self._is_running:
            logger.warning("ReconnectionScheduler is already running")
            return
        
        self._is_running = True
        
        # 启动调度循环
        self._scheduler_task = asyncio.create_task(self._scheduler_loop())
        
        logger.info("ReconnectionScheduler started")
    
    async def stop(self):
        """停止重连调度器"""
        self._is_running = False
        
        # 取消调度任务
        if self._scheduler_task and not self._scheduler_task.done():
            self._scheduler_task.cancel()
            try:
                await self._scheduler_task
            except asyncio.CancelledError:
                pass
        
        logger.info("ReconnectionScheduler stopped")
    
    async def _scheduler_loop(self):
        """
        调度循环 - 定期扫描需要重连的服务
        """
        logger.debug("[RECONNECT] Scheduler loop started")
        
        try:
            while self._is_running:
                # 扫描需要重连的服务
                await self._scan_reconnection_services()
                
                # 等待下一个扫描周期
                await asyncio.sleep(self._scan_interval)
                
        except asyncio.CancelledError:
            logger.debug("[RECONNECT] Scheduler loop cancelled")
        except Exception as e:
            logger.error(f"[RECONNECT] Scheduler loop error: {e}", exc_info=True)
    
    async def _scan_reconnection_services(self):
        """
        扫描所有 CIRCUIT_OPEN 状态的服务

        严格按照Functional Core, Imperative Shell原则：
        1. 调用纯同步核心生成扫描计划
        2. 纯异步执行缓存访问和事件发布
        3. 避免直接访问内存字典
        """
        try:
            # 1. 调用纯同步核心生成扫描计划
            scan_plan = self._generate_scan_plan()
            self._log_scan_summary(len(scan_plan["services_to_check"]))

            # 2. 纯异步执行扫描操作
            await self._execute_scan_plan(scan_plan)

        except Exception as e:
            logger.error(f"[RECONNECT] [ERROR] Failed to scan services: {e}", exc_info=True)

    def _log_scan_summary(self, count: int) -> None:
        """控制扫描结果日志的频率，避免空计划时频繁刷屏"""
        if count > 0:
            logger.debug(f"[RECONNECT] [SCAN] Scan plan generated: {count} services need to be checked")
            return
        now = time.time()
        if now - self._last_empty_scan_log >= self._empty_scan_log_interval:
            logger.debug("[RECONNECT] [SCAN] Scan plan generated: 0 services need to be checked")
            self._last_empty_scan_log = now

    def _generate_scan_plan(self) -> Dict[str, any]:
        """
        纯同步核心：生成重连扫描计划

        不涉及任何IO操作，只生成操作计划

        Returns:
            包含需要检查的服务列表的字典
        """
        current_time = datetime.now()
        services_to_check = []

        # 通过事件系统获取所有服务，避免直接访问内存字典
        # 这里暂时使用简化的实现，后续可以通过事件获取服务列表

        return {
            "scan_time": current_time,
            "services_to_check": services_to_check,
            "max_retries": self._max_retries
        }

    async def _execute_scan_plan(self, scan_plan: Dict[str, any]):
        """
        异步外壳：执行扫描计划

        Args:
            scan_plan: 由_generate_scan_plan生成的扫描计划
        """
        current_time = scan_plan["scan_time"]
        services_to_check = scan_plan["services_to_check"]

        if not services_to_check:
            # 没有需要检查的服务，从缓存层获取所有服务并检查状态
            services_to_check = await self._get_all_services_from_cache()

        for service_info in services_to_check:
            agent_id = service_info["agent_id"]
            service_name = service_info["service_name"]

            try:
                # 从缓存层获取服务状态
                state = await self._get_service_state_from_cache(agent_id, service_name)

                # 只处理 CIRCUIT_OPEN 状态的服务
                if state != ServiceConnectionState.CIRCUIT_OPEN:
                    continue

                # 从缓存层获取服务元数据
                metadata = await self._get_service_metadata_from_cache(agent_id, service_name)
                if not metadata:
                    continue

                # 检查是否到达重连时间
                if await self._should_retry_connection(metadata, current_time):
                    retry_count = await self._get_retry_count(metadata)

                    # 检查是否超过最大重试次数
                    if retry_count >= self._max_retries:
                        logger.warning(
                            f"[RECONNECT] Max retries reached: {service_name} (retries={retry_count})"
                        )
                        # 转换到 DISCONNECTED 状态
                        await self._transition_to_unreachable(agent_id, service_name)
                        continue

                    # 发布重连请求事件
                    logger.info(
                        f"[RECONNECT] Triggering reconnection: {service_name} "
                        f"(retry={retry_count + 1}/{self._max_retries})"
                    )

                    await self._publish_reconnection_requested(
                        agent_id, service_name, retry_count
                    )

                    # 更新元数据中的重试计数
                    metadata.reconnect_attempts = retry_count + 1
                    await self._set_service_metadata_in_cache(agent_id, service_name, metadata)

            except Exception as e:
                logger.error(f"[RECONNECT] [ERROR] Failed to process service {service_name}: {e}", exc_info=True)

    # ==================== 缓存层访问辅助方法 ====================

    async def _get_all_services_from_cache(self) -> List[Dict[str, str]]:
        """
        从缓存层获取所有服务

        Returns:
            服务信息列表，每个元素包含 agent_id 和 service_name
        """
        try:
            # 从缓存层获取所有服务实体
            services = []

            # 使用 _cache_layer_manager（CacheLayerManager）获取所有服务实体
            # 不再使用 _cache_layer，因为它在 Redis 模式下是 RedisStore，没有 get_all_entities_async 方法
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

                services.append({
                    "agent_id": agent_id,
                    "service_name": service_name
                })

            return services

        except Exception as e:
            logger.error(f"[RECONNECT] [ERROR] Failed to get service list from cache layer: {e}")
            return []

    async def _get_service_state_from_cache(self, agent_id: str, service_name: str) -> ServiceConnectionState:
        """
        从缓存层获取服务状态

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            服务连接状态
        """
        try:
            state_data = await self._registry.get_service_state_async(agent_id, service_name)

            if hasattr(state_data, 'health_status'):
                return ServiceConnectionState(state_data.health_status)
            elif isinstance(state_data, dict):
                health_status = state_data.get("health_status", "disconnected")
                return ServiceConnectionState(health_status)
            else:
                return ServiceConnectionState.DISCONNECTED

        except Exception as e:
            logger.debug(f"[RECONNECT] [ERROR] Failed to get service state {agent_id}:{service_name}: {e}")
            return ServiceConnectionState.DISCONNECTED

    async def _get_service_metadata_from_cache(self, agent_id: str, service_name: str):
        """
        从缓存层获取服务元数据

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            服务元数据，如果不存在则返回None
        """
        try:
            return await self._registry.get_service_metadata_async(agent_id, service_name)
        except Exception as e:
            logger.debug(f"[RECONNECT] [ERROR] Failed to get service metadata {agent_id}:{service_name}: {e}")
            return None

    async def _set_service_metadata_in_cache(self, agent_id: str, service_name: str, metadata):
        """
        在缓存层设置服务元数据

        Args:
            agent_id: Agent ID
            service_name: 服务名称
            metadata: 服务元数据
        """
        try:
            # 使用原始架构签名的方法
            await self._registry.set_service_metadata_async_v2(agent_id, service_name, metadata)
        except Exception as e:
            logger.error(f"[RECONNECT] [ERROR] Failed to set service metadata {agent_id}:{service_name}: {e}")

    async def _should_retry_connection(self, metadata, current_time) -> bool:
        """
        判断是否应该重连

        Args:
            metadata: 服务元数据
            current_time: 当前时间

        Returns:
            是否应该重连
        """
        if not hasattr(metadata, 'next_retry_time') or metadata.next_retry_time is None:
            return False

        return current_time >= metadata.next_retry_time

    async def _get_retry_count(self, metadata) -> int:
        """
        获取重试次数

        Args:
            metadata: 服务元数据

        Returns:
            重试次数
        """
        if hasattr(metadata, 'reconnect_attempts'):
            return metadata.reconnect_attempts
        elif isinstance(metadata, dict):
            return metadata.get('reconnect_attempts', 0)
        else:
            return 0

    async def _transition_to_unreachable(self, agent_id: str, service_name: str):
        """
        将服务转换到DISCONNECTED状态

        Args:
            agent_id: Agent ID
            service_name: 服务名称
        """
        try:
            logger.warning(f"[RECONNECT] Service marked as unreachable: {service_name}")

            # 发布状态变更事件
            state_event = ServiceStateChanged(
                agent_id=agent_id,
                service_name=service_name,
                old_state="CIRCUIT_OPEN",
                new_state="DISCONNECTED",
                timestamp=datetime.now(),
                reason="Max retries exceeded"
            )
            await self._event_bus.publish(state_event)

        except Exception as e:
            logger.error(f"[RECONNECT] [ERROR] Failed to transition state {agent_id}:{service_name}: {e}")

    async def _publish_reconnection_requested(self, agent_id: str, service_name: str, retry_count: int):
        """
        发布重连请求事件

        Args:
            agent_id: Agent ID
            service_name: 服务名称
            retry_count: 重试次数
        """
        try:
            reconnection_event = ReconnectionRequested(
                agent_id=agent_id,
                service_name=service_name,
                retry_count=retry_count,
                max_retries=self._max_retries,
                timestamp=datetime.now()
            )
            await self._event_bus.publish(reconnection_event)

        except Exception as e:
            logger.error(f"[RECONNECT] [ERROR] Failed to publish reconnection event {agent_id}:{service_name}: {e}")
    
    async def _on_state_changed(self, event: ServiceStateChanged):
        """
        处理状态变更 - 重置重试计数器
        """
        # 如果服务成功连接，重置重试计数器
        if event.new_state == "HEALTHY":
            # 从 pykv 异步获取元数据
            metadata = await self._registry.get_service_metadata_async(event.agent_id, event.service_name)
            if metadata:
                metadata.reconnect_attempts = 0
                metadata.next_retry_time = None
                await self._registry.set_service_metadata_async(event.agent_id, event.service_name, metadata)
                logger.info(f"[RECONNECT] Service recovered, resetting retry count: {event.service_name}")

        # 如果服务进入 CIRCUIT_OPEN 状态，调度重连
        elif event.new_state == "CIRCUIT_OPEN":
            await self._schedule_reconnection(event.agent_id, event.service_name)
    
    async def _on_connection_failed(self, event: ServiceConnectionFailed):
        """
        处理连接失败 - 调度重连
        """
        logger.debug(f"[RECONNECT] Connection failed, scheduling reconnection: {event.service_name}")
        await self._schedule_reconnection(event.agent_id, event.service_name)
    
    async def _schedule_reconnection(self, agent_id: str, service_name: str):
        """
        调度重连 - 计算下次重连时间
        """
        # 从 pykv 异步获取元数据
        metadata = await self._registry.get_service_metadata_async(agent_id, service_name)
        if not metadata:
            return

        retry_count = metadata.reconnect_attempts

        # 计算重连延迟（指数退避）
        delay = self._calculate_reconnect_delay(retry_count)
        next_retry_time = datetime.now() + timedelta(seconds=delay)

        # 更新元数据
        metadata.next_retry_time = next_retry_time
        await self._registry.set_service_metadata_async(agent_id, service_name, metadata)
        
        logger.info(
            f"[RECONNECT] Scheduled reconnection: {service_name} "
            f"(delay={delay:.1f}s, retry={retry_count})"
        )
        
        # 发布重连已调度事件
        event = ReconnectionScheduled(
            agent_id=agent_id,
            service_name=service_name,
            next_retry_time=next_retry_time.timestamp(),
            retry_delay=delay
        )
        await self._event_bus.publish(event)
    
    def _calculate_reconnect_delay(self, retry_count: int) -> float:
        """
        计算重连延迟（指数退避）
        
        公式: delay = min(base_delay * 2^retry_count, max_delay)
        """
        delay = self._base_delay * (2 ** retry_count)
        return min(delay, self._max_delay)
    
    async def _publish_reconnection_requested(
        self,
        agent_id: str,
        service_name: str,
        retry_count: int
    ):
        """发布重连请求事件"""
        event = ReconnectionRequested(
            agent_id=agent_id,
            service_name=service_name,
            retry_count=retry_count,
            reason="scheduled_retry"
        )
        await self._event_bus.publish(event)
    
    async def _transition_to_unreachable(self, agent_id: str, service_name: str):
        """通过事件系统请求转换到 DISCONNECTED 状态"""
        from mcpstore.core.events.service_events import ServiceTimeout

        event = ServiceTimeout(
            agent_id=agent_id,
            service_name=service_name,
            timeout_type="max_retries",
            elapsed_time=0.0,
        )
        await self._event_bus.publish(event)
