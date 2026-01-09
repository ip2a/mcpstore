"""
自动化委托方法生成器 - 优雅解决 ServiceRegistry 缺失方法问题

这个模块提供了一个优雅的解决方案，通过自动生成委托方法来处理
ServiceRegistry 重构后的接口兼容性问题，而不是手动一个一个添加委托方法。
"""

import inspect
import logging
from typing import Any, Dict, Optional, Set, Type

logger = logging.getLogger(__name__)


class DelegationGenerator:
    """
    自动化委托方法生成器

    这个类可以自动扫描服务类的方法，并在目标类中生成对应的委托方法，
    确保向后兼容性和接口一致性。
    """

    def __init__(self, target_class: Type):
        """
        初始化委托生成器

        Args:
            target_class: 要添加委托方法的目标类（通常是 ServiceRegistry）
        """
        self.target_class = target_class
        self.delegated_methods: Set[str] = set()

    def generate_delegation_methods(
        self,
        service_instance: Any,
        service_name: str,
        method_filter: Optional[callable] = None
    ) -> None:
        """
        为指定的服务实例生成委托方法

        Args:
            service_instance: 服务实例（如 self._service_state_service）
            service_name: 服务名称（用于错误消息）
            method_filter: 可选的方法过滤器函数
        """
        service_class = service_instance.__class__

        # 获取所有公共方法
        for method_name, method in inspect.getmembers(service_class, predicate=inspect.ismethod):
            if method_name.startswith('_'):
                continue  # 跳过私有方法

            # 应用方法过滤器
            if method_filter and not method_filter(method_name, method):
                continue

            # 检查目标类是否已有此方法
            if hasattr(self.target_class, method_name):
                continue  # 跳过已存在的方法

            # 生成委托方法
            try:
                self._generate_single_delegation_method(service_instance, method_name, service_name)
                self.delegated_methods.add(method_name)
            except Exception as e:
                logger.warning(f"Failed to generate delegation for {service_name}.{method_name}: {e}")

        # 处理静态方法和类方法
        for attr_name in dir(service_class):
            if attr_name.startswith('_'):
                continue

            attr = getattr(service_class, attr_name)
            if inspect.isfunction(attr):
                if hasattr(self.target_class, attr_name):
                    continue

                # 应用方法过滤器
                if method_filter and not method_filter(attr_name, attr):
                    continue

                try:
                    self._generate_single_delegation_method(service_instance, attr_name, service_name)
                    self.delegated_methods.add(attr_name)
                except Exception as e:
                    logger.warning(f"Failed to generate delegation for {service_name}.{attr_name}: {e}")

    def _generate_single_delegation_method(self, service_instance: Any, method_name: str, service_name: str):
        """
        生成单个委托方法

        Args:
            service_instance: 服务实例
            method_name: 方法名称
            service_name: 服务名称
        """
        method = getattr(service_instance.__class__, method_name)
        signature = inspect.signature(method)

        # 生成方法文档
        docstring = f"""委托方法: {service_name}.{method_name}

        这个方法自动生成，委托给 {service_name} 服务处理。
        参见 {service_name}.{method_name} 的详细文档。
        """

        # 生成方法体
        method_body = self._generate_method_body(service_instance, method_name, signature)

        # 在目标类中添加方法
        setattr(self.target_class, method_name, method_body)

    def _generate_method_body(self, service_instance: Any, method_name: str, signature) -> callable:
        """
        生成方法体

        Args:
            service_instance: 服务实例
            method_name: 方法名称
            signature: 方法签名

        Returns:
            生成的方法
        """
        # 获取服务实例的属性名（如 self._service_state_service）
        service_attr_name = None
        for attr_name, attr_value in self.target_class.__dict__.items():
            if attr_value is service_instance:
                service_attr_name = attr_name
                break

        if not service_attr_name:
            raise ValueError(f"Cannot find service instance attribute for {service_instance}")

        # 根据方法签名生成适当的方法
        if inspect.iscoroutinefunction(getattr(service_instance.__class__, method_name)):
            return self._generate_async_method_body(service_attr_name, method_name, signature)
        else:
            return self._generate_sync_method_body(service_attr_name, method_name, signature)

    def _generate_sync_method_body(self, service_attr_name: str, method_name: str, signature) -> callable:
        """生成同步方法体"""
        def delegating_method(self, *args, **kwargs):
            """自动生成的委托方法"""
            service = getattr(self, service_attr_name)
            method = getattr(service, method_name)
            return method(*args, **kwargs)

        # 设置方法签名和文档
        delegating_method.__name__ = method_name
        delegating_method.__qualname__ = f"{self.target_class.__name__}.{method_name}"
        delegating_method.__signature__ = signature
        delegating_method.__doc__ = f"""委托方法: {service_attr_name}.{method_name}"""

        return delegating_method

    def _generate_async_method_body(self, service_attr_name: str, method_name: str, signature) -> callable:
        """生成异步方法体"""
        async def async_delegating_method(self, *args, **kwargs):
            """自动生成的异步委托方法"""
            service = getattr(self, service_attr_name)
            method = getattr(service, method_name)
            return await method(*args, **kwargs)

        # 设置方法签名和文档
        async_delegating_method.__name__ = method_name
        async_delegating_method.__qualname__ = f"{self.target_class.__name__}.{method_name}"
        async_delegating_method.__signature__ = signature
        async_delegating_method.__doc__ = f"""异步委托方法: {service_attr_name}.{method_name}"""

        return async_delegating_method


def auto_generate_delegations(target_instance: Any, service_mappings: Dict[str, Any]) -> None:
    """
    自动生成所有委托方法的便捷函数

    Args:
        target_instance: 目标实例（通常是 ServiceRegistry 实例）
        service_mappings: 服务映射字典 {service_name: service_instance}
    """
    generator = DelegationGenerator(target_instance.__class__)

    for service_name, service_instance in service_mappings.items():
        logger.info(f"Generating delegation methods for {service_name}")

        # 定义方法过滤器 - 只生成公共方法，跳过已经存在的方法
        def method_filter(method_name: str, method: Any) -> bool:
            # 跳过特殊方法
            if method_name.startswith('_'):
                return False
            # 跳过已存在的方法
            if hasattr(target_instance.__class__, method_name):
                return False
            # 只生成函数方法
            if not (inspect.isfunction(method) or inspect.ismethod(method)):
                return False
            return True

        generator.generate_delegation_methods(
            service_instance,
            service_name,
            method_filter
        )

    logger.info(f"Generated {len(generator.delegated_methods)} delegation methods: {sorted(generator.delegated_methods)}")