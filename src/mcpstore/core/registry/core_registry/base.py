"""
Core Registry Base Classes - 基础类和接口定义

定义所有管理器的基础接口和抽象类，确保模块间的解耦和一致性。

注意：StateManagerInterface, PersistenceManagerInterface, CacheManagerInterface,
ManagerFactory, ManagerCoordinator 已废弃并移除。
功能已迁移到 core/cache/ 目录。
"""

import logging
from abc import ABC, abstractmethod
from typing import Dict, Any, Optional, List

logger = logging.getLogger(__name__)


class BaseManager(ABC):
    """基础管理器抽象类"""

    def __init__(self, cache_layer, naming_service, namespace: str = "default"):
        self._cache_layer = cache_layer
        self._naming = naming_service
        self._namespace = namespace
        # 统一使用模块路径作为 logger 名称，保证日志前缀与其他模块一致
        # 例如：mcpstore.core.registry.core_registry.session_manager - INFO - ...
        # 而不是：SessionManager - INFO - ...
        self._logger = logging.getLogger(self.__module__)

    @abstractmethod
    def initialize(self) -> None:
        """初始化管理器"""
        pass

    @abstractmethod
    def cleanup(self) -> None:
        """清理管理器资源"""
        pass


class ServiceManagerInterface(BaseManager):
    """服务管理器接口"""

    @abstractmethod
    def add_service(self, agent_id: str, name: str, **kwargs) -> bool:
        """添加服务"""
        pass

    @abstractmethod
    def add_service_async(self, agent_id: str, name: str, **kwargs) -> bool:
        """异步添加服务"""
        pass

    @abstractmethod
    def remove_service(self, agent_id: str, name: str) -> Optional[Any]:
        """移除服务"""
        pass

    @abstractmethod
    def remove_service_async(self, agent_id: str, name: str) -> Optional[Any]:
        """异步移除服务"""
        pass

    @abstractmethod
    def replace_service_tools(self, agent_id: str, service_name: str, **kwargs) -> Dict[str, Any]:
        """替换服务工具"""
        pass

    @abstractmethod
    def replace_service_tools_async(self, agent_id: str, service_name: str, **kwargs) -> Dict[str, Any]:
        """异步替换服务工具"""
        pass

    @abstractmethod
    def add_failed_service(self, agent_id: str, name: str, **kwargs) -> bool:
        """添加失败服务"""
        pass

    @abstractmethod
    def get_services_for_agent(self, agent_id: str) -> List[str]:
        """获取代理的所有服务"""
        pass

    @abstractmethod
    def get_service_details(self, agent_id: str, name: str) -> Dict[str, Any]:
        """获取服务详情"""
        pass

    @abstractmethod
    def get_service_info(self, agent_id: str, service_name: str) -> Optional['ServiceInfo']:
        """获取服务信息"""
        pass

    @abstractmethod
    def get_service_config(self, agent_id: str, name: str) -> Optional[Dict[str, Any]]:
        """获取服务配置"""
        pass

    @abstractmethod
    def clear(self, agent_id: str):
        """清除代理的所有服务"""
        pass

    @abstractmethod
    def clear_async(self, agent_id: str) -> None:
        """异步清除代理的所有服务"""
        pass


class ToolManagerInterface(BaseManager):
    """工具管理器接口"""

    @abstractmethod
    def get_all_tools(self, agent_id: str) -> List[Dict[str, Any]]:
        """获取所有工具"""
        pass

    @abstractmethod
    def get_all_tools_dict_async(self, agent_id: str) -> Dict[str, Dict[str, Any]]:
        """异步获取所有工具字典"""
        pass

    @abstractmethod
    def list_tools(self, agent_id: str) -> List['ToolInfo']:
        """列出工具"""
        pass

    @abstractmethod
    def get_all_tool_info(self, agent_id: str) -> List[Dict[str, Any]]:
        """获取所有工具信息"""
        pass

    @abstractmethod
    def get_tools_for_service(self, agent_id: str, service_name: str) -> List[str]:
        """获取服务的工具列表"""
        pass

    @abstractmethod
    def get_tools_for_service_async(self, agent_id: str, service_name: str) -> List[str]:
        """异步获取服务的工具列表"""
        pass

    @abstractmethod
    def get_tool_info(self, agent_id: str, tool_name: str) -> Dict[str, Any]:
        """获取工具信息"""
        pass

    @abstractmethod
    def get_session_for_tool(self, agent_id: str, tool_name: str) -> Optional[Any]:
        """获取工具的会话"""
        pass


class SessionManagerInterface(BaseManager):
    """会话管理器接口"""

    @abstractmethod
    def get_session(self, agent_id: str, name: str) -> Optional[Any]:
        """获取会话"""
        pass

    @abstractmethod
    def set_session(self, agent_id: str, service_name: str, session: Any) -> None:
        """设置会话"""
        pass

    @abstractmethod
    def clear_session(self, agent_id: str, service_name: str):
        """清除特定服务的会话"""
        pass

    @abstractmethod
    def clear_all_sessions(self, agent_id: str):
        """清除代理的所有会话"""
        pass
