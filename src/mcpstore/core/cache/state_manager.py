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
        valid_health_statuses = ["healthy", "unhealthy", "unknown", "initializing", "warning", "disconnected", "reconnecting"]
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

        # region agent log
        try:
            import json
            from pathlib import Path
            log_path = Path("/home/yuuu/app/2025/2025_6/mcpstore/.cursor/debug.log")
            log_record = {
                "sessionId": "debug-session",
                "runId": "pre-fix",
                "hypothesisId": "H1",
                "location": "state_manager.py:update_service_status",
                "message": "before_put_state_service_status",
                "data": {
                    "service_global_name": service_global_name,
                    "health_status": health_status,
                    "tools_count": len(tools),
                    "tool_original_names": [t.tool_original_name for t in tools],
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

        # 存储到状态层
        # region agent log
        try:
            import json
            from pathlib import Path
            log_path = Path("/home/yuuu/app/2025/2025_6/mcpstore/.cursor/debug.log")
            log_record = {
                "sessionId": "debug-session",
                "runId": "pre-fix",
                "hypothesisId": "H5",
                "location": "state_manager.py:update_service_status",
                "message": "before_put_state_final",
                "data": {
                    "service_global_name": service_global_name,
                    "health_status": health_status,
                    "status_dict_keys": list(status.to_dict().keys()),
                },
                "timestamp": int(time.time() * 1000),
            }
            log_path.parent.mkdir(parents=True, exist_ok=True)
            with log_path.open("a", encoding="utf-8") as f:
                f.write(json.dumps(log_record, ensure_ascii=False) + "\n")
        except Exception:
            pass
        # endregion
        await self._cache_layer.put_state(
            "service_status",
            service_global_name,
            status.to_dict()
        )
        # region agent log
        try:
            import json
            from pathlib import Path
            log_path = Path("/home/yuuu/app/2025/2025_6/mcpstore/.cursor/debug.log")
            log_record = {
                "sessionId": "debug-session",
                "runId": "pre-fix",
                "hypothesisId": "H5",
                "location": "state_manager.py:update_service_status",
                "message": "after_put_state_final",
                "data": {
                    "service_global_name": service_global_name,
                    "health_status": health_status,
                },
                "timestamp": int(time.time() * 1000),
            }
            log_path.parent.mkdir(parents=True, exist_ok=True)
            with log_path.open("a", encoding="utf-8") as f:
                f.write(json.dumps(log_record, ensure_ascii=False) + "\n")
        except Exception:
            pass
        # endregion
        
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
        # region agent log
        try:
            import json, time
            from pathlib import Path
            log_path = Path("/home/yuuu/app/2025/2025_6/mcpstore/.cursor/debug.log")
            log_record = {
                "sessionId": "debug-session",
                "runId": "pre-fix",
                "hypothesisId": "H5",
                "location": "state_manager.py:delete_service_status",
                "message": "before_delete_service_status",
                "data": {
                    "service_global_name": service_global_name,
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

        await self._cache_layer.delete_state("service_status", service_global_name)
        
        logger.debug(
            f"[StateManager] 删除服务状态: service={service_global_name}"
        )
    
    async def set_tool_available(
        self,
        service_global_name: str,
        tool_original_name: str
    ) -> None:
        """
        设置工具为可用状态
        
        Args:
            service_global_name: 服务全局名称
            tool_original_name: 工具原始名称
            
        Raises:
            RuntimeError: 如果服务状态不存在或工具不存在
        """
        await self._update_tool_status_by_original_name(
            service_global_name,
            tool_original_name,
            "available"
        )
    
    async def set_tool_unavailable(
        self,
        service_global_name: str,
        tool_original_name: str
    ) -> None:
        """
        设置工具为不可用状态
        
        Args:
            service_global_name: 服务全局名称
            tool_original_name: 工具原始名称
            
        Raises:
            RuntimeError: 如果服务状态不存在或工具不存在
        """
        await self._update_tool_status_by_original_name(
            service_global_name,
            tool_original_name,
            "unavailable"
        )
    
    async def _update_tool_status_by_original_name(
        self,
        service_global_name: str,
        tool_original_name: str,
        status: str
    ) -> None:
        """
        通过原始工具名更新工具状态
        
        Args:
            service_global_name: 服务全局名称
            tool_original_name: 工具原始名称
            status: 工具状态 ("available" | "unavailable")
            
        Raises:
            RuntimeError: 如果服务状态不存在或工具不存在
        """
        # 获取当前服务状态
        service_status = await self.get_service_status(service_global_name)
        
        if service_status is None:
            raise RuntimeError(
                f"服务状态不存在，无法更新工具状态: "
                f"service={service_global_name}, tool={tool_original_name}"
            )
        
        # 查找并更新工具状态（通过原始名称）
        tool_found = False
        for tool in service_status.tools:
            if tool.tool_original_name == tool_original_name:
                tool.status = status
                tool_found = True
                break
        
        if not tool_found:
            raise RuntimeError(
                f"工具不存在于服务状态中: "
                f"service={service_global_name}, tool_original_name={tool_original_name}"
            )
        
        # 保存更新后的服务状态
        await self._cache_layer.put_state(
            "service_status",
            service_global_name,
            service_status.to_dict()
        )
        
        logger.debug(
            f"[StateManager] 更新工具状态: service={service_global_name}, "
            f"tool_original_name={tool_original_name}, status={status}"
        )
    
    async def batch_set_tools_status(
        self,
        service_global_name: str,
        tool_original_names: List[str],
        status: str
    ) -> None:
        """
        批量设置工具状态
        
        Args:
            service_global_name: 服务全局名称
            tool_original_names: 工具原始名称列表
            status: 工具状态 ("available" | "unavailable")
            
        Raises:
            ValueError: 如果状态值无效
            RuntimeError: 如果服务状态不存在或任何工具不存在
        """
        # 验证状态值
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
                f"service={service_global_name}"
            )
        
        # 批量更新工具状态
        not_found_tools = []
        for tool_original_name in tool_original_names:
            tool_found = False
            for tool in service_status.tools:
                if tool.tool_original_name == tool_original_name:
                    tool.status = status
                    tool_found = True
                    break
            
            if not tool_found:
                not_found_tools.append(tool_original_name)
        
        if not_found_tools:
            raise RuntimeError(
                f"以下工具不存在于服务状态中: "
                f"service={service_global_name}, tools={not_found_tools}"
            )
        
        # 保存更新后的服务状态
        await self._cache_layer.put_state(
            "service_status",
            service_global_name,
            service_status.to_dict()
        )
        
        logger.debug(
            f"[StateManager] 批量更新工具状态: service={service_global_name}, "
            f"tools_count={len(tool_original_names)}, status={status}"
        )

    # ==================== 同步方法 (Functional Core) ====================

    def set_state_sync(self, service_global_name: str, state) -> None:
        """
        同步设置服务状态 (Functional Core - 纯同步操作)

        严格按照核心原则：
        1. Functional Core: 纯同步操作，无IO，无副作用
        2. 通过缓存层的同步接口实现
        3. 简单直接的状态设置

        Args:
            service_global_name: 服务全局名称
            state: 服务状态
        """
        try:
            from mcpstore.core.models.service import ServiceConnectionState
            # 转换为状态字典
            if isinstance(state, ServiceConnectionState):
                state_dict = {
                    "health_status": state,
                    "last_updated": str(datetime.now())
                }
            else:
                state_dict = state

            # Functional Core: 只准备数据，不进行IO操作
            # IO操作应该在 Imperative Shell 层处理
            # 这里我们返回需要保存的数据，由调用者决定如何保存
            logger.debug(f"[StateManager] 准备状态数据: {service_global_name} -> {state_dict}")
            # 注意：这个方法应该由 Imperative Shell 的异步包装器调用
            raise NotImplementedError("请在异步上下文中使用 update_service_status 方法")
            logger.debug(f"[StateManager] 同步设置状态: {service_global_name} -> {state}")

        except Exception as e:
            logger.error(f"[StateManager] 同步设置状态失败 {service_global_name}: {e}")
            raise

    def set_metadata_sync(self, service_global_name: str, metadata) -> None:
        """
        同步设置服务元数据 (Functional Core - 纯同步操作)

        严格按照核心原则：
        1. Functional Core: 纯同步操作，无IO，无副作用
        2. 通过缓存层的同步接口实现
        3. 简单直接的元数据设置

        Args:
            service_global_name: 服务全局名称
            metadata: 服务元数据
        """
        try:
            # 转换为元数据字典
            if hasattr(metadata, 'to_dict'):
                metadata_dict = metadata.to_dict()
            else:
                metadata_dict = metadata

            # 使用缓存层的同步接口保存元数据
            self._cache_layer.put_state_sync("service_metadata", service_global_name, metadata_dict)
            logger.debug(f"[StateManager] 同步设置元数据: {service_global_name}")

        except Exception as e:
            logger.error(f"[StateManager] 同步设置元数据失败 {service_global_name}: {e}")
            raise
