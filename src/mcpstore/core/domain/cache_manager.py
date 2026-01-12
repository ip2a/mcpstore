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
    ServiceAddRequested,
    ServiceBootstrapRequested,
    ServiceBootstrapped,
    ServiceBootstrapFailed,
    ServiceCached,
    ServiceConnected,
    ServicePersisting,
    ServicePersisted,
    ToolSyncStarted,
    ToolSyncCompleted,
    ServiceOperationFailed,
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
        self._event_bus.subscribe(ServiceBootstrapRequested, self._on_service_bootstrap_requested, priority=100)
        self._event_bus.subscribe(ServiceConnected, self._on_service_connected, priority=50)
        
        logger.info("CacheManager initialized and subscribed to events")

    async def _on_service_bootstrap_requested(self, event: ServiceBootstrapRequested):
        """
        处理 setup/bootstrap 场景的服务重放：只构建缓存与关系，不触发阻塞链路
        """
        logger.info(f"[CACHE] [BOOTSTRAP] Processing ServiceBootstrapRequested: {event.service_name}")
        logger.debug(f"[CACHE] [BOOTSTRAP] agent_id={event.agent_id}, client_id={event.client_id}, global_name={event.global_name}")
        origin_agent = event.origin_agent_id or event.agent_id
        origin_local_name = event.origin_local_name or event.service_name
        global_name = event.global_name or event.service_name
        global_agent_id = self._registry._naming.GLOBAL_AGENT_STORE

        transaction = CacheTransaction(agent_id=origin_agent)

        try:
            async with self._agent_locks.write(
                origin_agent,
                operation="cache_on_service_bootstrap_requested"
            ):
                await self._registry._ensure_agent_entity(origin_agent)
                await self._registry._ensure_agent_entity(global_agent_id)

                # 写入服务实体与初始状态
                await self._registry.add_service_async(
                    agent_id=global_agent_id,
                    name=global_name,
                    session=None,
                    tools=[],
                    service_config=event.service_config,
                    state=ServiceConnectionState.STARTUP
                )
                transaction.record(
                    "add_service_global_bootstrap",
                    self._registry.remove_service_async,
                    global_agent_id, global_name
                )

                # 建立 Agent-Service 关系
                await self._registry._relation_manager.add_agent_service(
                    agent_id=origin_agent,
                    service_original_name=origin_local_name,
                    service_global_name=global_name,
                    client_id=event.client_id
                )
                transaction.record(
                    "add_agent_service_bootstrap",
                    self._registry._relation_manager.remove_agent_service,
                    origin_agent, global_name
                )

                # 设置 service-client 映射（双向）
                await self._registry.set_service_client_mapping_async(origin_agent, origin_local_name, event.client_id)
                transaction.record(
                    "set_service_client_mapping_origin_bootstrap",
                    self._registry.remove_service_client_mapping,
                    origin_agent, origin_local_name
                )
                await self._registry.set_service_client_mapping_async(global_agent_id, global_name, event.client_id)
                transaction.record(
                    "set_service_client_mapping_global_bootstrap",
                    self._registry.remove_service_client_mapping,
                    global_agent_id, global_name
                )

                # 发布 bootstrap 完成事件，交给生命周期与健康组件后台处理
                bootstrapped = ServiceBootstrapped(
                    agent_id=origin_agent,
                    service_name=origin_local_name,
                    client_id=event.client_id,
                    global_name=global_name,
                    source=event.source,
                    service_config=event.service_config
                )
                await self._event_bus.publish(bootstrapped, wait=False)

        except Exception as e:
            logger.error(f"[CACHE] [BOOTSTRAP] Failed to cache service {event.service_name}: {e}", exc_info=True)
            await transaction.rollback()

            failed_event = ServiceBootstrapFailed(
                agent_id=origin_agent,
                service_name=origin_local_name,
                error_message=str(e),
                source=event.source,
                original_event=event
            )
            try:
                await self._event_bus.publish(failed_event, wait=False)
            except Exception as pub_err:
                logger.error(f"[CACHE] [BOOTSTRAP] Failed to publish ServiceBootstrapFailed: {pub_err}")
            return

    async def _on_service_add_requested(self, event: ServiceAddRequested):
        """
        Handle service add request - immediately add to cache
        """
        logger.info(f"[CACHE] Processing ServiceAddRequested: {event.service_name}")
        logger.debug(f"[CACHE] Event details: agent_id={event.agent_id}, client_id={event.client_id}, global_name={getattr(event, 'global_name', '')}")
        logger.debug(f"[CACHE] Service config keys: {list(event.service_config.keys()) if event.service_config else 'None'}")
        origin_agent = event.origin_agent_id or event.agent_id
        origin_local_name = event.origin_local_name or event.service_name
        global_name = event.global_name or event.service_name
        global_agent_id = self._registry._naming.GLOBAL_AGENT_STORE

        transaction = CacheTransaction(agent_id=origin_agent)
        
        try:
            # 使用 per-agent 锁保证并发安全
            async with self._agent_locks.write(
                origin_agent, 
                operation="cache_on_service_add_requested"
            ):
                # 确保 Agent 实体存在（来源 Agent + 全局 Agent）
                await self._registry._ensure_agent_entity(origin_agent)
                await self._registry._ensure_agent_entity(global_agent_id)

                # 1. 添加服务到缓存（全局视角，STARTUP 状态）
                await self._registry.add_service_async(
                    agent_id=global_agent_id,
                    name=global_name,
                    session=None,  # 暂无连接
                    tools=[],      # 暂无工具
                    service_config=event.service_config,
                    state=ServiceConnectionState.STARTUP
                )
                transaction.record(
                    "add_service_global",
                    self._registry.remove_service_async,
                    global_agent_id, global_name
                )

                # 2. 建立 Agent-Service 关系
                await self._registry._relation_manager.add_agent_service(
                    agent_id=origin_agent,
                    service_original_name=origin_local_name,
                    service_global_name=global_name,
                    client_id=event.client_id
                )
                transaction.record(
                    "add_agent_service",
                    self._registry._relation_manager.remove_agent_service,
                    origin_agent, global_name
                )

                # 3. 添加 Service-Client 映射（Agent 与 Global）
                logger.debug(f"[CACHE] Adding service-client mapping: {origin_agent}:{origin_local_name} -> {event.client_id}")
                await self._registry.set_service_client_mapping_async(
                    origin_agent, origin_local_name, event.client_id
                )
                await self._registry.set_service_client_mapping_async(
                    global_agent_id, global_name, event.client_id
                )
                transaction.record(
                    "set_service_client_mapping_agent",
                    self._registry.delete_service_client_mapping_async,
                    origin_agent, origin_local_name
                )
                transaction.record(
                    "set_service_client_mapping_global",
                    self._registry.delete_service_client_mapping_async,
                    global_agent_id, global_name
                )

                # 立即验证映射是否成功建立（使用异步版本）
                verify_client_id = await self._registry.get_service_client_id_async(origin_agent, origin_local_name)
                if verify_client_id != event.client_id:
                    error_msg = (
                        f"Service-client mapping verification failed! "
                        f"Expected: {event.client_id}, Got: {verify_client_id}"
                    )
                    logger.error(f"[CACHE] {error_msg}")
                    raise RuntimeError(error_msg)
                logger.debug(f"[CACHE] Service-client mapping verified: {origin_agent}:{origin_local_name} -> {verify_client_id}")

            logger.info(f"[CACHE] Service cached: {event.service_name}")
            logger.debug(f"[CACHE] Verification - client_id mapping: {verify_client_id}")
            
            # 注意：在新架构中，client_config 不再单独存储
            # 服务配置已经存储在服务实体中（service_entity.config）
            
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

            # 仅负责缓存与事件发布；连接请求由 orchestrator/connection_manager 统一触发
            
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
            tool_count = len(event.tools)
            try:
                if self._event_bus.get_subscriber_count(ServicePersisting) > 0:
                    await self._event_bus.publish(
                        ServicePersisting(
                            agent_id=event.agent_id,
                            service_name=event.service_name,
                            stage="cache",
                            tool_count=tool_count,
                            source_event=event
                        ),
                        wait=False
                    )
            except Exception as persist_evt_err:
                logger.debug(f"[CACHE] Failed to publish ServicePersisting: {persist_evt_err}")

            async with self._agent_locks.write(
                event.agent_id, 
                operation="cache_on_service_connected"
            ):
                # 从 pykv 读取现有服务配置（保持配置不丢失）
                # 这是关键：ServiceConnected 事件中没有 service_config 字段，
                # 必须从 pykv 读取已有配置，否则 add_service_async 会用空字典覆盖
                service_global_name = self._registry._naming.generate_service_global_name(
                    event.service_name, event.agent_id
                )
                service_entity = await self._registry._cache_service_manager.get_service(
                    service_global_name
                )
                if service_entity is None:
                    raise RuntimeError(
                        f"Service entity does not exist, cannot update cache: "
                        f"service_name={event.service_name}, agent_id={event.agent_id}, "
                        f"global_name={service_global_name}"
                    )
                existing_config = service_entity.config
                if not existing_config:
                    raise RuntimeError(
                        f"Service configuration is empty, data inconsistency: "
                        f"service_name={event.service_name}, agent_id={event.agent_id}, "
                        f"global_name={service_global_name}"
                    )
                
                # 清理旧的工具缓存（如果存在）
                existing_session = self._registry.get_session(event.agent_id, event.service_name)
                if existing_session:
                    self._registry.clear_service_tools_only(event.agent_id, event.service_name)

                # 工具同步开始事件（监控）
                try:
                    if self._event_bus.get_subscriber_count(ToolSyncStarted) > 0:
                        await self._event_bus.publish(
                            ToolSyncStarted(
                                agent_id=event.agent_id,
                                service_name=event.service_name,
                                total_tools=tool_count
                            ),
                            wait=False
                        )
                except Exception as tool_evt_err:
                    logger.debug(f"[CACHE] Failed to publish ToolSyncStarted: {tool_evt_err}")

                # 更新会话（不触发新增服务的初始化逻辑）
                if self._registry._session_manager:
                    self._registry._session_manager.set_session(
                        event.agent_id, event.service_name, event.session
                    )
                
                # 创建工具实体和 Service-Tool 关系（写入实体层和关系层）
                # 这是 list_tools 链路能正确获取工具的关键
                await self._create_tool_entities_and_relations(
                    event.agent_id,
                    event.service_name,
                    event.tools
                )

                # 工具同步完成事件（监控）
                try:
                    if self._event_bus.get_subscriber_count(ToolSyncCompleted) > 0:
                        await self._event_bus.publish(
                            ToolSyncCompleted(
                                agent_id=event.agent_id,
                                service_name=event.service_name,
                                total_tools=tool_count
                            ),
                            wait=False
                        )
                except Exception as tool_evt_err:
                    logger.debug(f"[CACHE] Failed to publish ToolSyncCompleted: {tool_evt_err}")
                
                # 更新服务状态（写入状态层）
                # 关键：这里写入完整的工具状态，LifecycleManager 只更新健康状态
                await self._update_service_status(
                    event.agent_id,
                    event.service_name,
                    event.tools
                )

                # 缓存落盘完成事件（工具与状态已写入）
                try:
                    if self._event_bus.get_subscriber_count(ServicePersisted) > 0:
                        await self._event_bus.publish(
                            ServicePersisted(
                                agent_id=event.agent_id,
                                service_name=event.service_name,
                                stage="cache",
                                tool_count=tool_count,
                                details={"health_status": "healthy"}
                            ),
                            wait=False
                        )
                except Exception as persist_evt_err:
                    logger.debug(f"[CACHE] Failed to publish ServicePersisted(cache): {persist_evt_err}")
            
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
            f"[CACHE] Creating tool entities and relations: agent_id={agent_id}, "
            f"service_name={service_name}, service_global_name={service_global_name}, "
            f"tools_count={len(tools)}"
        )
        
        # 获取必要的管理器
        tool_entity_manager = self._registry._cache_tool_manager
        relation_manager = self._registry._relation_manager
        
        if tool_entity_manager is None:
            raise RuntimeError(
                f"ToolEntityManager not initialized, cannot create tool entities: "
                f"service_global_name={service_global_name}"
            )
        
        if relation_manager is None:
            raise RuntimeError(
                f"RelationshipManager not initialized, cannot create Service-Tool relation: "
                f"service_global_name={service_global_name}"
            )
        
        # 遍历工具列表，创建实体和关系
        for tool_name, tool_def in tools:
            # 提取工具原始名称（去除服务前缀），保证用于全局名的基准是 FastMCP 标准格式
            from mcpstore.core.logic.tool_logic import ToolLogicCore
            original_tool_name = ToolLogicCore.extract_original_tool_name(
                tool_name,
                service_global_name,
                service_name
            )

            # 生成工具全局名称（基于去前缀后的原始名，避免重复前缀）
            tool_global_name = self._registry._naming.generate_tool_global_name(
                service_global_name, original_tool_name
            )

            logger.debug(
                f"[CACHE] Creating tool: tool_name={tool_name}, "
                f"tool_global_name={tool_global_name}, original={original_tool_name}"
            )

            # 1. 创建工具实体（写入实体层）
            await tool_entity_manager.create_tool(
                service_global_name=service_global_name,
                service_original_name=service_name,
                source_agent=agent_id,
                tool_original_name=original_tool_name,
                tool_def=tool_def
            )
            
            # 2. 创建 Service-Tool 关系（写入关系层）
            await relation_manager.add_service_tool(
                service_global_name=service_global_name,
                service_original_name=service_name,
                source_agent=agent_id,
                tool_global_name=tool_global_name,
                tool_original_name=original_tool_name
            )
        
        logger.info(
            f"[CACHE] Tool entities and relations created successfully: service_global_name={service_global_name}, "
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
            f"[CACHE] Updating service status: agent_id={agent_id}, "
            f"service_name={service_name}, service_global_name={service_global_name}, "
            f"tools_count={len(tools)}"
        )

        # 构建工具状态列表（所有工具默认 available）
        tools_status = []
        for tool_name, tool_def in tools:
            # 提取工具原始名称（去除服务前缀）
            # 注意：MCP 服务返回的工具名称可能已经带有服务前缀
            # 例如：mcpstore_get_current_weather -> get_current_weather
            from mcpstore.core.logic.tool_logic import ToolLogicCore
            original_tool_name = ToolLogicCore.extract_original_tool_name(
                tool_name,
                service_global_name,
                service_name
            )

            # 生成工具全局名称（基于去前缀后的原始名，避免重复前缀）
            tool_global_name = self._registry._naming.generate_tool_global_name(
                service_global_name, original_tool_name
            )
            
            tools_status.append({
                "tool_global_name": tool_global_name,
                "tool_original_name": original_tool_name,
                "status": "available"
            })
        
        # 获取 CacheStateManager（pykv 唯一真相数据源）
        state_manager = self._registry._cache_state_manager
        
        if state_manager is None:
            raise RuntimeError(
                f"CacheStateManager not initialized, cannot update service status: "
                f"service_global_name={service_global_name}"
            )
        
        await state_manager.update_service_status(
            service_global_name=service_global_name,
            health_status="healthy",
            tools_status=tools_status
        )
        
        logger.info(
            f"[CACHE] Service status updated successfully: service_global_name={service_global_name}, "
            f"tools_count={len(tools_status)}"
        )
