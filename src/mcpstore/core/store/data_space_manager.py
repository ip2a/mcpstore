"""
数据空间管理模块
负责处理 MCPStore 的数据空间相关功能
"""

from typing import Optional, Dict, Any, List
import logging

logger = logging.getLogger(__name__)


class DataSpaceManagerMixin:
    """数据空间管理 Mixin"""
    
    def get_data_space_info(self) -> Optional[Dict[str, Any]]:
        """
        获取数据空间信息

        Returns:
            Dict: 数据空间信息，如果未使用数据空间则返回None
        """
        if self._data_space_manager:
            return self._data_space_manager.get_workspace_info()
        return None

    def get_workspace_dir(self) -> Optional[str]:
        """
        获取工作空间目录路径

        Returns:
            str: 工作空间目录路径，如果未使用数据空间则返回None
        """
        if self._data_space_manager:
            return str(self._data_space_manager.workspace_dir)
        return None

    def is_using_data_space(self) -> bool:
        """
        检查是否使用了数据空间

        Returns:
            bool: 是否使用数据空间
        """
        return self._data_space_manager is not None

    async def _add_service(self, service_names: List[str], agent_id: Optional[str]) -> bool:
        """内部方法：批量添加服务，store级别支持全量注册，agent级别支持指定服务注册"""
        # store级别
        if agent_id is None:
            if not service_names:
                # 全量注册：使用统一同步机制
                if hasattr(self.orchestrator, 'sync_manager') and self.orchestrator.sync_manager:
                    sync_results = await self.orchestrator.sync_manager.sync_global_agent_store_from_mcp_json()
                    return bool(sync_results.get("added") or sync_results.get("updated"))
                else:
                    # 回退到旧方法（带警告）
                    resp = await self.register_all_services_for_store()
                    return bool(resp and resp.service_names)
            else:
                # 支持单独添加服务
                resp = await self.register_selected_services_for_store(service_names)
                return bool(resp and resp.service_names)
        # agent级别
        else:
            if service_names:
                resp = await self.register_services_for_agent(agent_id, service_names)
                return bool(resp and resp.service_names)
            else:
                logger.warning(f"Agent {agent_id} 级别不支持全量注册")
                return False

    async def add_service(self, service_names: List[str], agent_id: Optional[str] = None) -> bool:
        """添加服务的统一入口"""
        context = self.for_agent(agent_id) if agent_id else self.for_store()
        return await context.add_service(service_names)
