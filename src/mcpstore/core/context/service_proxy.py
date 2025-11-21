"""
MCPStore Service Proxy Module
服务代理对象，提供具体服务的操作方法
"""

import logging
from typing import Dict, List, Any

from mcpstore.core.models.tool import ToolInfo
from .types import ContextType

logger = logging.getLogger(__name__)


class ServiceProxy:
    """
    服务代理对象
    提供具体服务的所有操作方法，进一步缩小作用域
    """

    def __init__(self, context: 'MCPStoreContext', service_name: str):
        """
        初始化服务代理

        Args:
            context: 父级上下文对象
            service_name: 服务名称
        """
        self._context = context
        self._service_name = service_name
        self._context_type = context.context_type
        self._agent_id = context.agent_id

        logger.debug(f"[SERVICE_PROXY] Created proxy for service '{service_name}' in {self._context_type.value} context")

    @property
    def service_name(self) -> str:
        """获取服务名称"""
        return self._service_name

    @property
    def context_type(self) -> ContextType:
        """获取上下文类型"""
        return self._context_type

    # === 服务信息查询方法（两个单词） ===

    def service_info(self) -> Any:
        """
        获取服务详情（两个单词方法）

        Returns:
            Any: 服务详情信息
        """
        return self._context.get_service_info(self._service_name)

    def service_status(self) -> dict:
        """
        获取服务状态（两个单词方法）

        Returns:
            dict: 服务状态信息
        """
        return self._context.get_service_status(self._service_name)

    def health_details(self) -> dict:
        """
        获取详细健康信息（两个单词方法）

        Returns:
            dict: 详细健康检查结果（包含状态、响应时间、时间戳、错误信息、生命周期映射等）
        """
        try:
            # 计算实际查询使用的服务名（Agent 模式使用全局名）
            effective_name = self._service_name
            if self._context_type == ContextType.AGENT and getattr(self._context, "_service_mapper", None):
                effective_name = self._context._service_mapper.to_global_name(self._service_name)
            # 使用 orchestrator 的稳定公共 API
            result = self._context._sync_helper.run_async(
                self._context._store.orchestrator.health_details(
                    effective_name,
                    None  # 透明代理：统一在全局命名空间执行健康检查
                )
            )
            # 保持向后兼容：补齐 effective_name 字段
            if isinstance(result, dict) and "effective_name" not in result:
                result = {**result, "effective_name": effective_name, "service_name": self._service_name}
            return result
        except Exception as e:
            logger.error(f"Failed to get health details for {self._service_name}: {e}")
            return {"service_name": self._service_name, "status": "error", "error": str(e)}

    # === 服务健康检查方法（两个单词） ===

    def check_health(self) -> dict:
        """
        检查服务健康状态（两个单词方法）—返回该服务的健康摘要

        Returns:
            dict: 健康检查结果（服务级别摘要）
        """
        details = self.health_details()
        # 精简为摘要
        return {
            "service_name": details.get("service_name", self._service_name),
            "status": details.get("status", "unknown"),
            "healthy": details.get("healthy", False),
            "response_time": details.get("response_time"),
            "error_message": details.get("error_message")
        }

    def is_healthy(self) -> bool:
        """
        检查服务是否健康（两个单词方法）

        Returns:
            bool: 是否健康
        """
        try:
            # 通过orchestrator检查服务健康状态
            if self._context_type == ContextType.STORE:
                # 使用同步助手运行异步方法
                return self._context._sync_helper.run_async(
                    self._context._store.orchestrator.is_service_healthy(self._service_name)
                )
            else:
                return self._context._sync_helper.run_async(
                    self._context._store.orchestrator.is_service_healthy(self._service_name, self._agent_id)
                )
        except Exception as e:
            logger.error(f"Failed to check health for {self._service_name}: {e}")
            return False

    # === 工具管理方法（两个单词） ===

    def list_tools(self) -> List[ToolInfo]:
        """
        列出服务工具（两个单词方法）

        Returns:
            List[ToolInfo]: 工具列表
        """
        try:
            # 使用 orchestrator 快照：
            # - Store: 全局服务名
            # - Agent: 已投影为本地服务名
            agent_id = self._context._agent_id if self._context_type == ContextType.AGENT else None
            snapshot = self._context._sync_helper.run_async(
                self._context._store.orchestrator.tools_snapshot(agent_id)
            )
            filtered = [t for t in snapshot if isinstance(t, dict) and t.get("service_name") == self._service_name]
            return [ToolInfo(**t) for t in filtered]
        except Exception as e:
            logger.error(f"[SERVICE_PROXY.list_tools] failed: {e}")
            return []

    def tools_stats(self) -> Dict[str, Any]:
        """
        获取工具统计信息（两个单词方法）

        Returns:
            Dict[str, Any]: 工具统计信息（仅当前服务）
        """
        tools = self.list_tools()
        return {
            "tools": [
                {
                    "name": t.name,
                    "description": t.description,
                    "service_name": t.service_name,
                    "client_id": t.client_id,
                    "inputSchema": t.inputSchema,
                    "has_schema": t.inputSchema is not None
                }
                for t in tools
            ],
            "metadata": {
                "total_tools": len(tools),
                "services_count": 1,
                "tools_by_service": {self._service_name: len(tools)}
            }
        }

    # === 服务管理方法（两个单词） ===

    def update_config(self, config: Dict[str, Any]) -> bool:
        """
        更新服务配置（两个单词方法）

        Args:
            config: 新的配置

        Returns:
            bool: 更新是否成功
        """
        return self._context.update_service(self._service_name, config)

    def restart_service(self) -> bool:
        """
        重启服务（两个单词方法）

        Returns:
            bool: 重启是否成功
        """
        return self._context.restart_service(self._service_name)

    def delete_service(self) -> bool:
        """
        删除服务（两个单词方法）

        Returns:
            bool: 删除是否成功
        """
        return self._context.delete_service(self._service_name)
    def patch_config(self, updates: Dict[str, Any]) -> bool:
        """
        增量更新服务配置（两个单词方法）

        Args:
            updates: 要更新的配置项

        Returns:
            bool: 是否成功
        """
        return self._context.patch_service(self._service_name, updates)
    def remove_service(self) -> bool:
        """
        移除服务（两个单词方法）

        Returns:
            bool: 移除是否成功
        """
        # 通过orchestrator移除服务（同步封装）
        try:
            if self._context_type == ContextType.STORE:
                return self._context._sync_helper.run_async(
                    self._context._store.orchestrator.remove_service(self._service_name)
                )
            else:
                # Agent 模式需要传递 agent_id
                return self._context._sync_helper.run_async(
                    self._context._store.orchestrator.remove_service(self._service_name, self._agent_id)
                )
        except Exception as e:
            logger.error(f"Failed to remove service {self._service_name}: {e}")
            return False

    # === 服务内容管理方法（两个单词） ===

    def refresh_content(self) -> bool:
        """
        刷新服务内容（两个单词方法）

        Returns:
            bool: 刷新是否成功
        """
        try:
            if self._context_type == ContextType.STORE:
                return self._context._sync_helper.run_async(
                    self._context._store.orchestrator.refresh_service_content(self._service_name)
                )
            else:
                return self._context._sync_helper.run_async(
                    self._context._store.orchestrator.refresh_service_content(self._service_name, self._agent_id)
                )
        except Exception as e:
            logger.error(f"Failed to refresh content for {self._service_name}: {e}")
            return False

    def find_tool(self, tool_name: str) -> 'ToolProxy':
        """
        在当前服务范围内查找工具
        
        进一步缩小范围到特定服务的工具
        
        Args:
            tool_name: 工具名称
            
        Returns:
            ToolProxy: 工具代理对象，范围限定为当前服务
            
        Example:
            # 先获取服务，再查找服务内的工具
            weather_service = store.for_store().find_service('weather')
            weather_tool = weather_service.find_tool('get_current_weather')
            weather_tool.tool_info()        # 获取工具详情
            weather_tool.call_tool({...})   # 调用工具
            
            # Agent 模式下的服务工具查找
            demo_service = store.for_agent('demo1').find_service('service1')
            demo_tool = demo_service.find_tool('search_tool')
            demo_tool.usage_stats()         # 使用统计
        """
        from .tool_proxy import ToolProxy
        return ToolProxy(self._context, tool_name, scope='service', service_name=self._service_name)

    # === 便捷属性方法 ===

    @property
    def name(self) -> str:
        """获取服务名称（便捷属性）"""
        return self._service_name

    @property
    def tools_count(self) -> int:
        """获取工具数量（便捷属性）"""
        return len(self.list_tools())

    @property
    def is_connected(self) -> bool:
        """获取连接状态（便捷属性）"""
        try:
            service_info = self.service_info()
            if hasattr(service_info, 'connected'):
                return service_info.connected
            elif isinstance(service_info, dict):
                return service_info.get('connected', False)
            # 回退：从 orchestrator 的缓存状态判断
            status = self._context._store.orchestrator.get_service_status(
                self._service_name,
                self._agent_id if self._context_type == ContextType.AGENT else None
            )
            if isinstance(status, dict):
                return bool(status.get('healthy', False))
            return False
        except Exception:
            return False

    # === 字符串表示 ===

    def __str__(self) -> str:
        return f"ServiceProxy(service='{self._service_name}', context='{self._context_type.value}')"

    def __repr__(self) -> str:
        return self.__str__()
