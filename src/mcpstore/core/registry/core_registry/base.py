"""
Core Registry Base Classes - 基础类和接口定义

定义所有管理器的基础接口和抽象类，确保模块间的解耦和一致性。
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
        self._logger = logging.getLogger(self.__class__.__name__)

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


class StateManagerInterface(BaseManager):
    """状态管理器接口"""

    @abstractmethod
    def set_service_state(self, agent_id: str, service_name: str, state: Optional['ServiceConnectionState']):
        """设置服务状态"""
        pass

    @abstractmethod
    def set_service_metadata(self, agent_id: str, service_name: str, metadata: Optional['ServiceStateMetadata']):
        """设置服务元数据"""
        pass

    @abstractmethod
    def get_all_service_states(self, agent_id: str) -> Dict[str, 'ServiceConnectionState']:
        """获取所有服务状态"""
        pass

    @abstractmethod
    def get_all_service_states_async(self, agent_id: str) -> Dict[str, 'ServiceConnectionState']:
        """异步获取所有服务状态"""
        pass

    @abstractmethod
    def get_connected_services(self, agent_id: str) -> List[Dict[str, Any]]:
        """获取已连接的服务"""
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


class PersistenceManagerInterface(BaseManager):
    """持久化管理器接口"""

    @abstractmethod
    def load_services_from_json(self) -> Dict[str, Any]:
        """从JSON加载服务配置"""
        pass

    @abstractmethod
    def load_services_from_json_async(self) -> Dict[str, Any]:
        """异步从JSON加载服务配置"""
        pass

    @abstractmethod
    def extract_standard_mcp_config(self, service_config: Dict[str, Any]) -> Dict[str, Any]:
        """提取标准MCP配置"""
        pass


class CacheManagerInterface(BaseManager):
    """缓存管理器接口"""

    @abstractmethod
    def configure_cache_backend(self, cache_config: Dict[str, Any]) -> None:
        """配置缓存后端"""
        pass

    @abstractmethod
    def sync_to_storage(self, operation_name: str = "缓存同步"):
        """同步到存储"""
        pass

    @abstractmethod
    def ensure_sync_helper(self):
        """确保同步助手存在"""
        pass


class ManagerFactory:
    """管理器工厂类"""

    @staticmethod
    def create_service_manager(cache_layer, naming_service, **kwargs) -> ServiceManagerInterface:
        """创建服务管理器实例"""
        from .service_manager import ServiceManager
        return ServiceManager(cache_layer, naming_service, **kwargs)

    @staticmethod
    def create_tool_manager(cache_layer, naming_service, **kwargs) -> ToolManagerInterface:
        """创建工具管理器实例"""
        from .tool_manager import ToolManager
        return ToolManager(cache_layer, naming_service, **kwargs)

    @staticmethod
    def create_state_manager(cache_layer, naming_service, **kwargs) -> StateManagerInterface:
        """创建状态管理器实例"""
        from .state_manager import StateManager
        return StateManager(cache_layer, naming_service, **kwargs)

    @staticmethod
    def create_session_manager(cache_layer, naming_service, **kwargs) -> SessionManagerInterface:
        """创建会话管理器实例"""
        from .session_manager import SessionManager
        return SessionManager(cache_layer, naming_service, **kwargs)

    @staticmethod
    def create_persistence_manager(cache_layer, naming_service, **kwargs) -> PersistenceManagerInterface:
        """创建持久化管理器实例"""
        from .persistence import PersistenceManager
        return PersistenceManager(cache_layer, naming_service, **kwargs)

    @staticmethod
    def create_cache_manager(cache_layer, naming_service, **kwargs) -> CacheManagerInterface:
        """创建缓存管理器实例"""
        from .cache_manager import CacheManager
        return CacheManager(cache_layer, naming_service, **kwargs)


class ManagerCoordinator:
    """管理器协调器 - 处理管理器间的依赖和协作"""

    def __init__(self, cache_layer, naming_service, namespace: str = "default"):
        self._cache_layer = cache_layer
        self._naming = naming_service
        self._namespace = namespace
        self._managers = {}
        self._factory = ManagerFactory()

    def initialize_managers(self):
        """初始化所有管理器"""
        base_kwargs = {"namespace": self._namespace}

        self._managers = {
            'service': self._factory.create_service_manager(
                self._cache_layer, self._naming, **base_kwargs
            ),
            'tool': self._factory.create_tool_manager(
                self._cache_layer, self._naming, **base_kwargs
            ),
            'state': self._factory.create_state_manager(
                self._cache_layer, self._naming, **base_kwargs
            ),
            'session': self._factory.create_session_manager(
                self._cache_layer, self._naming, **base_kwargs
            ),
            'persistence': self._factory.create_persistence_manager(
                self._cache_layer, self._naming, **base_kwargs
            ),
            'cache': self._factory.create_cache_manager(
                self._cache_layer, self._naming, **base_kwargs
            )
        }

        # 初始化所有管理器
        for name, manager in self._managers.items():
            manager.initialize()
            logger.info(f"Initializing manager: {name}")

    def get_manager(self, manager_type: str):
        """获取指定类型的管理器"""
        if manager_type not in self._managers:
            raise ValueError(f"Unknown manager type: {manager_type}")
        return self._managers[manager_type]

    def cleanup_all_managers(self):
        """清理所有管理器"""
        for name, manager in self._managers.items():
            try:
                manager.cleanup()
                logger.info(f"Cleaning up manager: {name}")
            except Exception as e:
                logger.error(f"Error cleaning up manager {name}: {e}")
        self._managers.clear()
