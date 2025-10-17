"""
MCPStore FastMCP Authentication Module
FastMCP认证配置封装模块 - 完全基于FastMCP的认证功能
"""

# 注意：复杂的认证构建器已移除，现在使用简化的 auth/headers 参数方式
# 如需复杂认证配置，请直接使用 FastMCP 的原生API

from .types import (
    AuthProviderType,
    FastMCPAuthConfig,
)

__all__ = [
    # 基础类型定义（保留以供内部使用）
    'AuthProviderType',
    'FastMCPAuthConfig',
]

__version__ = "0.1.0"
__description__ = "MCPStore Authentication Configuration Module - FastMCP Auth Wrapper"
