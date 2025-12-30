"""
依赖注入容器 - 循环导入的优雅解决方案
通过依赖注入解决循环导入问题，提高代码的可测试性和可维护性
"""

import asyncio
import logging
from dataclasses import dataclass
from enum import Enum
from typing import Dict, Any, Type, TypeVar, Callable, Optional

logger = logging.getLogger(__name__)

T = TypeVar('T')


class DependencyScope(Enum):
    """依赖生命周期范围"""
    SINGLETON = "singleton"        # 单例，整个应用生命周期内只创建一次
    TRANSIENT = "transient"        # 瞬时，每次请求都创建新实例
    SCOPED = "scoped"              # 作用域，在特定范围内复用（如asyncio.Task）


@dataclass
class ServiceDescriptor:
    """服务描述符"""
    interface: Type
    implementation: Optional[Type] = None
    factory: Optional[Callable] = None
    instance: Optional[Any] = None
    scope: DependencyScope = DependencyScope.SINGLETON
    dependencies: Dict[str, str] = None  # 依赖映射：参数名 -> 服务名

    def __post_init__(self):
        if self.dependencies is None:
            self.dependencies = {}


class DIContainer:
    """依赖注入容器"""

    def __init__(self):
        self._services: Dict[str, ServiceDescriptor] = {}
        self._singletons: Dict[str, Any] = {}
        self._scoped: Dict[str, Dict[str, Any]] = {}  # task_id -> instances
        self._lock = asyncio.Lock()

    def register_singleton(self, service_name: str, interface: Type,
                          implementation: Type = None, factory: Callable = None,
                          instance: Any = None):
        """注册单例服务"""
        descriptor = ServiceDescriptor(
            interface=interface,
            implementation=implementation,
            factory=factory,
            instance=instance,
            scope=DependencyScope.SINGLETON
        )
        self._services[service_name] = descriptor

    def register_transient(self, service_name: str, interface: Type,
                          implementation: Type = None, factory: Callable = None):
        """注册瞬时服务"""
        descriptor = ServiceDescriptor(
            interface=interface,
            implementation=implementation,
            factory=factory,
            scope=DependencyScope.TRANSIENT
        )
        self._services[service_name] = descriptor

    def register_scoped(self, service_name: str, interface: Type,
                       implementation: Type = None, factory: Callable = None):
        """注册作用域服务"""
        descriptor = ServiceDescriptor(
            interface=interface,
            implementation=implementation,
            factory=factory,
            scope=DependencyScope.SCOPED
        )
        self._services[service_name] = descriptor

    async def resolve(self, service_name: str) -> Any:
        """解析服务"""
        async with self._lock:
            if service_name not in self._services:
                raise ValueError(f"Service {service_name} not registered")

            descriptor = self._services[service_name]

            # 检查作用域
            if descriptor.scope == DependencyScope.SINGLETON:
                return await self._resolve_singleton(service_name, descriptor)
            elif descriptor.scope == DependencyScope.SCOPED:
                return await self._resolve_scoped(service_name, descriptor)
            else:  # TRANSIENT
                return await self._resolve_transient(descriptor)

    async def _resolve_singleton(self, service_name: str,
                               descriptor: ServiceDescriptor) -> Any:
        """解析单例服务"""
        if service_name in self._singletons:
            return self._singletons[service_name]

        instance = await self._create_instance(descriptor)
        self._singletons[service_name] = instance
        return instance

    async def _resolve_scoped(self, service_name: str,
                            descriptor: ServiceDescriptor) -> Any:
        """解析作用域服务"""
        task_id = id(asyncio.current_task())

        if task_id not in self._scoped:
            self._scoped[task_id] = {}

        if service_name in self._scoped[task_id]:
            return self._scoped[task_id][service_name]

        instance = await self._create_instance(descriptor)
        self._scoped[task_id][service_name] = instance
        return instance

    async def _resolve_transient(self, descriptor: ServiceDescriptor) -> Any:
        """解析瞬时服务"""
        return await self._create_instance(descriptor)

    async def _create_instance(self, descriptor: ServiceDescriptor) -> Any:
        """创建服务实例"""
        # 如果已有实例（单例预注册）
        if descriptor.instance is not None:
            return descriptor.instance

        # 使用工厂方法
        if descriptor.factory:
            return await self._invoke_factory(descriptor.factory)

        # 使用实现类
        if descriptor.implementation:
            return await self._create_implementation(descriptor)

        raise ValueError(f"No way to create instance for {descriptor.interface}")

    async def _invoke_factory(self, factory: Callable) -> Any:
        """调用工厂方法"""
        # 检查是否需要注入依赖
        import inspect
        sig = inspect.signature(factory)

        if not sig.parameters:
            # 无参数工厂
            if asyncio.iscoroutinefunction(factory):
                return await factory()
            else:
                return factory()

        # 需要依赖注入
        kwargs = {}
        for param_name, param in sig.parameters.items():
            if param_name in ['container', 'di_container']:
                kwargs[param_name] = self
            else:
                # 尝试从容器解析依赖
                dependency_name = param_name
                service_desc = self._services.get(dependency_name)
                if service_desc:
                    kwargs[param_name] = await self.resolve(dependency_name)

        if asyncio.iscoroutinefunction(factory):
            return await factory(**kwargs)
        else:
            return factory(**kwargs)

    async def _create_implementation(self, descriptor: ServiceDescriptor) -> Any:
        """创建实现类实例"""
        # 解析依赖
        dependencies = {}
        for dep_param, dep_service in descriptor.dependencies.items():
            dependencies[dep_param] = await self.resolve(dep_service)

        # 创建实例
        instance = descriptor.implementation(**dependencies)
        return instance

    def clear_scoped(self, task_id: int = None):
        """清理作用域实例"""
        if task_id is None:
            task_id = id(asyncio.current_task())
        self._scoped.pop(task_id, None)

    async def dispose(self):
        """释放容器资源"""
        # 清理单例
        for instance in self._singletons.values():
            if hasattr(instance, 'close') and asyncio.iscoroutinefunction(instance.close):
                await instance.close()

        # 清理作用域
        self._scoped.clear()
        self._singletons.clear()


