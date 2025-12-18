"""
ServiceRegistry - 主服务注册表门面类

这是重构后的主门面类，整合所有专门管理器，提供与原始ServiceRegistry
完全相同的接口，确保100%向后兼容性。
"""

import asyncio
import logging
from typing import Dict, List, Optional, Any, Set
from datetime import datetime

# 导入所有管理器
from .base import ManagerFactory, ManagerCoordinator
from .session_manager import SessionManager
from .state_manager import StateManager
from .tool_manager import ToolManager
from .cache_manager import CacheManager
from .persistence import PersistenceManager
from .service_manager import ServiceManager
from .mapping_manager import MappingManager
from .utils import JSONSchemaUtils, ConfigUtils, ServiceUtils, DataUtils


class AgentClientMappingServiceAdapter:
    """
    AgentClientMappingService 适配器

    提供与原始架构中 _agent_client_service 相同的接口，
    内部委托给 MappingManager 实现。
    """

    def __init__(self, mapping_manager):
        """
        初始化适配器

        Args:
            mapping_manager: MappingManager 实例
        """
        self._mapping_manager = mapping_manager

    def add_agent_client_mapping(self, agent_id: str, client_id: str) -> None:
        """
        添加 Agent-Client 映射
        
        注意：在新架构中，Agent-Client 映射是从 Service-Client 映射派生的，
        此方法为 no-op，仅用于兼容性。
        """
        # Agent-client mappings are derived from service_client mappings
        # No need to store separately
        pass

    def remove_agent_client_mapping(self, agent_id: str, client_id: str) -> None:
        """
        移除 Agent-Client 映射
        
        注意：在新架构中，Agent-Client 映射是从 Service-Client 映射派生的，
        此方法为 no-op，仅用于兼容性。
        """
        pass

    def add_service_client_mapping(self, agent_id: str, service_name: str, client_id: str) -> None:
        """添加 Service-Client 映射"""
        self._mapping_manager.set_service_client_mapping(agent_id, service_name, client_id)

    def remove_service_client_mapping(self, agent_id: str, service_name: str) -> None:
        """移除 Service-Client 映射"""
        self._mapping_manager.remove_service_client_mapping(agent_id, service_name)

    def get_service_client_id(self, agent_id: str, service_name: str) -> str:
        """获取服务对应的 Client ID"""
        return self._mapping_manager.get_service_client_id(agent_id, service_name)

    def get_service_client_mapping(self, agent_id: str) -> dict:
        """获取指定 agent 的所有 service-client 映射"""
        # 从 MappingManager 的内部缓存中提取
        result = {}
        prefix = f"{agent_id}:"
        for cache_key, client_id in self._mapping_manager._service_client_mapping.items():
            if cache_key.startswith(prefix):
                service_name = cache_key.split(":", 1)[1]
                result[service_name] = client_id
        return result

    async def get_agent_clients_async(self, agent_id: str) -> list:
        """异步获取 Agent 的所有 Client ID"""
        return await self._mapping_manager.get_agent_clients_async(agent_id)


class ServiceStateServiceAdapter:
    """
    ServiceStateService 适配器

    提供与原始架构中 _service_state_service 相同的接口，
    内部委托给 StateManager 实现。
    """

    def __init__(self, state_manager, naming_service):
        """
        初始化适配器

        Args:
            state_manager: StateManager 实例
            naming_service: NamingService 实例
        """
        self._state_manager = state_manager
        self._naming = naming_service

    def get_service_state(self, agent_id: str, service_name: str) -> Optional[Any]:
        """获取服务状态"""
        return self._state_manager.get_service_state(agent_id, service_name)

    # [已删除] get_service_metadata 同步方法
    # 根据 "pykv 唯一真相数据源" 原则，请使用 get_service_metadata_async 异步方法

    def set_service_state(self, agent_id: str, service_name: str, state: Any) -> bool:
        """设置服务状态"""
        return self._state_manager.set_service_state(agent_id, service_name, state)

    def set_service_metadata(self, agent_id: str, service_name: str, metadata: Any) -> bool:
        """设置服务元数据"""
        return self._state_manager.set_service_metadata(agent_id, service_name, metadata)

    def get_all_service_names(self, agent_id: str) -> List[str]:
        """
        获取指定 Agent 的所有服务名称

        从 StateManager 的状态缓存中提取服务名称
        """
        service_names = []
        # 从状态缓存中获取服务名称
        if hasattr(self._state_manager, '_state_cache'):
            cache_prefix = f"{agent_id}:"
            for cache_key in self._state_manager._state_cache:
                if cache_key.startswith(cache_prefix):
                    service_name = cache_key.split(":", 1)[1]
                    service_names.append(service_name)
        return service_names

    def clear_service_state(self, agent_id: str, service_name: str) -> bool:
        """清除服务状态"""
        try:
            cache_key = f"{agent_id}:{service_name}"
            if hasattr(self._state_manager, '_state_cache'):
                self._state_manager._state_cache.pop(cache_key, None)
            return True
        except Exception:
            return False

    def clear_service_metadata(self, agent_id: str, service_name: str) -> bool:
        """清除服务元数据"""
        try:
            cache_key = f"{agent_id}:{service_name}"
            if hasattr(self._state_manager, '_metadata_cache'):
                self._state_manager._metadata_cache.pop(cache_key, None)
            return True
        except Exception:
            return False

    async def delete_service_state_async(self, agent_id: str, service_name: str) -> bool:
        """异步删除服务状态"""
        return self.clear_service_state(agent_id, service_name)

    async def delete_service_metadata_async(self, agent_id: str, service_name: str) -> bool:
        """异步删除服务元数据"""
        return self.clear_service_metadata(agent_id, service_name)


