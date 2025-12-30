"""
Registry Factory - 简化工厂模式实现
通过工厂模式创建服务注册表

这个工厂利用现有的kv_store_factory模式，提供统一的服务创建接口。
ServiceRegistry 使用新的三层缓存架构，内部自己创建所有管理器。
"""

import logging
from abc import ABC, abstractmethod
from typing import Dict, Any, Optional

from .core_registry import ServiceRegistry
from .kv_store_factory import _build_kv_store

logger = logging.getLogger(__name__)


class RegistryFactoryInterface(ABC):
    """注册表工厂接口 - 定义统一创建接口"""

    @abstractmethod
    def create_service_registry(self, kv_store) -> 'ServiceRegistry':
        """创建服务注册表"""
        pass


class ProductionRegistryFactory(RegistryFactoryInterface):
    """
    生产级注册表工厂 - 简化实现

    特点：
    - ServiceRegistry 使用新的三层缓存架构
    - ServiceRegistry 内部自己创建所有管理器（CacheLayerManager、NamingService 等）
    - 工厂只负责传递 kv_store
    """

    @staticmethod
    def create_service_registry(kv_store, namespace: str = "mcpstore") -> 'ServiceRegistry':
        """
        通过工厂模式创建ServiceRegistry实例

        Args:
            kv_store: 键值存储实例（由kv_store_factory创建）
            namespace: 缓存命名空间（默认: "mcpstore"）

        Returns:
            ServiceRegistry: 配置完成的注册表实例

        Raises:
            RuntimeError: 如果服务创建失败
        """
        try:
            logger.debug("Creating ServiceRegistry with new cache architecture")

            # ServiceRegistry 使用新的三层缓存架构
            # 内部会自动创建：
            # - CacheLayerManager
            # - NamingService
            # - ServiceEntityManager
            # - ToolEntityManager
            # - RelationshipManager
            # - StateManager
            registry = ServiceRegistry(
                kv_store=kv_store,
                namespace=namespace
            )

            logger.info("ServiceRegistry created via factory pattern (new cache architecture)")
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
    """测试用注册表工厂 - 简化实现"""

    def __init__(self, namespace: str = "test"):
        self.namespace = namespace

    def create_service_registry(self, kv_store) -> 'ServiceRegistry':
        """创建测试用注册表"""
        logger.debug("Creating ServiceRegistry for testing")

        # ServiceRegistry 使用新的三层缓存架构
        # 测试时使用 "test" 命名空间隔离数据
        return ServiceRegistry(
            kv_store=kv_store,
            namespace=self.namespace
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


def create_registry_from_kv_store(kv_store, test_mode: bool = False, namespace: str = "mcpstore") -> 'ServiceRegistry':
    """
    从KV存储创建注册表

    Args:
        kv_store: KV存储实例
        test_mode: 是否使用测试工厂
        namespace: 缓存命名空间（默认: "mcpstore"）

    Returns:
        ServiceRegistry: 创建的注册表实例
    """
    if test_mode:
        return TestRegistryFactory(namespace=namespace).create_service_registry(kv_store)
    else:
        return ProductionRegistryFactory.create_service_registry(kv_store, namespace=namespace)


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