# 全局容器实例
_global_container: Optional[DIContainer] = None


def get_container() -> DIContainer:
    """获取全局依赖注入容器"""
    global _global_container
    if _global_container is None:
        _global_container = DIContainer()
    return _global_container


async def configure_services(container: DIContainer):
    """配置所有服务"""
    # 配置配置服务
    container.register_singleton(
        "config_service",
        interface=object,  # 实际应该是 IConfigService 接口
        factory=lambda container: container.resolve("config_factory")
    )

    # 配置配置工厂
    container.register_singleton(
        "config_factory",
        interface=object,
        factory=create_config_service
    )

    # 配置其他服务...
    logger.info("Dependency injection container configured")


async def create_config_service(container: DIContainer) -> 'ConfigService':
    """创建配置服务的工厂方法"""
    # 延迟导入避免循环依赖
    from mcpstore.core.configuration.config_service import ConfigService

    # 获取依赖的KV存储
    kv_store = await container.resolve("kv_store")

    return ConfigService(kv_store)


# 使用示例
class ExampleService:
    """示例服务，展示依赖注入"""

    def __init__(self, config_service: 'ConfigService', container: DIContainer):
        self.config_service = config_service
        self.container = container

    async def get_config(self, key: str) -> Any:
        return await self.config_service.get_config(key)


async def setup_di_container():
    """设置依赖注入容器"""
    container = get_container()

    # 注册核心服务
    container.register_singleton(
        "kv_store",
        interface=object,
        factory=lambda: MockAsyncKeyValue()  # 或者真实的KV存储
    )

    await configure_services(container)

    return container