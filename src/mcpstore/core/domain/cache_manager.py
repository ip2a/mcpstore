"""
缓存管理器 - 负责所有缓存操作

职责:
1. 监听 ServiceAddRequested 事件
2. 添加服务到缓存（事务性）
3. 发布 ServiceCached 事件
4. 监听 ServiceConnected 事件，更新缓存
"""

import asyncio
import logging
from dataclasses import dataclass, field
from typing import List, Callable

from mcpstore.core.events.event_bus import EventBus
from mcpstore.core.events.service_events import (
    ServiceAddRequested, ServiceCached, ServiceConnected, ServiceOperationFailed
)
from mcpstore.core.models.service import ServiceConnectionState

logger = logging.getLogger(__name__)


@dataclass
class CacheTransaction:
    """缓存事务 - 支持回滚"""
    agent_id: str
    operations: List[tuple[str, Callable, tuple]] = field(default_factory=list)
    
    def record(self, operation_name: str, rollback_func: Callable, *args):
        """记录操作（用于回滚）"""
        self.operations.append((operation_name, rollback_func, args))
    
    async def rollback(self):
        """回滚所有操作"""
        logger.warning(f"Rolling back {len(self.operations)} cache operations for agent {self.agent_id}")
        for op_name, rollback_func, args in reversed(self.operations):
            try:
                if asyncio.iscoroutinefunction(rollback_func):
                    await rollback_func(*args)
                else:
                    rollback_func(*args)
                logger.debug(f"Rolled back: {op_name}")
            except Exception as e:
                logger.error(f"Rollback failed for {op_name}: {e}")


class CacheManager:
    """
    缓存管理器
    
    职责:
    1. 监听 ServiceAddRequested 事件
    2. 添加服务到缓存（事务性）
    3. 发布 ServiceCached 事件
    4. 监听 ServiceConnected 事件，更新缓存
    """
    
    def __init__(self, event_bus: EventBus, registry: 'CoreRegistry', agent_locks: 'AgentLocks'):
        self._event_bus = event_bus
        self._registry = registry
        self._agent_locks = agent_locks
        
        # 订阅事件
        self._event_bus.subscribe(ServiceAddRequested, self._on_service_add_requested, priority=100)
        self._event_bus.subscribe(ServiceConnected, self._on_service_connected, priority=50)
        
        logger.info("CacheManager initialized and subscribed to events")
    
    async def _on_service_add_requested(self, event: ServiceAddRequested):
        """
        处理服务添加请求 - 立即添加到缓存
        """
        logger.info(f"[CACHE] Processing ServiceAddRequested: {event.service_name}")
        
        transaction = CacheTransaction(agent_id=event.agent_id)
        
        try:
            # 使用 per-agent 锁保证并发安全
            async with self._agent_locks.write(event.agent_id):
                # 1. 添加服务到缓存（INITIALIZING 状态）
                self._registry.add_service(
                    agent_id=event.agent_id,
                    name=event.service_name,
                    session=None,  # 暂无连接
                    tools=[],      # 暂无工具
                    service_config=event.service_config,
                    state=ServiceConnectionState.INITIALIZING
                )
                transaction.record(
                    "add_service",
                    self._registry.remove_service,
                    event.agent_id, event.service_name
                )
                
                # 2. 添加 Agent-Client 映射
                self._registry.add_agent_client_mapping(event.agent_id, event.client_id)
                transaction.record(
                    "add_agent_client_mapping",
                    self._registry.remove_agent_client_mapping,
                    event.agent_id, event.client_id
                )
                
                # 3. 添加 Client 配置
                self._registry.add_client_config(event.client_id, {
                    "mcpServers": {event.service_name: event.service_config}
                })
                transaction.record(
                    "add_client_config",
                    self._registry.remove_client_config,
                    event.client_id
                )
                
                # 4. 添加 Service-Client 映射
                self._registry.add_service_client_mapping(
                    event.agent_id, event.service_name, event.client_id
                )
                transaction.record(
                    "add_service_client_mapping",
                    self._registry.remove_service_client_mapping,
                    event.agent_id, event.service_name
                )
            
            logger.info(f"[CACHE] Service cached: {event.service_name}")
            
            # 发布成功事件
            cached_event = ServiceCached(
                agent_id=event.agent_id,
                service_name=event.service_name,
                client_id=event.client_id,
                cache_keys=[
                    f"service:{event.agent_id}:{event.service_name}",
                    f"agent_client:{event.agent_id}:{event.client_id}",
                    f"client_config:{event.client_id}",
                    f"service_client:{event.agent_id}:{event.service_name}"
                ]
            )
            await self._event_bus.publish(cached_event)
            
        except Exception as e:
            logger.error(f"[CACHE] Failed to cache service {event.service_name}: {e}", exc_info=True)
            
            # 回滚事务
            await transaction.rollback()
            
            # 发布失败事件
            error_event = ServiceOperationFailed(
                agent_id=event.agent_id,
                service_name=event.service_name,
                operation="cache",
                error_message=str(e),
                original_event=event
            )
            await self._event_bus.publish(error_event)
    
    async def _on_service_connected(self, event: ServiceConnected):
        """
        处理服务连接成功 - 更新缓存中的 session 和 tools
        """
        logger.info(f"[CACHE] Updating cache for connected service: {event.service_name}")
        
        try:
            async with self._agent_locks.write(event.agent_id):
                # 清理旧的工具缓存（如果存在）
                existing_session = self._registry.get_session(event.agent_id, event.service_name)
                if existing_session:
                    self._registry.clear_service_tools_only(event.agent_id, event.service_name)
                
                # 更新服务缓存（保留映射）
                self._registry.add_service(
                    agent_id=event.agent_id,
                    name=event.service_name,
                    session=event.session,
                    tools=event.tools,
                    preserve_mappings=True  # 保留已有的映射关系
                )
            
            logger.info(f"[CACHE] Cache updated for {event.service_name} with {len(event.tools)} tools")

            # 工具缓存更新完成后：统一触发器（强一致）
            try:
                # 优先获取全局 agent id
                cm = getattr(self, 'orchestrator', None)
                gid = getattr(getattr(cm, 'client_manager', None), 'global_agent_store_id', None) or event.agent_id
                if hasattr(self._registry, 'tools_changed'):
                    self._registry.tools_changed(gid, aggressive=True)
                logger.debug(f"[SNAPSHOT] cache_manager: tools_changed triggered service={event.service_name}")
            except Exception as e:
                logger.warning(f"[SNAPSHOT] cache_manager: tools_changed failed: {e}")
            
        except Exception as e:
            logger.error(f"[CACHE] Failed to update cache for {event.service_name}: {e}", exc_info=True)
            
            # 发布失败事件
            error_event = ServiceOperationFailed(
                agent_id=event.agent_id,
                service_name=event.service_name,
                operation="cache_update",
                error_message=str(e),
                original_event=event
            )
            await self._event_bus.publish(error_event)

