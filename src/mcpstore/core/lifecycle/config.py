"""
Service Lifecycle Configuration

保持核心模块的导入路径稳定，统一复用 config.config_dataclasses.ServiceLifecycleConfig，避免旧字段缺失。
"""

from mcpstore.config.config_dataclasses import ServiceLifecycleConfig

__all__ = ["ServiceLifecycleConfig"]
