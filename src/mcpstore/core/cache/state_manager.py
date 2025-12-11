"""
状态管理器

管理服务和工具的运行时状态。
"""

import time
import logging
from typing import Dict, List, Optional, Any

from .cache_layer_manager import CacheLayerManager
from .models import ServiceStatus, ToolStatusItem

logger = logging.getLogger(__name__)


class StateManager:
    """
    状态管理器
    
    负责管理服务和工具的运行时状态，包括健康状态、工具可用性等。
    所有状态数据存储在状态层。
    """
    
    def __init__(self, cache_layer: CacheLayerManager):
        """
        初始化状态管理器
        
        Args:
            cache_layer: 缓存层管理器
        """
        self._cache_layer = cache_layer
        logger.debug("[StateManager] 状态管理器初始化完成")
    
    async def update_service_status(
        self,
        service_global_name: str,
        health_status: str,
        tools_status: List[Dict[str, Any]],
        connection_attempts: int = 0,
        max_connection_attempts: int = 3,
        current_error: Optional[str] = None
    ) -> None:
        """
        更新服务状态
        
        Args:
            service_global_name: 服务全局名称
            health_status: 健康状态 ("healthy" | "unhealthy" | "unknown")
            tools_status: 工具状态列表
            connection_attempts: 连接尝试次数
            max_connection_attempts: 最大连接尝试次数
            current_error: 当前错误信息
            
        Raises:
            ValueError: 如果健康状态值无效
        """
        # 验证健康状态
        valid_health_statuses = ["healthy", "unhealthy", "unknown"]
        if health_status not in valid_health_statuses:
            raise ValueError(
                f"无效的健康状态: {health_status}. "
                f"有效值: {valid_health_statuses}"
            )
        
        # 验证工具状态
        tools = []
        for tool_status in tools_status:
            if not isinstance(tool_status, dict):
                raise ValueError(
                    f"工具状态必须是字典类型，实际类型: {type(tool_status).__name__}"
                )
            
            # 创建 ToolStatusItem 进行验证
            tool_item = ToolStatusItem.from_dict(tool_status)
            tools.append(tool_item)
        
        # 创建服务状态对象
        status = ServiceStatus(
            service_global_name=service_global_name,
            health_status=health_status,
            last_health_check=int(time.time()),
            connection_attempts=connection_attempts,
            max_connection_attempts=max_connection_attempts,
            current_error=current_error,
            tools=tools
        )
        
        # 存储到状态层
        await self._cache_layer.put_state(
            "service_status",
            service_global_name,
            status.to_dict()
        )
        
        logger.debug(
            f"[StateManager] 更新服务状态: service={service_global_name}, "
            f"health={health_status}, tools_count={len(tools)}"
        )
    
    async def get_service_status(
        self,
        service_global_name: str
    ) -> Optional[ServiceStatus]:
        """
        获取服务状态
        
        Args:
            service_global_name: 服务全局名称
            
        Returns:
            服务状态对象，如果不存在则返回 None
        """
        status_data = await self._cache_layer.get_state(
            "service_status",
            service_global_name
        )
        
        if status_data is None:
            logger.debug(
                f"[StateManager] 服务状态不存在: service={service_global_name}"
            )
            return None
        
        status = ServiceStatus.from_dict(status_data)
        
        logger.debug(
            f"[StateManager] 获取服务状态: service={service_global_name}, "
            f"health={status.health_status}"
        )
        
        return status
    
    async def update_tool_status(
        self,
        service_global_name: str,
        tool_global_name: str,
        status: str
    ) -> None:
        """
        更新工具状态
        
        Args:
            service_global_name: 服务全局名称
            tool_global_name: 工具全局名称
            status: 工具状态 ("available" | "unavailable")
            
        Raises:
            ValueError: 如果工具状态值无效
            RuntimeError: 如果服务状态不存在
        """
        # 验证工具状态
        valid_statuses = ["available", "unavailable"]
        if status not in valid_statuses:
            raise ValueError(
                f"无效的工具状态: {status}. "
                f"有效值: {valid_statuses}"
            )
        
        # 获取当前服务状态
        service_status = await self.get_service_status(service_global_name)
        
        if service_status is None:
            raise RuntimeError(
                f"服务状态不存在，无法更新工具状态: "
                f"service={service_global_name}, tool={tool_global_name}"
            )
        
        # 查找并更新工具状态
        tool_found = False
        for tool in service_status.tools:
            if tool.tool_global_name == tool_global_name:
                tool.status = status
                tool_found = True
                break
        
        if not tool_found:
            raise RuntimeError(
                f"工具不存在于服务状态中: "
                f"service={service_global_name}, tool={tool_global_name}"
            )
        
        # 保存更新后的服务状态
        await self._cache_layer.put_state(
            "service_status",
            service_global_name,
            service_status.to_dict()
        )
        
        logger.debug(
            f"[StateManager] 更新工具状态: service={service_global_name}, "
            f"tool={tool_global_name}, status={status}"
        )
    
    async def delete_service_status(self, service_global_name: str) -> None:
        """
        删除服务状态
        
        Args:
            service_global_name: 服务全局名称
        """
        await self._cache_layer.delete_state("service_status", service_global_name)
        
        logger.debug(
            f"[StateManager] 删除服务状态: service={service_global_name}"
        )
