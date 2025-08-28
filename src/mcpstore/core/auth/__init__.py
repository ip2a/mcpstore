"""
MCPStore FastMCP Authentication Module
FastMCP认证配置封装模块 - 完全基于FastMCP的认证功能
"""

from .builder import AuthServiceBuilder, AuthProviderBuilder, AuthTokenBuilder
from .manager import AuthConfigManager, get_auth_config_manager
from .types import (
    AuthProviderConfig, 
    AuthProviderType,
    FastMCPAuthConfig,
    HubAuthConfig,
    JWTPayloadConfig
)

__all__ = [
    # 构建器
    'AuthServiceBuilder', 
    'AuthProviderBuilder',
    'AuthTokenBuilder',
    
    # 管理器
    'AuthConfigManager',
    'get_auth_config_manager',
    
    # 类型定义
    'AuthProviderConfig',
    'AuthProviderType',
    'FastMCPAuthConfig',
    'HubAuthConfig',
    'JWTPayloadConfig'
]

__version__ = "0.1.0"
__description__ = "MCPStore Authentication Configuration Module - FastMCP Auth Wrapper"
