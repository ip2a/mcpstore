"""
服务实体管理器

负责管理服务实体的 CRUD 操作。
"""

import logging
import time
from typing import Any, Dict, List, Optional, TYPE_CHECKING

from .models import ServiceEntity
from .naming_service import NamingService

if TYPE_CHECKING:
    from .cache_layer_manager import CacheLayerManager

logger = logging.getLogger(__name__)


class ServiceEntityManager:
    """
    服务实体管理器
    
    管理服务实体的创建、查询、更新和删除操作。
    """
    
    def __init__(
        self,
        cache_layer: 'CacheLayerManager',
        naming: NamingService
    ):
        """
        初始化服务实体管理器
        
        Args:
            cache_layer: 缓存层管理器
            naming: 命名服务
        """
        self._cache_layer = cache_layer
        self._naming = naming
        logger.debug("[SERVICE_ENTITY] Initializing ServiceEntityManager")
    
    async def create_service(
        self,
        agent_id: str,
        original_name: str,
        config: Dict[str, Any]
    ) -> str:
        """
        创建服务实体
        
        Args:
            agent_id: Agent ID
            original_name: 服务原始名称
            config: 服务配置
            
        Returns:
            服务全局名称
            
        Raises:
            ValueError: 如果参数无效
            RuntimeError: 如果创建失败
        """
        if not agent_id:
            raise ValueError("Agent ID cannot be empty")
        if not original_name:
            raise ValueError("Service original name cannot be empty")
        if not isinstance(config, dict):
            raise ValueError(
                f"Service config must be a dictionary type, actual type: {type(config).__name__}"
            )
        
        # 生成全局名称
        global_name = self._naming.generate_service_global_name(
            original_name,
            agent_id
        )
        
        # 检查服务是否已存在（基于全局名称判断）
        existing = await self._cache_layer.get_entity("services", global_name)
        if existing:
            # 全局名称相同，认为是同一个实体，更新配置
            entity = ServiceEntity(
                service_global_name=global_name,
                service_original_name=original_name,
                source_agent=agent_id,
                config=config,
                added_time=existing.get("added_time", int(time.time()))
            )
            
            await self._cache_layer.put_entity(
                "services",
                global_name,
                entity.to_dict()
            )

            logger.info(
                f"[SERVICE_ENTITY] Updated service entity: global_name={global_name}, "
                f"original_name={original_name}, agent_id={agent_id}"
            )
            return global_name
        
        # 创建新服务实体
        entity = ServiceEntity(
            service_global_name=global_name,
            service_original_name=original_name,
            source_agent=agent_id,
            config=config,
            added_time=int(time.time())
        )
        
        # 存储到实体层
        await self._cache_layer.put_entity(
            "services",
            global_name,
            entity.to_dict()
        )

        logger.info(
            f"[SERVICE_ENTITY] Created service entity: global_name={global_name}, "
            f"original_name={original_name}, agent_id={agent_id}"
        )
        
        return global_name
    
    async def get_service(self, global_name: str) -> Optional[ServiceEntity]:
        """
        获取服务实体
        
        Args:
            global_name: 服务全局名称
            
        Returns:
            服务实体，如果不存在返回 None
            
        Raises:
            ValueError: 如果参数无效
            RuntimeError: 如果获取失败
        """
        if not global_name:
            raise ValueError("Service global name cannot be empty")
        
        # 从实体层获取
        data = await self._cache_layer.get_entity("services", global_name)
        
        if data is None:
            logger.debug(
                f"[SERVICE_ENTITY] Service does not exist: global_name={global_name}"
            )
            return None

        # 转换为实体对象
        try:
            entity = ServiceEntity.from_dict(data)
            logger.debug(
                f"[SERVICE_ENTITY] Retrieved service entity: global_name={global_name}"
            )
            return entity
        except Exception as e:
            logger.error(
                f"[SERVICE_ENTITY] Failed to parse service entity: global_name={global_name}, "
                f"error={e}"
            )
            raise RuntimeError(
                f"Failed to parse service entity: global_name={global_name}, error={e}"
            ) from e
    
    async def update_service(
        self,
        global_name: str,
        config: Dict[str, Any]
    ) -> None:
        """
        更新服务配置
        
        Args:
            global_name: 服务全局名称
            config: 新的服务配置
            
        Raises:
            ValueError: 如果参数无效
            KeyError: 如果服务不存在
            RuntimeError: 如果更新失败
        """
        if not global_name:
            raise ValueError("Service global name cannot be empty")
        if not isinstance(config, dict):
            raise ValueError(
                f"Service config must be a dictionary type, actual type: {type(config).__name__}"
            )
        
        # 获取现有服务
        entity = await self.get_service(global_name)
        if entity is None:
            raise KeyError(f"Service does not exist: global_name={global_name}")
        
        # 更新配置
        entity.config = config
        
        # 保存到实体层
        await self._cache_layer.put_entity(
            "services",
            global_name,
            entity.to_dict()
        )

        logger.info(
            f"[SERVICE_ENTITY] Updated service config: global_name={global_name}"
        )
    
    async def delete_service(self, global_name: str) -> None:
        """
        删除服务实体
        
        Args:
            global_name: 服务全局名称
            
        Raises:
            ValueError: 如果参数无效
            RuntimeError: 如果删除失败
        """
        if not global_name:
            raise ValueError("Service global name cannot be empty")

        # 从实体层删除
        await self._cache_layer.delete_entity("services", global_name)

        logger.info(
            f"[SERVICE_ENTITY] Deleted service entity: global_name={global_name}"
        )
    
    async def list_services_by_agent(
        self,
        agent_id: str
    ) -> List[ServiceEntity]:
        """
        列出 Agent 的所有服务
        
        注意：此方法需要配合 RelationshipManager 使用，
        先从关系层获取服务列表，再批量获取实体。
        
        这里提供一个简化版本，仅用于测试。
        实际使用时应该通过 RelationshipManager 获取服务列表。
        
        Args:
            agent_id: Agent ID
            
        Returns:
            服务实体列表
            
        Raises:
            ValueError: 如果参数无效
        """
        if not agent_id:
            raise ValueError("Agent ID cannot be empty")
        
        # 注意：这是一个简化实现
        # 实际应该从关系层获取服务列表，然后批量获取实体
        # 这里暂时返回空列表，等待 RelationshipManager 实现后再完善
        
        logger.debug(
            f"[SERVICE_ENTITY] List agent services (simplified version): agent_id={agent_id}"
        )
        
        return []
    
    async def get_many_services(
        self,
        global_names: List[str]
    ) -> List[Optional[ServiceEntity]]:
        """
        批量获取服务实体
        
        Args:
            global_names: 服务全局名称列表
            
        Returns:
            服务实体列表，不存在的服务返回 None
            
        Raises:
            ValueError: 如果参数无效
            RuntimeError: 如果获取失败
        """
        if not isinstance(global_names, list):
            raise ValueError(
                f"global_names must be a list type, actual type: {type(global_names).__name__}"
            )
        
        if not global_names:
            return []
        
        # 批量获取
        data_list = await self._cache_layer.get_many_entities(
            "services",
            global_names
        )
        
        # 转换为实体对象
        entities = []
        for i, data in enumerate(data_list):
            if data is None:
                entities.append(None)
            else:
                try:
                    entity = ServiceEntity.from_dict(data)
                    entities.append(entity)
                except Exception as e:
                    logger.error(
                        f"[SERVICE_ENTITY] Failed to parse service entity: "
                        f"global_name={global_names[i]}, error={e}"
                    )
                    # 解析失败时返回 None
                    entities.append(None)
        
        logger.debug(
            f"[SERVICE_ENTITY] Batch retrieved services: count={len(global_names)}, "
            f"found={sum(1 for e in entities if e is not None)}"
        )
        
        return entities

    def get_service_entity_sync(self, global_name: str) -> Optional[ServiceEntity]:
        """
        同步获取服务实体 (Functional Core - 纯同步操作)

        严格按照核心原则：
        1. Functional Core: 纯同步操作，无IO，无副作用
        2. 使用现有同步接口，遵循架构模式

        Args:
            global_name: 服务全局名称

        Returns:
            ServiceEntity 如果存在，否则 None
        """
        # Functional Core: 使用现有的同步接口获取所有服务
        all_entities = self._cache_layer.get_all_entities_sync("services")

        # 纯函数操作：从字典中查找指定的实体
        entity_data = all_entities.get(global_name)

        if entity_data:
            return ServiceEntity.from_dict(entity_data)
        return None
