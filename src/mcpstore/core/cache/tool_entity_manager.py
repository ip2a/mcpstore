"""
工具实体管理器

负责管理工具实体的 CRUD 操作。
"""

import hashlib
import json
import logging
import time
from typing import Any, Dict, List, Optional, TYPE_CHECKING

from .models import ToolEntity
from .naming_service import NamingService

if TYPE_CHECKING:
    from .cache_layer_manager import CacheLayerManager

logger = logging.getLogger(__name__)


class ToolEntityManager:
    """
    工具实体管理器
    
    管理工具实体的创建、查询和删除操作。
    """
    
    def __init__(
        self,
        cache_layer: 'CacheLayerManager',
        naming: NamingService
    ):
        """
        初始化工具实体管理器
        
        Args:
            cache_layer: 缓存层管理器
            naming: 命名服务
        """
        self._cache_layer = cache_layer
        self._naming = naming
        logger.debug("[TOOL_ENTITY] 初始化 ToolEntityManager")
    
    @staticmethod
    def _generate_tool_hash(tool_def: Dict[str, Any]) -> str:
        """
        生成工具定义的哈希值
        
        Args:
            tool_def: 工具定义
            
        Returns:
            SHA256 哈希值
        """
        # 将工具定义转换为稳定的 JSON 字符串
        tool_json = json.dumps(tool_def, sort_keys=True)
        # 计算 SHA256 哈希
        hash_obj = hashlib.sha256(tool_json.encode('utf-8'))
        return f"sha256:{hash_obj.hexdigest()}"
    
    async def create_tool(
        self,
        service_global_name: str,
        service_original_name: str,
        source_agent: str,
        tool_original_name: str,
        tool_def: Dict[str, Any]
    ) -> str:
        """
        创建工具实体
        
        Args:
            service_global_name: 服务全局名称
            service_original_name: 服务原始名称
            source_agent: 来源 Agent ID
            tool_original_name: 工具原始名称
            tool_def: 工具定义（包含 description 和 input_schema）
            
        Returns:
            工具全局名称
            
        Raises:
            ValueError: 如果参数无效
            RuntimeError: 如果创建失败
        """
        if not service_global_name:
            raise ValueError("服务全局名称不能为空")
        if not service_original_name:
            raise ValueError("服务原始名称不能为空")
        if not source_agent:
            raise ValueError("来源 Agent ID 不能为空")
        if not tool_original_name:
            raise ValueError("工具原始名称不能为空")
        if not isinstance(tool_def, dict):
            raise ValueError(
                f"工具定义必须是字典类型，实际类型: {type(tool_def).__name__}"
            )
        
        # 处理嵌套的工具定义格式
        # 支持两种格式：
        # 1. 直接格式: {"description": "...", "inputSchema": {...}}
        # 2. 嵌套格式: {"type": "function", "function": {"description": "...", "parameters": {...}}}
        actual_def = tool_def
        if "function" in tool_def and isinstance(tool_def["function"], dict):
            fn = tool_def["function"]
            actual_def = {
                "description": fn.get("description", ""),
                "inputSchema": fn.get("parameters", fn.get("inputSchema", {})),
                "name": fn.get("name", tool_original_name),
                "display_name": fn.get("display_name", tool_original_name),
                "service_name": fn.get("service_name", service_original_name)
            }
        
        # 验证工具定义包含必需字段
        description = actual_def.get("description", "")
        input_schema = actual_def.get("inputSchema", actual_def.get("parameters", {}))
        
        # 生成工具全局名称
        tool_global_name = self._naming.generate_tool_global_name(
            service_global_name,
            tool_original_name
        )
        
        # 生成工具哈希
        tool_hash = self._generate_tool_hash(tool_def)
        
        # 检查工具是否已存在（基于全局名称判断）
        existing = await self._cache_layer.get_entity("tools", tool_global_name)
        if existing:
            # 全局名称相同，认为是同一个实体，更新配置
            entity = ToolEntity(
                tool_global_name=tool_global_name,
                tool_original_name=tool_original_name,
                service_global_name=service_global_name,
                service_original_name=service_original_name,
                source_agent=source_agent,
                description=description,
                input_schema=input_schema,
                created_time=existing.get("created_time", int(time.time())),
                tool_hash=tool_hash
            )
            
            await self._cache_layer.put_entity(
                "tools",
                tool_global_name,
                entity.to_dict()
            )
            
            logger.info(
                f"[TOOL_ENTITY] 更新工具实体: tool_global_name={tool_global_name}, "
                f"tool_original_name={tool_original_name}, "
                f"service_global_name={service_global_name}"
            )
            
            return tool_global_name
        
        # 创建新工具实体
        entity = ToolEntity(
            tool_global_name=tool_global_name,
            tool_original_name=tool_original_name,
            service_global_name=service_global_name,
            service_original_name=service_original_name,
            source_agent=source_agent,
            description=description,
            input_schema=input_schema,
            created_time=int(time.time()),
            tool_hash=tool_hash
        )
        
        # 存储到实体层
        await self._cache_layer.put_entity(
            "tools",
            tool_global_name,
            entity.to_dict()
        )
        
        logger.info(
            f"[TOOL_ENTITY] 创建工具实体: tool_global_name={tool_global_name}, "
            f"tool_original_name={tool_original_name}, "
            f"service_global_name={service_global_name}"
        )
        
        return tool_global_name
    
    async def get_tool(self, tool_global_name: str) -> Optional[ToolEntity]:
        """
        获取工具实体
        
        Args:
            tool_global_name: 工具全局名称
            
        Returns:
            工具实体，如果不存在返回 None
            
        Raises:
            ValueError: 如果参数无效
            RuntimeError: 如果获取失败
        """
        if not tool_global_name:
            raise ValueError("工具全局名称不能为空")
        
        # 从实体层获取
        data = await self._cache_layer.get_entity("tools", tool_global_name)
        
        if data is None:
            logger.debug(
                f"[TOOL_ENTITY] 工具不存在: tool_global_name={tool_global_name}"
            )
            return None
        
        # 转换为实体对象
        try:
            entity = ToolEntity.from_dict(data)
            logger.debug(
                f"[TOOL_ENTITY] 获取工具实体: tool_global_name={tool_global_name}"
            )
            return entity
        except Exception as e:
            logger.error(
                f"[TOOL_ENTITY] 解析工具实体失败: "
                f"tool_global_name={tool_global_name}, error={e}"
            )
            raise RuntimeError(
                f"解析工具实体失败: tool_global_name={tool_global_name}, error={e}"
            ) from e
    
    async def delete_tool(self, tool_global_name: str) -> None:
        """
        删除工具实体
        
        Args:
            tool_global_name: 工具全局名称
            
        Raises:
            ValueError: 如果参数无效
            RuntimeError: 如果删除失败
        """
        if not tool_global_name:
            raise ValueError("工具全局名称不能为空")
        
        # 从实体层删除
        await self._cache_layer.delete_entity("tools", tool_global_name)
        
        logger.info(
            f"[TOOL_ENTITY] 删除工具实体: tool_global_name={tool_global_name}"
        )
    
    async def list_tools_by_service(
        self,
        service_global_name: str
    ) -> List[ToolEntity]:
        """
        列出服务的所有工具
        
        注意：此方法需要配合 RelationshipManager 使用，
        先从关系层获取工具列表，再批量获取实体。
        
        这里提供一个简化版本，仅用于测试。
        实际使用时应该通过 RelationshipManager 获取工具列表。
        
        Args:
            service_global_name: 服务全局名称
            
        Returns:
            工具实体列表
            
        Raises:
            ValueError: 如果参数无效
        """
        if not service_global_name:
            raise ValueError("服务全局名称不能为空")
        
        # 注意：这是一个简化实现
        # 实际应该从关系层获取工具列表，然后批量获取实体
        # 这里暂时返回空列表，等待 RelationshipManager 实现后再完善
        
        logger.debug(
            f"[TOOL_ENTITY] 列出服务工具（简化版本）: "
            f"service_global_name={service_global_name}"
        )
        
        return []
    
    async def get_many_tools(
        self,
        tool_global_names: List[str]
    ) -> List[Optional[ToolEntity]]:
        """
        批量获取工具实体
        
        Args:
            tool_global_names: 工具全局名称列表
            
        Returns:
            工具实体列表，不存在的工具返回 None
            
        Raises:
            ValueError: 如果参数无效
            RuntimeError: 如果获取失败
        """
        if not isinstance(tool_global_names, list):
            raise ValueError(
                f"tool_global_names 必须是列表类型，"
                f"实际类型: {type(tool_global_names).__name__}"
            )
        
        if not tool_global_names:
            return []
        
        # 批量获取
        data_list = await self._cache_layer.get_many_entities(
            "tools",
            tool_global_names
        )
        
        # 转换为实体对象
        entities = []
        for i, data in enumerate(data_list):
            if data is None:
                entities.append(None)
            else:
                try:
                    entity = ToolEntity.from_dict(data)
                    entities.append(entity)
                except Exception as e:
                    logger.error(
                        f"[TOOL_ENTITY] 解析工具实体失败: "
                        f"tool_global_name={tool_global_names[i]}, error={e}"
                    )
                    # 解析失败时返回 None
                    entities.append(None)
        
        logger.debug(
            f"[TOOL_ENTITY] 批量获取工具: count={len(tool_global_names)}, "
            f"found={sum(1 for e in entities if e is not None)}"
        )
        
        return entities
