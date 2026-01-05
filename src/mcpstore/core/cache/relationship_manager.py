"""
关系管理器

负责管理实体间的关系映射：
- Agent-Service 关系
- Service-Tool 关系
"""

import logging
import time
from typing import Any, Dict, List, TYPE_CHECKING

from .models import (
    AgentServiceRelation,
    ServiceRelationItem,
    ServiceToolRelation,
    ToolRelationItem
)

if TYPE_CHECKING:
    from .cache_layer_manager import CacheLayerManager

logger = logging.getLogger(__name__)


class RelationshipManager:
    """
    关系管理器
    
    管理实体间的关系映射，包括：
    - Agent-Service 关系（key 是 agent_id）
    - Service-Tool 关系（key 是 service_global_name）
    """
    
    def __init__(self, cache_layer: 'CacheLayerManager'):
        """
        初始化关系管理器
        
        Args:
            cache_layer: 缓存层管理器实例
        """
        self._cache_layer = cache_layer
        logger.debug("[RELATIONSHIP] Initializing RelationshipManager")
    
    # ==================== Agent-Service 关系管理 ====================
    
    async def add_agent_service(
        self,
        agent_id: str,
        service_original_name: str,
        service_global_name: str,
        client_id: str
    ) -> None:
        """
        添加 Agent-Service 关系
        
        Args:
            agent_id: Agent ID
            service_original_name: 服务原始名称
            service_global_name: 服务全局名称
            client_id: 客户端 ID
            
        Raises:
            ValueError: 如果参数无效
            KeyError: 如果服务实体不存在
            RuntimeError: 如果添加失败
        """
        if not agent_id:
            raise ValueError("Agent ID cannot be empty")
        if not service_original_name:
            raise ValueError("Service original name cannot be empty")
        if not service_global_name:
            raise ValueError("Service global name cannot be empty")
        if not client_id:
            raise ValueError("Client ID cannot be empty")
        
        # 验证服务实体存在
        service_entity = await self._cache_layer.get_entity(
            "services",
            service_global_name
        )
        if service_entity is None:
            raise KeyError(
                f"Service entity does not exist: service_global_name={service_global_name}"
            )
        
        logger.debug(
            f"[RELATIONSHIP] Adding Agent-Service relation: agent_id={agent_id}, "
            f"service_original_name={service_original_name}, "
            f"service_global_name={service_global_name}, client_id={client_id}"
        )
        
        # 获取现有关系
        relation_data = await self._cache_layer.get_relation(
            "agent_services",
            agent_id
        )
        
        if relation_data is None:
            # 创建新关系
            relation = AgentServiceRelation(services=[])
        else:
            # 解析现有关系
            relation = AgentServiceRelation.from_dict(relation_data)
        
        # 检查服务是否已存在（基于全局名称判断）
        for i, service in enumerate(relation.services):
            if service.service_global_name == service_global_name:
                # 全局名称相同，认为是同一个关系，更新配置
                relation.services[i] = ServiceRelationItem(
                    service_original_name=service_original_name,
                    service_global_name=service_global_name,
                    client_id=client_id,
                    established_time=service.established_time,
                    last_access=int(time.time())
                )
                
                await self._cache_layer.put_relation(
                    "agent_services",
                    agent_id,
                    relation.to_dict()
                )
                
                logger.info(
                    f"[RELATIONSHIP] Updated Agent-Service relation: agent_id={agent_id}, "
                    f"service_global_name={service_global_name}"
                )
                return
        
        # 添加新服务
        current_time = int(time.time())
        new_service = ServiceRelationItem(
            service_original_name=service_original_name,
            service_global_name=service_global_name,
            client_id=client_id,
            established_time=current_time,
            last_access=current_time
        )
        relation.services.append(new_service)
        
        # 保存关系
        await self._cache_layer.put_relation(
            "agent_services",
            agent_id,
            relation.to_dict()
        )
        
        logger.info(
            f"[RELATIONSHIP] Successfully added Agent-Service relation: agent_id={agent_id}, "
            f"service_global_name={service_global_name}"
        )
    
    async def remove_agent_service(
        self,
        agent_id: str,
        service_global_name: str
    ) -> None:
        """
        移除 Agent-Service 关系
        
        Args:
            agent_id: Agent ID
            service_global_name: 服务全局名称
            
        Raises:
            ValueError: 如果参数无效
            KeyError: 如果关系不存在
            RuntimeError: 如果移除失败
        """
        if not agent_id:
            raise ValueError("Agent ID cannot be empty")
        if not service_global_name:
            raise ValueError("Service global name cannot be empty")
        
        logger.debug(
            f"[RELATIONSHIP] Removing Agent-Service relation: agent_id={agent_id}, "
            f"service_global_name={service_global_name}"
        )
        
        # 获取现有关系
        relation_data = await self._cache_layer.get_relation(
            "agent_services",
            agent_id
        )
        
        if relation_data is None:
            raise KeyError(
                f"Agent relation does not exist: agent_id={agent_id}"
            )
        
        # 解析关系
        relation = AgentServiceRelation.from_dict(relation_data)
        
        # 查找并移除服务
        original_count = len(relation.services)
        relation.services = [
            service for service in relation.services
            if service.service_global_name != service_global_name
        ]
        
        if len(relation.services) == original_count:
            raise KeyError(
                f"Service does not exist in Agent relation: agent_id={agent_id}, "
                f"service_global_name={service_global_name}"
            )
        
        # 保存更新后的关系
        if len(relation.services) == 0:
            # 如果没有服务了，删除整个关系
            await self._cache_layer.delete_relation("agent_services", agent_id)
            logger.info(
                f"[RELATIONSHIP] Deleted empty Agent relation: agent_id={agent_id}"
            )
        else:
            # 保存更新后的关系
            await self._cache_layer.put_relation(
                "agent_services",
                agent_id,
                relation.to_dict()
            )
            logger.info(
                f"[RELATIONSHIP] Successfully removed Agent-Service relation: "
                f"agent_id={agent_id}, service_global_name={service_global_name}"
            )
    
    async def get_agent_services(
        self,
        agent_id: str
    ) -> List[Dict[str, Any]]:
        """
        获取 Agent 的所有服务关系
        
        Args:
            agent_id: Agent ID
            
        Returns:
            服务关系列表，如果不存在返回空列表
            
        Raises:
            ValueError: 如果参数无效
            RuntimeError: 如果获取失败
        """
        if not agent_id:
            raise ValueError("Agent ID cannot be empty")
        
        logger.debug(
            f"[RELATIONSHIP] Getting Agent service relations: agent_id={agent_id}"
        )
        
        # 获取关系
        relation_data = await self._cache_layer.get_relation(
            "agent_services",
            agent_id
        )
        
        if relation_data is None:
            logger.debug(
                f"[RELATIONSHIP] Agent relation does not exist: agent_id={agent_id}"
            )
            return []
        
        # 解析关系
        relation = AgentServiceRelation.from_dict(relation_data)
        
        # 转换为字典列表
        services = [service.to_dict() for service in relation.services]
        
        logger.debug(
            f"[RELATIONSHIP] Retrieved {len(services)} service relations: "
            f"agent_id={agent_id}"
        )
        
        return services
    
    # ==================== Service-Tool 关系管理 ====================
    
    async def add_service_tool(
        self,
        service_global_name: str,
        service_original_name: str,
        source_agent: str,
        tool_global_name: str,
        tool_original_name: str
    ) -> None:
        """
        添加 Service-Tool 关系
        
        Args:
            service_global_name: 服务全局名称
            service_original_name: 服务原始名称
            source_agent: 来源 Agent
            tool_global_name: 工具全局名称
            tool_original_name: 工具原始名称
            
        Raises:
            ValueError: 如果参数无效
            KeyError: 如果工具实体不存在
            RuntimeError: 如果添加失败
        """
        if not service_global_name:
            raise ValueError("Service global name cannot be empty")
        if not service_original_name:
            raise ValueError("Service original name cannot be empty")
        if not source_agent:
            raise ValueError("Source Agent cannot be empty")
        if not tool_global_name:
            raise ValueError("Tool global name cannot be empty")
        if not tool_original_name:
            raise ValueError("Tool original name cannot be empty")
        
        # 注意：不在这里验证工具实体存在性
        # 原因：
        # 1. 在 cache_manager._create_tool_entities_and_relations 中，
        #    create_tool 和 add_service_tool 是顺序调用的
        # 2. Redis 写入后可能存在短暂的读取延迟
        # 3. 关系层和实体层是独立的，关系层不应该依赖实体层的即时可读性
        # 4. 调用方负责确保工具实体已创建
        
        logger.debug(
            f"[RELATIONSHIP] Adding Service-Tool relation: "
            f"service_global_name={service_global_name}, "
            f"tool_global_name={tool_global_name}"
        )
        
        # 获取现有关系
        relation_data = await self._cache_layer.get_relation(
            "service_tools",
            service_global_name
        )
        
        if relation_data is None:
            # 创建新关系
            relation = ServiceToolRelation(
                service_global_name=service_global_name,
                service_original_name=service_original_name,
                source_agent=source_agent,
                tools=[]
            )
        else:
            # 解析现有关系
            relation = ServiceToolRelation.from_dict(relation_data)
        
        # 检查工具是否已存在（基于全局名称判断）
        for i, tool in enumerate(relation.tools):
            if tool.tool_global_name == tool_global_name:
                # 全局名称相同，认为是同一个关系，更新配置
                relation.tools[i] = ToolRelationItem(
                    tool_global_name=tool_global_name,
                    tool_original_name=tool_original_name
                )
                
                await self._cache_layer.put_relation(
                    "service_tools",
                    service_global_name,
                    relation.to_dict()
                )
                
                logger.info(
                    f"[RELATIONSHIP] Updated Service-Tool relation: "
                    f"service_global_name={service_global_name}, "
                    f"tool_global_name={tool_global_name}"
                )
                return
        
        # 添加新工具
        new_tool = ToolRelationItem(
            tool_global_name=tool_global_name,
            tool_original_name=tool_original_name
        )
        relation.tools.append(new_tool)
        
        # 保存关系
        await self._cache_layer.put_relation(
            "service_tools",
            service_global_name,
            relation.to_dict()
        )
        
        logger.info(
            f"[RELATIONSHIP] Successfully added Service-Tool relation: "
            f"service_global_name={service_global_name}, "
            f"tool_global_name={tool_global_name}"
        )
    
    async def remove_service_tool(
        self,
        service_global_name: str,
        tool_global_name: str
    ) -> None:
        """
        移除 Service-Tool 关系
        
        Args:
            service_global_name: 服务全局名称
            tool_global_name: 工具全局名称
            
        Raises:
            ValueError: 如果参数无效
            KeyError: 如果关系不存在
            RuntimeError: 如果移除失败
        """
        if not service_global_name:
            raise ValueError("Service global name cannot be empty")
        if not tool_global_name:
            raise ValueError("Tool global name cannot be empty")
        
        logger.debug(
            f"[RELATIONSHIP] Removing Service-Tool relation: "
            f"service_global_name={service_global_name}, "
            f"tool_global_name={tool_global_name}"
        )
        
        # 获取现有关系
        relation_data = await self._cache_layer.get_relation(
            "service_tools",
            service_global_name
        )
        
        if relation_data is None:
            raise KeyError(
                f"Service relation does not exist: service_global_name={service_global_name}"
            )
        
        # 解析关系
        relation = ServiceToolRelation.from_dict(relation_data)
        
        # 查找并移除工具
        original_count = len(relation.tools)
        relation.tools = [
            tool for tool in relation.tools
            if tool.tool_global_name != tool_global_name
        ]
        
        if len(relation.tools) == original_count:
            raise KeyError(
                f"Tool does not exist in service relation: "
                f"service_global_name={service_global_name}, "
                f"tool_global_name={tool_global_name}"
            )
        
        # 保存更新后的关系
        if len(relation.tools) == 0:
            # 如果没有工具了，删除整个关系
            await self._cache_layer.delete_relation(
                "service_tools",
                service_global_name
            )
            logger.info(
                f"[RELATIONSHIP] Deleted empty service relation: "
                f"service_global_name={service_global_name}"
            )
        else:
            # 保存更新后的关系
            await self._cache_layer.put_relation(
                "service_tools",
                service_global_name,
                relation.to_dict()
            )
            logger.info(
                f"[RELATIONSHIP] Successfully removed Service-Tool relation: "
                f"service_global_name={service_global_name}, "
                f"tool_global_name={tool_global_name}"
            )
    
    async def get_service_tools(
        self,
        service_global_name: str
    ) -> List[Dict[str, Any]]:
        """
        获取服务的所有工具关系
        
        Args:
            service_global_name: 服务全局名称
            
        Returns:
            工具关系列表，如果不存在返回空列表
            
        Raises:
            ValueError: 如果参数无效
            RuntimeError: 如果获取失败
        """
        if not service_global_name:
            raise ValueError("Service global name cannot be empty")
        
        logger.debug(
            f"[RELATIONSHIP] Getting service tool relations: "
            f"service_global_name={service_global_name}"
        )
        
        # 获取关系
        relation_data = await self._cache_layer.get_relation(
            "service_tools",
            service_global_name
        )
        
        if relation_data is None:
            logger.debug(
                f"[RELATIONSHIP] Service relation does not exist: "
                f"service_global_name={service_global_name}"
            )
            return []
        
        # 解析关系
        relation = ServiceToolRelation.from_dict(relation_data)
        
        # 转换为字典列表
        tools = [tool.to_dict() for tool in relation.tools]
        
        logger.debug(
            f"[RELATIONSHIP] Retrieved {len(tools)} tool relations: "
            f"service_global_name={service_global_name}"
        )
        
        return tools
    
    # ==================== 级联删除操作 ====================
    
    async def remove_service_cascade(
        self,
        agent_id: str,
        service_global_name: str
    ) -> None:
        """
        级联删除服务相关的所有关系
        
        删除顺序：
        1. 移除 Agent-Service 关系
        2. 删除 Service-Tool 关系
        
        Args:
            agent_id: Agent ID
            service_global_name: 服务全局名称
            
        Raises:
            ValueError: 如果参数无效
            RuntimeError: 如果删除失败
        """
        if not agent_id:
            raise ValueError("Agent ID cannot be empty")
        if not service_global_name:
            raise ValueError("Service global name cannot be empty")
        
        logger.info(
            f"[RELATIONSHIP] Cascading delete service relations: agent_id={agent_id}, "
            f"service_global_name={service_global_name}"
        )
        
        # 1. 移除 Agent-Service 关系
        try:
            await self.remove_agent_service(agent_id, service_global_name)
        except KeyError as e:
            logger.warning(
                f"[RELATIONSHIP] Agent-Service relation does not exist, skipping: {e}"
            )
        
        # 2. 删除 Service-Tool 关系
        try:
            await self._cache_layer.delete_relation(
                "service_tools",
                service_global_name
            )
            logger.info(
                f"[RELATIONSHIP] Deleted Service-Tool relation: "
                f"service_global_name={service_global_name}"
            )
        except Exception as e:
            logger.warning(
                f"[RELATIONSHIP] Failed to delete Service-Tool relation: {e}"
            )
        
        logger.info(
            f"[RELATIONSHIP] Cascading delete completed: service_global_name={service_global_name}"
        )
