"""
优雅的注册表实现 - 组合模式 + 接口抽象
展示真正的工厂类设计模式
"""

import logging
from dataclasses import dataclass
from typing import Dict, Any, List, Optional, Protocol, Type

logger = logging.getLogger(__name__)

# === 1. 定义接口协议（Protocol是更现代的抽象方式） ===

class IServiceStateService(Protocol):
    """服务状态服务接口"""
    def get_service_state(self, agent_id: str, service_name: str): ...
    def set_service_state(self, agent_id: str, service_name: str, state): ...
    async def get_service_metadata_async(self, agent_id: str, service_name: str): ...
    def set_service_metadata(self, agent_id: str, service_name: str, metadata): ...
    def has_service(self, agent_id: str, service_name: str) -> bool: ...

class IAgentClientMappingService(Protocol):
    """代理客户端映射服务接口"""
    async def get_agent_clients_async(self, agent_id: str) -> List[str]: ...
    def add_service_client_mapping(self, agent_id: str, service_name: str, client_id: str): ...
    def get_service_client_id(self, agent_id: str, service_name: str): ...

class IClientConfigService(Protocol):
    """客户端配置服务接口"""
    def get_client_config_from_cache(self, client_id: str) -> Optional[Dict[str, Any]]: ...
    def add_client_config(self, client_id: str, config: Dict[str, Any]): ...

# === 2. 服务工厂类（真正的工厂模式） ===

@dataclass
class RegistryServiceFactory:
    """注册表服务工厂 - 优雅的工厂模式实现"""

    # 使用Protocol类型注解，支持依赖注入
    service_state_service: IServiceStateService
    agent_client_service: IAgentClientMappingService
    client_config_service: IClientConfigService

    @classmethod
    def create(cls,
               service_state_impl: Type,
               agent_client_impl: Type,
               client_config_impl: Type,
               **kwargs) -> 'RegistryServiceFactory':
        """
        工厂方法 - 根据具体实现类创建工厂实例

        Args:
            service_state_impl: 服务状态服务的具体实现类
            agent_client_impl: 代理客户端映射服务的具体实现类
            client_config_impl: 客户端配置服务的具体实现类
            **kwargs: 传递给实现类的参数

        Returns:
            RegistryServiceFactory: 工厂实例
        """
        # 真正的工厂模式 - 创建具体服务实例
        service_state_service = service_state_impl(**kwargs)
        agent_client_service = agent_client_impl(**kwargs)
        client_config_service = client_config_impl(**kwargs)

        return cls(
            service_state_service=service_state_service,
            agent_client_service=agent_client_service,
            client_config_service=client_config_service
        )

    def create_service_registry(self) -> 'ElegantServiceRegistry':
        """
        工厂方法 - 创建服务注册表实例

        Returns:
            ElegantServiceRegistry: 优雅的服务注册表
        """
        return ElegantServiceRegistry(factory=self)

# === 3. 优雅的注册表实现（组合模式） ===

class ElegantServiceRegistry:
    """优雅的服务注册表 - 使用组合模式和工厂模式"""

    def __init__(self, factory: RegistryServiceFactory):
        """
        通过工厂注入所有服务依赖

        Args:
            factory: 服务工厂实例
        """
        self._factory = factory
        self._services = {
            'state': factory.service_state_service,
            'client_mapping': factory.agent_client_service,
            'client_config': factory.client_config_service
        }

        logger.info("ElegantServiceRegistry initialized with dependency injection")

    # === 动态方法代理 - 使用__getattr__实现优雅的委托 ===

    def __getattr__(self, name: str):
        """
        动态方法代理 - 优雅的委托模式

        当访问不存在的方法时，自动查找并调用对应的服务方法
        """
        # 查找哪个服务有这个方法
        for service_name, service in self._services.items():
            if hasattr(service, name):
                method = getattr(service, name)
                logger.debug(f"Method '{name}' proxied to {service_name}")
                return method

        # 如果没有找到，抛出更清晰的错误
        available_methods = []
        for service_name, service in self._services.items():
            available_methods.extend([f"{service_name}.{m}" for m in dir(service) if not m.startswith('_')])

        raise AttributeError(
            f"Method '{name}' not found in any service. "
            f"Available methods: {available_methods[:10]}..."  # 只显示前10个避免太长
        )

    # === 显式委托方法（可选，用于性能关键路径） ===

    def get_service_state(self, agent_id: str, service_name: str):
        """显式委托方法 - 性能优化"""
        return self._factory.service_state_service.get_service_state(agent_id, service_name)

    async def get_service_metadata_async(self, agent_id: str, service_name: str):
        """显式委托方法 - 从 pykv 异步获取元数据"""
        return await self._factory.service_state_service.get_service_metadata_async(agent_id, service_name)

    def set_service_metadata(self, agent_id: str, service_name: str, metadata):
        """显式委托方法 - 性能优化"""
        return self._factory.service_state_service.set_service_metadata(agent_id, service_name, metadata)

    def has_service(self, agent_id: str, service_name: str) -> bool:
        """显式委托方法 - 性能优化"""
        return self._factory.service_state_service.has_service(agent_id, service_name)

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
        return await self._factory.service_state_service.has_service_async(agent_id, service_name)

    async def get_agent_clients_async(self, agent_id: str) -> List[str]:
        """显式委托方法 - 从 pykv 获取 Agent 客户端"""
        return await self._factory.agent_client_service.get_agent_clients_async(agent_id)

    def get_client_config_from_cache(self, client_id: str) -> Optional[Dict[str, Any]]:
        """显式委托方法 - 性能优化"""
        return self._factory.client_config_service.get_client_config_from_cache(client_id)

    def add_client_config(self, client_id: str, config: Dict[str, Any]):
        """显式委托方法 - 性能优化"""
        return self._factory.client_config_service.add_client_config(client_id, config)

    # === 组合模式的高级功能 ===

    def replace_service(self, service_type: str, new_service):
        """
        运行时替换服务实现 - 真正的组合模式优势

        Args:
            service_type: 服务类型 ('state', 'client_mapping', 'client_config')
            new_service: 新的服务实例
        """
        if service_type in self._services:
            old_service = self._services[service_type]
            self._services[service_type] = new_service
            logger.info(f"Replaced {service_type} service: {type(old_service)} -> {type(new_service)}")
        else:
            raise ValueError(f"Unknown service type: {service_type}")

    def get_service_info(self) -> Dict[str, str]:
        """获取当前服务信息 - 用于调试"""
        return {
            name: f"{type(service).__module__}.{type(service).__name__}"
            for name, service in self._services.items()
        }
