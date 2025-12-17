"""
Cache Manager - Responsible for all cache operations

Responsibilities:
1. Listen to ServiceAddRequested events
2. Add services to cache (transactional)
3. Publish ServiceCached events
4. Listen to ServiceConnected events, update cache
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
    """Cache transaction - supports rollback"""
    agent_id: str
    operations: List[tuple[str, Callable, tuple]] = field(default_factory=list)
    
    def record(self, operation_name: str, rollback_func: Callable, *args):
        """Record operation (for rollback)"""
        self.operations.append((operation_name, rollback_func, args))
    
    async def rollback(self):
        """Rollback all operations"""
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
    Cache Manager

    Responsibilities:
    1. Listen to ServiceAddRequested events
    2. Add services to cache (transactional)
    3. Publish ServiceCached events
    4. Listen to ServiceConnected events, update cache
    """
    
    def __init__(self, event_bus: EventBus, registry: 'CoreRegistry', agent_locks: 'AgentLocks'):
        self._event_bus = event_bus
        self._registry = registry
        self._agent_locks = agent_locks
        
        # Subscribe to events
        self._event_bus.subscribe(ServiceAddRequested, self._on_service_add_requested, priority=100)
        self._event_bus.subscribe(ServiceConnected, self._on_service_connected, priority=50)
        
        logger.info("CacheManager initialized and subscribed to events")
    
    async def _on_service_add_requested(self, event: ServiceAddRequested):
        """
        Handle service add request - immediately add to cache
        """
        logger.info(f"[CACHE] Processing ServiceAddRequested: {event.service_name}")
        logger.debug(f"[CACHE] Event details: agent_id={event.agent_id}, client_id={event.client_id}")
        logger.debug(f"[CACHE] Service config keys: {list(event.service_config.keys()) if event.service_config else 'None'}")
        
        transaction = CacheTransaction(agent_id=event.agent_id)
        
        try:
            # 使用 per-agent 锁保证并发安全
            async with self._agent_locks.write(event.agent_id):
                # 1. 添加服务到缓存（INITIALIZING 状态）
                # 使用异步版本避免事件循环冲突
                await self._registry.add_service_async(
                    agent_id=event.agent_id,
                    name=event.service_name,
                    session=None,  # 暂无连接
                    tools=[],      # 暂无工具
                    service_config=event.service_config,
                    state=ServiceConnectionState.INITIALIZING
                )
                transaction.record(
                    "add_service",
                    self._registry.remove_service_async,
                    event.agent_id, event.service_name
                )
                
                # 2. 添加 Agent-Client 映射
                self._registry._agent_client_service.add_agent_client_mapping(event.agent_id, event.client_id)
                transaction.record(
                    "add_agent_client_mapping",
                    self._registry._agent_client_service.remove_agent_client_mapping,
                    event.agent_id, event.client_id
                )
                
                # 3. 添加 Client 配置
                self._registry._client_config_service.add_client_config(event.client_id, {
                    "mcpServers": {event.service_name: event.service_config}
                })
                transaction.record(
                    "add_client_config",
                    self._registry._client_config_service.remove_client_config,
                    event.client_id
                )
                
                # 4. 添加 Service-Client 映射
                logger.debug(f"[CACHE] Adding service-client mapping: {event.agent_id}:{event.service_name} -> {event.client_id}")
                self._registry._agent_client_service.add_service_client_mapping(
                    event.agent_id, event.service_name, event.client_id
                )
                transaction.record(
                    "add_service_client_mapping",
                    self._registry._agent_client_service.remove_service_client_mapping,
                    event.agent_id, event.service_name
                )

                # 立即验证映射是否成功建立
                verify_client_id = self._registry._agent_client_service.get_service_client_id(event.agent_id, event.service_name)
                if verify_client_id != event.client_id:
                    error_msg = (
                        f"Service-client mapping verification failed! "
                        f"Expected: {event.client_id}, Got: {verify_client_id}"
                    )
                    logger.error(f"[CACHE] {error_msg}")
                    raise RuntimeError(error_msg)
                logger.debug(f"[CACHE] Service-client mapping verified: {event.agent_id}:{event.service_name} -> {verify_client_id}")

            logger.info(f"[CACHE] Service cached: {event.service_name}")
            logger.debug(f"[CACHE] Verification - client_id mapping: {verify_client_id}")
            
            verify_config = self._registry._client_config_service.get_client_config_from_cache(event.client_id)
            logger.debug(f"[CACHE] Verification - client config exists: {verify_config is not None}")
            
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
            logger.info(f"[CACHE] Publishing ServiceCached event for {event.service_name}")
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
        
        关键职责：
        1. 更新服务的 session
        2. 创建工具实体（写入实体层）
        3. 创建 Service-Tool 关系（写入关系层）
        4. 更新服务状态（写入状态层）
        """
        logger.info(f"[CACHE] Updating cache for connected service: {event.service_name}")
        
        try:
            async with self._agent_locks.write(event.agent_id):
                # 清理旧的工具缓存（如果存在）
                existing_session = self._registry.get_session(event.agent_id, event.service_name)
                if existing_session:
                    self._registry.clear_service_tools_only(event.agent_id, event.service_name)
                
                # 更新服务缓存（保留映射）
                await self._registry.add_service_async(
                    agent_id=event.agent_id,
                    name=event.service_name,
                    session=event.session,
                    tools=event.tools,
                    preserve_mappings=True
                )
                
                # 创建工具实体和 Service-Tool 关系（写入实体层和关系层）
                # 这是 list_tools 链路能正确获取工具的关键
                await self._create_tool_entities_and_relations(
                    event.agent_id,
                    event.service_name,
                    event.tools
                )
                
                # 更新服务状态（写入状态层）
                await self._update_service_status(
                    event.agent_id,
                    event.service_name,
                    event.tools
                )
            
            logger.info(f"[CACHE] Cache updated for {event.service_name} with {len(event.tools)} tools")

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

    async def _create_tool_entities_and_relations(
        self,
        agent_id: str,
        service_name: str,
        tools: list
    ) -> None:
        """
        创建工具实体和 Service-Tool 关系
        
        写入实体层和关系层，这是 list_tools 链路能正确获取工具的关键。
        
        Args:
            agent_id: Agent ID
            service_name: 服务名称
            tools: 工具列表 [(tool_name, tool_def), ...]
            
        Raises:
            RuntimeError: 如果必要的管理器未初始化
        """
        # 获取服务的全局名称
        service_global_name = self._registry._naming.generate_service_global_name(
            service_name, agent_id
        )
        
        logger.info(
            f"[CACHE] 创建工具实体和关系: agent_id={agent_id}, "
            f"service_name={service_name}, service_global_name={service_global_name}, "
            f"tools_count={len(tools)}"
        )
        
        # 获取必要的管理器
        tool_entity_manager = self._registry._cache_tool_manager
        relation_manager = self._registry._relation_manager
        
        if tool_entity_manager is None:
            raise RuntimeError(
                f"ToolEntityManager 未初始化，无法创建工具实体: "
                f"service_global_name={service_global_name}"
            )
        
        if relation_manager is None:
            raise RuntimeError(
                f"RelationshipManager 未初始化，无法创建 Service-Tool 关系: "
                f"service_global_name={service_global_name}"
            )
        
        # 遍历工具列表，创建实体和关系
        for tool_name, tool_def in tools:
            # 生成工具全局名称
            tool_global_name = self._registry._naming.generate_tool_global_name(
                service_global_name, tool_name
            )
            
            logger.debug(
                f"[CACHE] 创建工具: tool_name={tool_name}, "
                f"tool_global_name={tool_global_name}"
            )
            
            # 1. 创建工具实体（写入实体层）
            await tool_entity_manager.create_tool(
                service_global_name=service_global_name,
                service_original_name=service_name,
                source_agent=agent_id,
                tool_original_name=tool_name,
                tool_def=tool_def
            )
            
            # 2. 创建 Service-Tool 关系（写入关系层）
            await relation_manager.add_service_tool(
                service_global_name=service_global_name,
                service_original_name=service_name,
                source_agent=agent_id,
                tool_global_name=tool_global_name,
                tool_original_name=tool_name
            )
        
        logger.info(
            f"[CACHE] 工具实体和关系创建成功: service_global_name={service_global_name}, "
            f"tools_count={len(tools)}"
        )

    async def _update_service_status(
        self,
        agent_id: str,
        service_name: str,
        tools: list
    ) -> None:
        """
        更新服务状态到 pykv 状态层
        
        Args:
            agent_id: Agent ID
            service_name: 服务名称
            tools: 工具列表 [(tool_name, tool_def), ...]
            
        Raises:
            RuntimeError: 如果 CacheStateManager 未初始化
        """
        # 获取服务的全局名称
        service_global_name = self._registry._naming.generate_service_global_name(
            service_name, agent_id
        )
        
        logger.debug(
            f"[CACHE] 更新服务状态: agent_id={agent_id}, "
            f"service_name={service_name}, service_global_name={service_global_name}, "
            f"tools_count={len(tools)}"
        )

        # region agent log
        try:
            import json, time
            from pathlib import Path
            log_path = Path("/home/yuuu/app/2025/2025_6/mcpstore/.cursor/debug.log")
            tool_names = [t[0] for t in tools]
            log_record = {
                "sessionId": "debug-session",
                "runId": "pre-fix",
                "hypothesisId": "H1",
                "location": "cache_manager.py:_update_service_status",
                "message": "before_update_service_status",
                "data": {
                    "agent_id": agent_id,
                    "service_name": service_name,
                    "service_global_name": service_global_name,
                    "tools_count": len(tools),
                    "tool_names": tool_names,
                },
                "timestamp": int(time.time() * 1000),
            }
            log_path.parent.mkdir(parents=True, exist_ok=True)
            with log_path.open("a", encoding="utf-8") as f:
                f.write(json.dumps(log_record, ensure_ascii=False) + "\n")
        except Exception:
            # 调试日志失败不影响主流程
            pass
        # endregion
        
        # 构建工具状态列表（所有工具默认 available）
        tools_status = []
        for tool_name, tool_def in tools:
            # 生成工具全局名称
            tool_global_name = self._registry._naming.generate_tool_global_name(
                service_global_name, tool_name
            )
            
            tools_status.append({
                "tool_global_name": tool_global_name,
                "tool_original_name": tool_name,
                "status": "available"
            })
        
        # 获取 CacheStateManager（pykv 唯一真相数据源）
        state_manager = self._registry._cache_state_manager
        
        if state_manager is None:
            raise RuntimeError(
                f"CacheStateManager 未初始化，无法更新服务状态: "
                f"service_global_name={service_global_name}"
            )
        
        await state_manager.update_service_status(
            service_global_name=service_global_name,
            health_status="healthy",
            tools_status=tools_status
        )
        
        logger.info(
            f"[CACHE] 服务状态更新成功: service_global_name={service_global_name}, "
            f"tools_count={len(tools_status)}"
        )
