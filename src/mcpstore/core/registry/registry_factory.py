"""
Registry Factory - 零委托模式实现
通过工厂模式创建服务注册表，避免委托方法

这个工厂利用现有的kv_store_factory模式，提供统一的服务创建接口。
"""

import logging
from typing import Dict, Any, Optional, Union
from abc import ABC, abstractmethod

from .kv_store_factory import _build_kv_store
from .service_state_service import ServiceStateService
from .agent_client_mapping_service import AgentClientMappingService
from .client_config_service import ClientConfigService
from .core_registry import ServiceRegistry

logger = logging.getLogger(__name__)


class RegistryFactoryInterface(ABC):
    """注册表工厂接口 - 定义统一创建接口"""

    @abstractmethod
    def create_service_registry(self, kv_store) -> 'ServiceRegistry':
        """创建服务注册表"""
        pass


class ProductionRegistryFactory(RegistryFactoryInterface):
    """
    生产级注册表工厂 - 零委托模式实现

    特点：
    - 真正的工厂模式，而非简单委托
    - 依赖注入，而非内部创建
    - 利用现有kv_store_factory成熟模式
    - 保持API兼容性
    """

    @staticmethod
    def create_service_registry(kv_store) -> 'ServiceRegistry':
        """
        通过工厂模式创建ServiceRegistry实例

        Args:
            kv_store: 键值存储实例（由kv_store_factory创建）

        Returns:
            ServiceRegistry: 配置完成的注册表实例

        Raises:
            RuntimeError: 如果服务创建失败
        """
        try:
            logger.debug("Creating service services via dependency injection")

            # 1. 创建共享的基础组件
            from .state_backend import KVRegistryStateBackend
            from .kv_storage_adapter import KVStorageAdapter

            state_backend = KVRegistryStateBackend(kv_store)
            kv_adapter = KVStorageAdapter(kv_store)

            # 2. 创建所有必需的服务组件（依赖注入）
            # ServiceStateService需要sync_helper，我们提供一个lambda
            def sync_helper_provider():
                # 延迟创建sync_helper，与ServiceRegistry保持一致
                from ..store.base_store import AsyncSyncHelper
                return AsyncSyncHelper()

            service_state_service = ServiceStateService(
                kv_store=kv_store,
                state_backend=state_backend,
                kv_adapter=kv_adapter,
                sync_helper=sync_helper_provider
            )

            agent_client_service = AgentClientMappingService(
                kv_store,
                state_backend,
                kv_adapter
            )
            client_config_service = ClientConfigService(kv_store, state_backend, kv_adapter)

            logger.debug("All service components created successfully")

            # 3. 创建注册表实例（直接依赖注入，零委托）
            # ToolManagementService将在ServiceRegistry内部创建（需要registry引用）
            registry = ServiceRegistry(
                kv_store=kv_store,
                service_state_service=service_state_service,
                agent_client_service=agent_client_service,
                client_config_service=client_config_service,
                tool_management_service=None  # 将在ServiceRegistry中创建
            )

            logger.info("ServiceRegistry created via factory pattern (zero delegation)")
            return registry

        except Exception as e:
            logger.error(f"Failed to create ServiceRegistry via factory: {e}")
            raise RuntimeError(f"Registry creation failed: {e}") from e

    @staticmethod
    def create_from_config(config: Optional[Dict[str, Any]] = None) -> 'ServiceRegistry':
        """
        从配置创建注册表

        Args:
            config: 配置字典

        Returns:
            ServiceRegistry: 配置完成的注册表实例
        """
        # 使用现有的kv_store_factory创建存储后端
        kv_store = _build_kv_store(config)

        # 委托给主工厂方法
        return ProductionRegistryFactory.create_service_registry(kv_store)


class TestRegistryFactory(RegistryFactoryInterface):
    """测试用注册表工厂 - 支持模拟依赖注入"""

    def __init__(self,
                 mock_service_state_service=None,
                 mock_agent_client_service=None,
                 mock_client_config_service=None):
        self.mock_service_state_service = mock_service_state_service
        self.mock_agent_client_service = mock_agent_client_service
        self.mock_client_config_service = mock_client_config_service

    def create_service_registry(self, kv_store) -> 'ServiceRegistry':
        """创建带模拟依赖的注册表（用于测试）"""
        logger.debug("Creating ServiceRegistry with mock dependencies")

        # 注入模拟依赖（如果提供）
        service_state_service = (
            self.mock_service_state_service or
            ServiceStateService(kv_store)
        )
        agent_client_service = (
            self.mock_agent_client_service or
            AgentClientMappingService(
                kv_store,
                service_state_service._state_backend,
                service_state_service._kv_adapter
            )
        )
        client_config_service = (
            self.mock_client_config_service or
            ClientConfigService(kv_store)
        )

        return ServiceRegistry(
            kv_store=kv_store,
            service_state_service=service_state_service,
            agent_client_service=agent_client_service,
            client_config_service=client_config_service
        )


# 公共工厂接口
def create_registry_from_config(config: Optional[Dict[str, Any]] = None,
                                test_mode: bool = False) -> 'ServiceRegistry':
    """
    创建注册表的公共接口

    Args:
        config: 配置字典
        test_mode: 是否使用测试工厂

    Returns:
        ServiceRegistry: 创建的注册表实例
    """
    if test_mode:
        return TestRegistryFactory().create_service_registry(None)  # kv_store在测试中被mock
    else:
        return ProductionRegistryFactory.create_from_config(config)


def create_registry_from_kv_store(kv_store, test_mode: bool = False) -> 'ServiceRegistry':
    """
    从KV存储创建注册表

    Args:
        kv_store: KV存储实例
        test_mode: 是否使用测试工厂

    Returns:
        ServiceRegistry: 创建的注册表实例
    """
    if test_mode:
        return TestRegistryFactory().create_service_registry(kv_store)
    else:
        return ProductionRegistryFactory.create_service_registry(kv_store)


# 向后兼容的工厂函数
def create_service_registry(kv_store) -> 'ServiceRegistry':
    """
    向后兼容的工厂函数

    Args:
        kv_store: KV存储实例

    Returns:
        ServiceRegistry: 创建的注册表实例
    """
    return ProductionRegistryFactory.create_service_registry(kv_store)