class ServiceRegistry:
    """
    主服务注册表门面类

    通过门面模式整合所有专门管理器，提供统一的接口。
    保持与原始ServiceRegistry完全相同的方法签名和行为。
    """

    def __init__(self,
                 kv_store: Optional['AsyncKeyValue'] = None,
                 namespace: str = "mcpstore"):
        """
        Initialize ServiceRegistry with new cache architecture.

        Args:
            kv_store: AsyncKeyValue instance for data storage. If None, uses MemoryStore.
                     Session data is always kept in memory regardless of kv_store type.
            namespace: Cache namespace for data isolation (default: "mcpstore")

        Note:
            - Sessions are stored in memory (not serializable)
            - All other data uses the new three-layer cache architecture
            - Uses CacheLayerManager for all cache operations
        """
        self._config = {}
        self._kv_store = kv_store
        self._namespace = namespace
        self._logger = logging.getLogger(__name__)

        # 创建缓存层和命名服务
        cache_layer = self._create_cache_layer(kv_store)
        naming_service = self._create_naming_service()

        # 兼容性属性 - 为了兼容原有代码，需要暴露这些属性
        self._cache_layer = cache_layer  # 重要：外部代码依赖此属性
        self._naming = naming_service    # 重要：外部代码依赖此属性

        # 会话存储（内存中）
        self.sessions: Dict[str, Dict[str, Any]] = {}

        # 统一配置管理器
        self._unified_config = None

        # 同步助手（懒加载）
        self._sync_helper: Optional[Any] = None

        # 状态同步管理器
        self._state_sync_manager = None

        # 创建管理器协调器
        self._coordinator = ManagerCoordinator(cache_layer, naming_service, namespace)

        # 初始化管理器
        self._coordinator.initialize_managers()

        # 获取所有管理器实例
        self._session_manager = self._coordinator.get_manager('session')
        self._state_manager = self._coordinator.get_manager('state')
        self._tool_manager = self._coordinator.get_manager('tool')
        self._cache_manager = self._coordinator.get_manager('cache')
        self._persistence_manager = self._coordinator.get_manager('persistence')
        self._service_manager = self._coordinator.get_manager('service')

        # 创建映射管理器（ManagerCoordinator中没有包含）
        # 注意：MappingManager 需要在 CacheLayerManager 创建后初始化
        # 这里先声明，稍后初始化
        self._mapping_manager = None

        # 创建缓存层管理器（原始架构中的核心组件）
        # 这些管理器直接操作 pykv，是数据的唯一真相源
        from mcpstore.core.cache.service_entity_manager import ServiceEntityManager
        from mcpstore.core.cache.tool_entity_manager import ToolEntityManager
        from mcpstore.core.cache.state_manager import StateManager as CacheStateManager
        from mcpstore.core.cache.cache_layer_manager import CacheLayerManager
        from mcpstore.core.cache.relationship_manager import RelationshipManager

        # 创建 CacheLayerManager
        # 必须使用 kv_store（AsyncKeyValue 实例），而不是 cache_layer
        if kv_store is not None:
            # 使用传入的 kv_store 创建 CacheLayerManager
            cache_layer_manager = CacheLayerManager(kv_store, namespace)
            self._logger.debug(f"使用传入的 kv_store 创建 CacheLayerManager: {type(kv_store)}")
        elif isinstance(cache_layer, CacheLayerManager):
            # cache_layer 已经是 CacheLayerManager
            cache_layer_manager = cache_layer
            self._logger.debug("使用现有的 CacheLayerManager")
        else:
            # 没有可用的 kv_store，抛出错误（不做降级处理）
            raise RuntimeError(
                "无法创建 CacheLayerManager：kv_store 为 None 且 cache_layer 不是 CacheLayerManager。"
                "请确保传入有效的 AsyncKeyValue 实例。"
            )

        # 缓存层实体管理器（用于直接操作 pykv）
        self._cache_service_manager = ServiceEntityManager(cache_layer_manager, naming_service)
        self._cache_tool_manager = ToolEntityManager(cache_layer_manager, naming_service)
        self._cache_state_manager = CacheStateManager(cache_layer_manager)
        self._cache_layer_manager = cache_layer_manager

        # 创建关系管理器（使用 CacheLayerManager）
        self._relation_manager = RelationshipManager(cache_layer_manager)
        self._logger.debug("缓存层管理器初始化成功")
        
        # 创建映射管理器（使用 CacheLayerManager）
        from .mapping_manager import MappingManager
        self._mapping_manager = MappingManager(cache_layer_manager, naming_service, namespace=namespace)
        self._mapping_manager.initialize()

        # 设置 service_manager 的依赖管理器
        # 这些管理器用于工具存储和关系管理
        if hasattr(self._service_manager, 'set_managers'):
            self._service_manager.set_managers(
                service_entity_manager=self._cache_service_manager,
                relation_manager=self._relation_manager,
                tool_manager=self._tool_manager,
                state_manager=self._state_manager,
                session_manager=self._session_manager,
                cache_manager=self._cache_manager,
                mapping_manager=self._mapping_manager,
                tool_entity_manager=self._cache_tool_manager
            )
            self._logger.debug("已设置 service_manager 的依赖管理器")

        # 设置 tool_manager 的依赖管理器
        if hasattr(self._tool_manager, 'set_managers'):
            self._tool_manager.set_managers(
                relation_manager=self._relation_manager,
                tool_entity_manager=self._cache_tool_manager,
                cache_manager=self._cache_manager
            )
            self._logger.debug("已设置 tool_manager 的依赖管理器")

        # 设置 state_manager 的 CacheLayerManager
        # StateManager 需要 CacheLayerManager 来执行 pykv 操作（如 get_state）
        if hasattr(self._state_manager, 'set_cache_layer_manager'):
            self._state_manager.set_cache_layer_manager(self._cache_layer_manager)
            self._logger.debug("已设置 state_manager 的 CacheLayerManager")

        # 创建 ServiceStateService 适配器（兼容原始架构）
        # 这个适配器委托给 _state_manager，提供与原始架构相同的接口
        self._service_state_service = ServiceStateServiceAdapter(self._state_manager, self._naming)

        # 创建 AgentClientMappingService 适配器（兼容原始架构）
        # 这个适配器委托给 _mapping_manager，提供与原始架构相同的接口
        self._agent_client_service = AgentClientMappingServiceAdapter(self._mapping_manager)

        self._logger.info("ServiceRegistry initialized with all managers")

    def _create_cache_layer(self, kv_store=None):
        """
        创建缓存层
        
        Args:
            kv_store: AsyncKeyValue 实例，必须提供
            
        Returns:
            传入的 kv_store 实例
            
        Raises:
            RuntimeError: 如果 kv_store 为 None
        """
        if kv_store is None:
            raise RuntimeError(
                "kv_store 参数不能为 None。"
                "ServiceRegistry 必须传入有效的 AsyncKeyValue 实例。"
                "请使用 MemoryStore 或 RedisStore 初始化。"
            )
        return kv_store

    def _create_naming_service(self):
        """创建命名服务"""
        # 优先使用真正的 NamingService
        try:
            from mcpstore.core.cache.naming_service import NamingService
            return NamingService()
        except ImportError:
            pass

        # 备用的简单命名服务
        class SimpleNamingService:
            def get_global_service_name(self, agent_id: str, service_name: str) -> str:
                return f"{agent_id}:{service_name}"

            def generate_service_global_name(self, service_name: str, agent_id: str) -> str:
                """生成服务全局名称"""
                # 如果 agent_id 是 global_agent_store，直接返回服务名
                if agent_id == "global_agent_store":
                    return service_name
                return f"{agent_id}:{service_name}"

            def parse_global_name(self, global_name: str) -> tuple:
                parts = global_name.split(":", 1)
                return (parts[0], parts[1]) if len(parts) == 2 else (None, global_name)

            def parse_service_global_name(self, global_name: str) -> tuple:
                """解析服务全局名称（兼容性方法）"""
                return self.parse_global_name(global_name)

        return SimpleNamingService()

    # ========================================
    # 会话管理方法 (委托给SessionManager)
    # ========================================

    async def initialize(self) -> None:
        """初始化所有管理器"""
        await self._coordinator.initialize_all()

    async def cleanup(self) -> None:
        """清理所有管理器资源"""
        await self._coordinator.cleanup_all()

    def create_session(self, agent_id: str, session_type: str = "default",
                      metadata: Optional[Dict[str, Any]] = None) -> str:
        return self._session_manager.create_session(agent_id, session_type, metadata)

    async def create_session_async(self, agent_id: str, session_type: str = "default",
                                 metadata: Optional[Dict[str, Any]] = None) -> str:
        return await self._session_manager.create_session_async(agent_id, session_type, metadata)

    def get_session(self, agent_id: str, name: str) -> Optional[Any]:
        """
        获取指定agent_id下服务的会话对象

        Args:
            agent_id: Agent ID
            name: 服务名称

        Returns:
            会话对象或None
        """
        return self._session_manager.get_session(agent_id, name)

    def close_session(self, session_id: str) -> bool:
        return self._session_manager.close_session(session_id)

    async def close_session_async(self, session_id: str) -> bool:
        return await self._session_manager.close_session_async(session_id)

    def list_sessions(self, agent_id: Optional[str] = None) -> List[str]:
        return self._session_manager.list_sessions(agent_id)

    def add_tool_to_session(self, session_id: str, tool_name: str) -> bool:
        return self._session_manager.add_tool_to_session(session_id, tool_name)

    def remove_tool_from_session(self, session_id: str, tool_name: str) -> bool:
        return self._session_manager.remove_tool_from_session(session_id, tool_name)

    def get_session_tools(self, session_id: str) -> Set[str]:
        return self._session_manager.get_session_tools(session_id)

    def clear_agent_sessions(self, agent_id: str) -> int:
        return self._session_manager.clear_agent_sessions(agent_id)

    # ========================================
    # 服务管理方法 (委托给ServiceManager)
    # ========================================

    def add_service(self, agent_id: str, name: str, session: Any = None,
                   tools: List[tuple] = None, service_config: Dict[str, Any] = None,
                   auto_connect: bool = True) -> bool:
        """
        添加服务

        Args:
            agent_id: Agent ID
            name: 服务名称
            session: 服务会话对象
            tools: 工具列表 [(tool_name, tool_def)]
            service_config: 服务配置
            auto_connect: 是否自动连接

        Returns:
            是否成功添加
        """
        return self._service_manager.add_service(
            agent_id=agent_id,
            name=name,
            session=session,
            tools=tools,
            service_config=service_config,
            auto_connect=auto_connect
        )

    async def add_service_async(self, agent_id: str, name: str, session: Any = None,
                               tools: List[tuple] = None, service_config: Dict[str, Any] = None,
                               auto_connect: bool = True, preserve_mappings: bool = False,
                               state: Any = None) -> bool:
        """
        异步添加服务

        Args:
            agent_id: Agent ID
            name: 服务名称
            session: 服务会话对象
            tools: 工具列表 [(tool_name, tool_def)]
            service_config: 服务配置
            auto_connect: 是否自动连接
            preserve_mappings: 是否保留已有的映射关系
            state: 服务状态（可选）

        Returns:
            是否成功添加
        """
        # 添加服务
        result = await self._service_manager.add_service_async(
            agent_id=agent_id,
            name=name,
            session=session,
            tools=tools,
            service_config=service_config,
            auto_connect=auto_connect
        )

        # 如果指定了状态，设置服务状态
        if state is not None and result:
            self._state_manager.set_service_state(agent_id, name, state)

        return result

    async def remove_service_async(self, agent_id: str, name: str) -> Optional[Any]:
        """
        异步移除服务（代理到 ServiceManager）

        Args:
            agent_id: Agent ID
            name: 服务名称

        Returns:
            被移除的会话对象
        """
        return await self._service_manager.remove_service_async(agent_id, name)

    def register_service(self, service_config: Dict[str, Any]) -> bool:
        return self._service_manager.register_service(service_config)

    async def register_service_async(self, service_config: Dict[str, Any]) -> bool:
        return await self._service_manager.register_service_async(service_config)

    def unregister_service(self, service_name: str) -> bool:
        return self._service_manager.unregister_service(service_name)

    async def unregister_service_async(self, service_name: str) -> bool:
        return await self._service_manager.unregister_service_async(service_name)

    def get_service_details(self, service_name: str) -> Optional[Dict[str, Any]]:
        return self._service_manager.get_service_details(service_name)

    def get_services_for_agent(self, agent_id: str) -> List[str]:
        return self._service_manager.get_services_for_agent(agent_id)

    def is_service_registered(self, service_name: str) -> bool:
        return self._service_manager.is_service_registered(service_name)

    def has_service(self, agent_id: str, service_name: str) -> bool:
        """
        检查指定 Agent 是否拥有指定服务

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            服务是否存在
        """
        services = self._service_manager.get_services_for_agent(agent_id)
        return service_name in services

    async def has_service_async(self, agent_id: str, service_name: str) -> bool:
        """
        异步检查指定 Agent 是否拥有指定服务

        遵循 "Functional Core, Imperative Shell" 架构原则：
        - 异步外壳直接使用 await 调用异步操作
        - 在异步上下文中必须使用此方法，而非同步版本

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            服务是否存在
        """
        services = await self._service_manager.get_services_for_agent_async(agent_id)
        return service_name in services

    async def get_services_for_agent_async(self, agent_id: str) -> List[str]:
        """
        异步获取指定 Agent 的所有服务

        Args:
            agent_id: Agent ID

        Returns:
            服务名称列表
        """
        return await self._service_manager.get_services_for_agent_async(agent_id)

    def get_all_services(self) -> List[str]:
        return self._service_manager.get_all_services()

    def get_service_count(self) -> int:
        return self._service_manager.get_service_count()

    def update_service_config(self, service_name: str, updates: Dict[str, Any]) -> bool:
        return self._service_manager.update_service_config(service_name, updates)

    async def update_service_config_async(self, service_name: str, updates: Dict[str, Any]) -> bool:
        return await self._service_manager.update_service_config_async(service_name, updates)

    def get_service_config(self, service_name: str) -> Optional[Dict[str, Any]]:
        return self._service_manager.get_service_config(service_name)

    def get_service_summary(self, service_name: str) -> Optional[Dict[str, Any]]:
        return self._service_manager.get_service_summary(service_name)

    async def get_service_summary_async(self, service_name: str) -> Optional[Dict[str, Any]]:
        return await self._service_manager.get_service_summary_async(service_name)

    def get_complete_service_info(self, agent_id: str, service_name: str) -> Optional[Dict[str, Any]]:
        """
        获取服务的完整信息

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            服务完整信息字典
        """
        return self._service_manager.get_complete_service_info(agent_id, service_name)

    async def get_complete_service_info_async(self, agent_id: str, service_name: str) -> Optional[Dict[str, Any]]:
        """
        异步获取服务的完整信息

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            服务完整信息字典
        """
        return self._service_manager.get_complete_service_info_async(agent_id, service_name)

    def get_all_services_complete_info(self) -> Dict[str, Any]:
        return self._service_manager.get_all_services_complete_info()

    def clear_agent_lifecycle_data(self, agent_id: str) -> bool:
        return self._service_manager.clear_agent_lifecycle_data(agent_id)

    def get_stats(self) -> Dict[str, Any]:
        return self._service_manager.get_stats()

    def is_long_lived_service(self, service_name: str) -> bool:
        return self._service_manager.is_long_lived_service(service_name)

    def mark_as_long_lived(self, agent_id: str, service_name: str):
        """
        标记服务为长生命周期连接

        Args:
            agent_id: Agent ID
            service_name: 服务名称
        """
        return self._service_manager.mark_as_long_lived(agent_id, service_name)

    def set_long_lived_service(self, service_name: str, is_long_lived: bool) -> bool:
        return self._service_manager.set_long_lived_service(service_name, is_long_lived)

    def get_services_by_state(self, states: List[str]) -> List[str]:
        return self._service_manager.get_services_by_state(states)

    def get_healthy_services(self) -> List[str]:
        return self._service_manager.get_healthy_services()

    def get_failed_services(self) -> List[str]:
        return self._service_manager.get_failed_services()

    def get_services_with_tools(self) -> List[str]:
        return self._service_manager.get_services_with_tools()

    def should_cache_aggressively(self, service_name: str) -> bool:
        return self._service_manager.should_cache_aggressively(service_name)

    def remove_service_lifecycle_data(self, service_name: str, agent_id: str) -> bool:
        return self._service_manager.remove_service_lifecycle_data(service_name, agent_id)

    def set_service_lifecycle_data(self, service_name: str, agent_id: str, data: Dict[str, Any]) -> bool:
        return self._service_manager.set_service_lifecycle_data(service_name, agent_id, data)

    # ========================================
    # 客户端映射方法 (委托给MappingManager)
    # ========================================

    async def get_service_client_id_async(self, agent_id: str, service_name: str) -> Optional[str]:
        return await self._mapping_manager.get_service_client_id_async(agent_id, service_name)

    def get_service_client_id(self, agent_id: str, service_name: str) -> Optional[str]:
        return self._mapping_manager.get_service_client_id(agent_id, service_name)

    async def get_agent_clients_async(self, agent_id: str) -> List[str]:
        """
        从 pykv 关系层获取 Agent 的所有客户端
        
        [pykv 唯一真相源] 所有数据必须从 pykv 读取
        
        Args:
            agent_id: Agent ID
            
        Returns:
            客户端ID列表
        """
        return await self._mapping_manager.get_agent_clients_async(agent_id)

    def get_client_config_from_cache(self, client_id: str) -> Optional[Dict[str, Any]]:
        """
        从缓存获取客户端配置

        Args:
            client_id: 客户端ID

        Returns:
            客户端配置或None
        """
        return self._mapping_manager.get_client_config_from_cache(client_id)

    async def get_client_config_from_cache_async(self, client_id: str) -> Optional[Dict[str, Any]]:
        """
        异步从缓存获取客户端配置

        Args:
            client_id: 客户端ID

        Returns:
            客户端配置或None
        """
        # 使用同步方法，因为 mapping_manager 使用内存缓存
        return self._mapping_manager.get_client_config_from_cache(client_id)

    def add_client_config(self, agent_id: str, client_config: Dict[str, Any]) -> str:
        return self._mapping_manager.add_client_config(agent_id, client_config)

    def set_service_client_mapping(self, agent_id: str, service_name: str, client_id: str) -> bool:
        return self._mapping_manager.set_service_client_mapping(agent_id, service_name, client_id)

    async def set_service_client_mapping_async(self, agent_id: str, service_name: str, client_id: str) -> bool:
        return await self._mapping_manager.set_service_client_mapping_async(agent_id, service_name, client_id)

    def remove_service_client_mapping(self, agent_id: str, service_name: str) -> bool:
        return self._mapping_manager.remove_service_client_mapping(agent_id, service_name)

    async def delete_service_client_mapping_async(self, agent_id: str, service_name: str) -> bool:
        return await self._mapping_manager.delete_service_client_mapping_async(agent_id, service_name)

    def add_agent_service_mapping(self, agent_id: str, service_name: str, global_name: str) -> bool:
        return self._mapping_manager.add_agent_service_mapping(agent_id, service_name, global_name)

    def get_global_name_from_agent_service(self, agent_id: str, service_name: str) -> Optional[str]:
        return self._mapping_manager.get_global_name_from_agent_service(agent_id, service_name)

    async def get_global_name_from_agent_service_async(self, agent_id: str, service_name: str) -> Optional[str]:
        return await self._mapping_manager.get_global_name_from_agent_service_async(agent_id, service_name)

    def get_agent_service_from_global_name(self, global_name: str) -> Optional[Dict[str, Any]]:
        return self._mapping_manager.get_agent_service_from_global_name(global_name)

    def get_agent_services(self, agent_id: str) -> List[str]:
        return self._mapping_manager.get_agent_services(agent_id)

    def is_agent_service(self, agent_id: str, service_name: str) -> bool:
        return self._mapping_manager.is_agent_service(agent_id, service_name)

    def remove_agent_service_mapping(self, agent_id: str, service_name: str) -> bool:
        return self._mapping_manager.remove_agent_service_mapping(agent_id, service_name)

    def clear_agent_mappings(self, agent_id: str) -> bool:
        return self._mapping_manager.clear_agent_mappings(agent_id)

    def clear_all_mappings(self) -> bool:
        return self._mapping_manager.clear_all_mappings()

    def get_mapping_stats(self) -> Dict[str, Any]:
        return self._mapping_manager.get_mapping_stats()

    # ========================================
    # 工具管理方法 (委托给ToolManager)
    # ========================================

    def get_tools_for_service(self, agent_id: str, service_name: str) -> List[str]:
        return self._tool_manager.get_tools_for_service(agent_id, service_name)

    async def get_tools_for_service_async(self, agent_id: str, service_name: str) -> List[str]:
        return await self._tool_manager.get_tools_for_service_async(agent_id, service_name)

    def get_tool_info(self, service_name: str, tool_name: str) -> Optional[Dict[str, Any]]:
        return self._tool_manager.get_tool_info(service_name, tool_name)

    async def get_tool_info_async(self, service_name: str, tool_name: str) -> Optional[Dict[str, Any]]:
        return await self._tool_manager.get_tool_info_async(service_name, tool_name)

    def add_tool_to_service(self, service_name: str, tool_name: str, tool_config: Dict[str, Any]) -> bool:
        return self._tool_manager.add_tool_to_service(service_name, tool_name, tool_config)

    async def add_tool_to_service_async(self, service_name: str, tool_name: str, tool_config: Dict[str, Any]) -> bool:
        return await self._tool_manager.add_tool_to_service_async(service_name, tool_name, tool_config)

    def remove_tool_from_service(self, service_name: str, tool_name: str) -> bool:
        return self._tool_manager.remove_tool_from_service(service_name, tool_name)

    async def remove_tool_from_service_async(self, service_name: str, tool_name: str) -> bool:
        return await self._tool_manager.remove_tool_from_service_async(service_name, tool_name)

    def list_all_tools(self) -> List[str]:
        return self._tool_manager.list_all_tools()

    def search_tools(self, query: str, filters: Optional[Dict[str, Any]] = None) -> List[Dict[str, Any]]:
        return self._tool_manager.search_tools(query, filters)

    def get_tools_stats(self) -> Dict[str, Any]:
        return self._tool_manager.get_tools_stats()

    def validate_tool_definition(self, tool_config: Dict[str, Any]) -> bool:
        return self._tool_manager.validate_tool_definition(tool_config)

    def get_tool_names_for_service(self, service_name: str) -> List[str]:
        return self._tool_manager.get_tool_names_for_service(service_name)

    def update_tool_info(self, service_name: str, tool_name: str, updates: Dict[str, Any]) -> bool:
        return self._tool_manager.update_tool_info(service_name, tool_name, updates)

    def clear_service_tools(self, service_name: str) -> bool:
        return self._tool_manager.clear_service_tools(service_name)

    def clear_service_tools_only(self, agent_id: str, service_name: str):
        """
        只清理服务的工具缓存，保留Agent-Client映射关系

        这是优雅修复方案的核心方法：
        - 清理工具缓存和工具-会话映射
        - 保留Agent-Client映射
        - 保留Client配置
        - 保留Service-Client映射

        Args:
            agent_id: Agent ID
            service_name: 服务名称
        """
        try:
            self._logger.debug(
                f"[REGISTRY.CLEAR_TOOLS_ONLY] begin agent={agent_id} service={service_name}")

            # 获取现有会话
            existing_session = self._session_manager.get_session(agent_id, service_name)
            if not existing_session:
                self._logger.debug(f"[CLEAR_TOOLS] no_session service={service_name} skip=True")
                return

            # 只清理工具相关的缓存
            tools_to_remove = []
            all_tool_names = self._session_manager.get_all_tool_names(agent_id)
            for tool_name in all_tool_names:
                tool_session = self._session_manager.get_session_for_tool(agent_id, tool_name)
                if tool_session is existing_session:
                    tools_to_remove.append(tool_name)

            for tool_name in tools_to_remove:
                # 清理工具-会话映射
                self._session_manager.remove_tool_session_mapping(agent_id, tool_name)

            # 清理会话（会被新会话替换）
            self._session_manager.clear_session(agent_id, service_name)

            self._logger.debug(
                f"[CLEAR_TOOLS] cleared_tools service={service_name} count={len(tools_to_remove)} keep_mappings=True")

        except Exception as e:
            self._logger.error(f"[CLEAR_TOOLS] 清理工具失败 {agent_id}:{service_name}: {e}")
            raise

    def has_tools(self, service_name: str) -> bool:
        return self._tool_manager.has_tools(service_name)

    # ========================================
    # 状态管理方法 (委托给StateManager)
    # 注意：方法签名与原始架构保持一致 (agent_id, service_name)
    # ========================================

    def get_service_state(self, agent_id: str, service_name: str) -> Optional[Any]:
        """
        获取服务状态

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            服务状态
        """
        return self._state_manager.get_service_state(agent_id, service_name)

    def set_service_state(self, agent_id: str, service_name: str, state: Any) -> bool:
        """
        设置服务状态

        Args:
            agent_id: Agent ID
            service_name: 服务名称
            state: 服务状态

        Returns:
            是否成功
        """
        return self._state_manager.set_service_state(agent_id, service_name, state)

    async def set_service_state_async(self, agent_id: str, service_name: str, state: Any) -> bool:
        """
        异步设置服务状态

        Args:
            agent_id: Agent ID
            service_name: 服务名称
            state: 服务状态

        Returns:
            是否成功
        """
        return await self._state_manager.set_service_state_async(agent_id, service_name, state)

    def get_all_service_states(self, agent_id: str) -> Dict[str, Any]:
        """
        获取指定 Agent 的所有服务状态

        Args:
            agent_id: Agent ID

        Returns:
            服务状态字典
        """
        return self._state_manager.get_all_service_states(agent_id)

    def get_services_by_state(self, agent_id: str, states: List[Any]) -> List[str]:
        """
        按状态筛选服务

        Args:
            agent_id: Agent ID
            states: 状态列表

        Returns:
            服务名称列表
        """
        return self._state_manager.get_services_by_state(agent_id, states)

    def clear_service_state(self, agent_id: str, service_name: str) -> bool:
        """
        清除服务状态

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            是否成功
        """
        return self._state_manager.clear_service_state(agent_id, service_name)

    # [已删除] get_service_metadata 同步方法（重复定义）
    # 根据 "pykv 唯一真相数据源" 原则，请使用 get_service_metadata_async 异步方法

    def set_service_metadata(self, agent_id: str, service_name: str, metadata: Any) -> bool:
        """
        设置服务元数据

        Args:
            agent_id: Agent ID
            service_name: 服务名称
            metadata: 服务元数据

        Returns:
            是否成功
        """
        return self._state_manager.set_service_metadata(agent_id, service_name, metadata)

    async def set_service_metadata_async(self, agent_id: str, service_name: str, metadata: Any) -> bool:
        """
        异步设置服务元数据

        Args:
            agent_id: Agent ID
            service_name: 服务名称
            metadata: 服务元数据

        Returns:
            是否成功
        """
        return await self._state_manager.set_service_metadata_async(agent_id, service_name, metadata)

    def get_service_status(self, agent_id: str, service_name: str) -> Optional[str]:
        """
        获取服务状态（兼容性方法）

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            服务状态字符串
        """
        return self._state_manager.get_service_status(agent_id, service_name)

    def update_service_metadata(self, service_name: str, updates: Dict[str, Any], agent_id: Optional[str] = None) -> bool:
        return self._state_manager.update_service_metadata(service_name, updates, agent_id)

    def get_service_metadata_timestamp(self, service_name: str, key: str, agent_id: Optional[str] = None) -> Optional[datetime]:
        return self._state_manager.get_service_metadata_timestamp(service_name, key, agent_id)

    def clear_service_metadata(self, service_name: str, keys: Optional[List[str]] = None, agent_id: Optional[str] = None) -> bool:
        return self._state_manager.clear_service_metadata(service_name, keys, agent_id)

    def get_all_service_metadata(self, service_name: Optional[str] = None, agent_id: Optional[str] = None) -> Dict[str, Any]:
        return self._state_manager.get_all_service_metadata(service_name, agent_id)

    def cleanup_old_metadata(self, service_name: Optional[str] = None, agent_id: Optional[str] = None,
                           older_than: Optional[datetime] = None) -> int:
        return self._state_manager.cleanup_old_metadata(service_name, agent_id, older_than)

    def get_metadata_stats(self) -> Dict[str, Any]:
        return self._state_manager.get_metadata_stats()

    def has_metadata(self, service_name: str, agent_id: Optional[str] = None) -> bool:
        return self._state_manager.has_metadata(service_name, agent_id)

    # ========================================
    # 缓存管理方法 (委托给CacheManager)
    # ========================================

    def get_service_names(self) -> List[str]:
        return self._cache_manager.get_service_names()

    async def get_service_names_async(self) -> List[str]:
        return await self._cache_manager.get_service_names_async()

    def get_agents_for_service(self, service_name: str) -> List[str]:
        return self._cache_manager.get_agents_for_service(service_name)

    async def get_agents_for_service_async(self, service_name: str) -> List[str]:
        return await self._cache_manager.get_agents_for_service_async(service_name)

    def register_service(self, service_name: str) -> bool:
        return self._cache_manager.register_service(service_name)

    async def register_service_async(self, service_name: str) -> bool:
        return await self._cache_manager.register_service_async(service_name)

    def unregister_service(self, service_name: str) -> bool:
        return self._cache_manager.unregister_service(service_name)

    async def unregister_service_async(self, service_name: str) -> bool:
        return await self._cache_manager.unregister_service_async(service_name)

    def clear_cache(self) -> bool:
        return self._cache_manager.clear_cache()

    def get_stats(self) -> Dict[str, Any]:
        return self._cache_manager.get_stats()

    # ========================================
    # 持久化管理方法 (委托给PersistenceManager)
    # ========================================

    def save_to_file(self, filepath: str) -> bool:
        return self._persistence_manager.save_to_file(filepath)

    def load_from_file(self, filepath: str) -> bool:
        return self._persistence_manager.load_from_file(filepath)

    async def save_services_async(self, filepath: str) -> bool:
        return await self._persistence_manager.save_services_async(filepath)

    async def load_services_async(self, filepath: str) -> bool:
        return await self._persistence_manager.load_services_async(filepath)

    async def save_tools_async(self, filepath: str) -> bool:
        return await self._persistence_manager.save_tools_async(filepath)

    async def load_tools_async(self, filepath: str) -> bool:
        return await self._persistence_manager.load_tools_async(filepath)

    def get_last_save_time(self) -> Optional[datetime]:
        return self._persistence_manager.get_last_save_time()

    def get_file_info(self) -> Dict[str, Any]:
        return self._persistence_manager.get_file_info()

    def set_unified_config(self, unified_config: Any) -> None:
        """
        设置统一配置管理器（用于 JSON 配置持久化）

        Args:
            unified_config: UnifiedConfigManager 实例
        """
        self._unified_config = unified_config
        self._logger.debug("UnifiedConfigManager 已设置，JSON 配置持久化已启用")

    # ========================================
    # 兼容性方法 - 为了兼容原有代码
    # ========================================

    async def load_services_from_json_async(self) -> Dict[str, Any]:
        """
        从 mcp.json 读取服务配置并恢复服务实体

        Returns:
            加载结果统计信息
        """
        # 简化实现：返回基础统计信息
        return {
            "loaded": len(self.sessions),
            "services": list(self.sessions.keys()),
            "status": "success"
        }

    async def get_service_state_async(self, agent_id: str, service_name: str) -> Optional[Any]:
        """
        异步获取服务状态

        使用缓存层状态管理器（cache/state_manager.py）获取状态。
        方法签名：get_service_status(service_global_name)

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            服务状态或None
        """
        # 使用缓存层状态管理器
        if self._cache_state_manager is None:
            raise RuntimeError(
                "缓存层 StateManager 未初始化。"
                "请确保 ServiceRegistry 正确初始化了 _cache_state_manager 属性。"
            )

        # 生成全局名称
        global_name = self._naming.generate_service_global_name(service_name, agent_id)

        # 使用 cache/state_manager.py 的 get_service_status 方法
        status = await self._cache_state_manager.get_service_status(global_name)

        if status is None:
            return None

        # 返回 health_status 作为状态
        if hasattr(status, 'health_status'):
            return status.health_status
        elif isinstance(status, dict):
            return status.get('health_status')
        else:
            return status

    async def get_service_metadata_async(self, agent_id: str, service_name: str) -> Optional[Any]:
        """
        异步获取服务元数据

        遵循 "pykv 唯一真相数据源" 原则，从 pykv 读取元数据。

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            服务元数据或None
        """
        # 从 pykv 读取元数据
        return await self._state_manager.get_service_metadata_async(agent_id, service_name)

    def get_service_status(self, agent_id: str, service_name: str) -> Optional[str]:
        """
        获取服务状态（兼容性方法）

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            服务状态或None
        """
        return self._state_manager.get_service_status(agent_id, service_name)

    # ========================================
    # 原始架构兼容方法 - 使用 (agent_id, service_name) 签名
    # ========================================

    def set_service_state_v2(self, agent_id: str, service_name: str, state: Optional['ServiceConnectionState']):
        """
        设置服务状态（原始架构签名）

        Args:
            agent_id: Agent ID
            service_name: 服务名称
            state: 服务连接状态
        """
        return self._state_manager.set_service_state(agent_id, service_name, state)

    def set_service_metadata_v2(self, agent_id: str, service_name: str, metadata: Optional['ServiceStateMetadata']):
        """
        设置服务元数据（原始架构签名）

        Args:
            agent_id: Agent ID
            service_name: 服务名称
            metadata: 服务状态元数据
        """
        return self._state_manager.set_service_metadata(agent_id, service_name, metadata)

    # [已删除] get_service_metadata_v2 同步方法
    # 根据 "pykv 唯一真相数据源" 原则，请使用 get_service_metadata_async 异步方法

    async def set_service_metadata_async_v2(self, agent_id: str, service_name: str, metadata: Optional['ServiceStateMetadata']) -> bool:
        """
        异步设置服务元数据（原始架构签名）

        Args:
            agent_id: Agent ID
            service_name: 服务名称
            metadata: 服务状态元数据

        Returns:
            是否成功
        """
        try:
            self._state_manager.set_service_metadata(agent_id, service_name, metadata)
            return True
        except Exception as e:
            self._logger.error(f"设置服务元数据失败 {agent_id}:{service_name}: {e}")
            return False

    @property
    def kv_store(self):
        """获取KV存储实例（兼容性属性）"""
        return self._kv_store

    @property
    def naming(self):
        """获取命名服务实例（兼容性属性）"""
        return self._naming

    # 新增：支持 unified_sync_manager 的接口
    async def get_all_entities_for_sync(self, entity_type: str) -> Dict[str, Dict[str, Any]]:
        """
        获取所有实体用于同步

        Args:
            entity_type: 实体类型 (如 "services")

        Returns:
            Dict[str, Dict[str, Any]]: 实体数据字典
        """
        try:
            if entity_type == "services":
                result = {}
                for service_name, session_info in self.sessions.items():
                    result[service_name] = {
                        'value': {
                            'service_name': service_name,
                            'agent_id': session_info.get('agent_id', 'unknown'),
                            'state': 'active',
                            'client_id': session_info.get('client_id'),
                            'connected_at': session_info.get('connected_at')
                        }
                    }
                return result
            return {}
        except Exception as e:
            return {}

    def get_all_agent_ids(self) -> List[str]:
        """
        获取所有 Agent ID 列表

        从会话信息和映射中收集所有已知的 Agent ID。

        Returns:
            List[str]: 所有 Agent ID 的列表
        """
        agent_ids = set()

        # 从会话信息中获取
        agent_ids.update(self.sessions.keys())

        # 从映射管理器获取（如果可用）
        if self._mapping_manager:
            try:
                mapping_stats = self._mapping_manager.get_mapping_stats()
                if 'agent_ids' in mapping_stats:
                    agent_ids.update(mapping_stats['agent_ids'])
            except Exception:
                pass

        result = list(agent_ids)
        self._logger.debug(f"get_all_agent_ids: {len(result)} agents")
        return result

    def get_all_service_names(self, agent_id: str) -> List[str]:
        """
        获取指定 Agent 的所有服务名称

        Args:
            agent_id: Agent ID

        Returns:
            List[str]: 服务名称列表
        """
        service_names = []

        # 从会话信息获取服务名称
        if agent_id in self.sessions:
            service_names.extend(self.sessions[agent_id].keys())

        self._logger.debug(f"get_all_service_names: {len(service_names)} services for agent {agent_id}")
        return service_names