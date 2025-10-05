"""
重连调度器 - 负责自动重连管理

职责:
1. 定期扫描 RECONNECTING 状态的服务
2. 检查是否到达重连时间
3. 发布 ReconnectionRequested 事件
4. 管理重连延迟策略（指数退避）
"""

import asyncio
import logging
from datetime import datetime, timedelta
from typing import Dict, Optional

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
    1. 定期扫描 RECONNECTING 状态的服务
    2. 检查是否到达重连时间
    3. 发布 ReconnectionRequested 事件
    4. 管理重连延迟策略（指数退避）
    """
    
    def __init__(
        self, 
        event_bus: EventBus, 
        registry: 'CoreRegistry',
        scan_interval: float = 1.0,  # 默认1秒扫描一次
        base_delay: float = 2.0,  # 基础延迟2秒
        max_delay: float = 300.0,  # 最大延迟5分钟
        max_retries: int = 10  # 最大重试次数
    ):
        self._event_bus = event_bus
        self._registry = registry
        self._scan_interval = scan_interval
        self._base_delay = base_delay
        self._max_delay = max_delay
        self._max_retries = max_retries
        
        # 调度器状态
        self._is_running = False
        self._scheduler_task: Optional[asyncio.Task] = None
        
        # 重连计数器
        self._retry_counts: Dict[tuple, int] = {}  # (agent_id, service_name) -> retry_count
        
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
        扫描所有 RECONNECTING 状态的服务
        """
        current_time = datetime.now()
        
        # 遍历所有 agent
        for agent_id in self._registry.service_states.keys():
            service_names = self._registry.get_all_service_names(agent_id)
            
            for service_name in service_names:
                state = self._registry.get_service_state(agent_id, service_name)
                
                # 只处理 RECONNECTING 状态的服务
                if state != ServiceConnectionState.RECONNECTING:
                    continue
                
                metadata = self._registry.get_service_metadata(agent_id, service_name)
                if not metadata:
                    continue
                
                # 检查是否到达重连时间
                if metadata.next_retry_time and current_time >= metadata.next_retry_time:
                    # 获取重试次数
                    key = (agent_id, service_name)
                    retry_count = self._retry_counts.get(key, 0)
                    
                    # 检查是否超过最大重试次数
                    if retry_count >= self._max_retries:
                        logger.warning(
                            f"[RECONNECT] Max retries reached: {service_name} "
                            f"(retries={retry_count})"
                        )
                        # 转换到 UNREACHABLE 状态
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
                    
                    # 增加重试计数
                    self._retry_counts[key] = retry_count + 1
    
    async def _on_state_changed(self, event: ServiceStateChanged):
        """
        处理状态变更 - 重置重试计数器
        """
        key = (event.agent_id, event.service_name)
        
        # 如果服务成功连接，重置重试计数器
        if event.new_state == "HEALTHY":
            if key in self._retry_counts:
                logger.info(f"[RECONNECT] Service recovered, resetting retry count: {event.service_name}")
                del self._retry_counts[key]
        
        # 如果服务进入 RECONNECTING 状态，调度重连
        elif event.new_state == "RECONNECTING":
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
        key = (agent_id, service_name)
        retry_count = self._retry_counts.get(key, 0)
        
        # 计算重连延迟（指数退避）
        delay = self._calculate_reconnect_delay(retry_count)
        next_retry_time = datetime.now() + timedelta(seconds=delay)
        
        # 更新元数据
        metadata = self._registry.get_service_metadata(agent_id, service_name)
        if metadata:
            metadata.next_retry_time = next_retry_time
            metadata.reconnect_attempts = retry_count
        
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
        """转换到 UNREACHABLE 状态"""
        from mcpstore.core.events.service_events import ServiceStateChanged
        
        old_state = self._registry.get_service_state(agent_id, service_name)
        self._registry.set_service_state(agent_id, service_name, ServiceConnectionState.UNREACHABLE)
        
        # 发布状态变更事件
        event = ServiceStateChanged(
            agent_id=agent_id,
            service_name=service_name,
            old_state=old_state.value if old_state else "UNKNOWN",
            new_state="UNREACHABLE",
            reason="max_retries_exceeded",
            source="reconnection_scheduler"
        )
        await self._event_bus.publish(event)
        
        # 清理重试计数器
        key = (agent_id, service_name)
        if key in self._retry_counts:
            del self._retry_counts[key]

