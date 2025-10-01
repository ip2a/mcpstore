"""
基础设施层模块

包含依赖注入容器和其他基础设施组件：
- ServiceContainer: 依赖注入容器
"""

from .container import ServiceContainer

__all__ = [
    "ServiceContainer",
]

