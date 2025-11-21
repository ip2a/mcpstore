"""
MCPStore Hub Module
Hub模块 - 分布式服务打包和管理功能

实现方案1：Hub 服务功能（基础分布式架构）
- 服务打包：将现有缓存的服务打包为独立Hub服务
- 分布式架构：每个Hub运行在独立的进程中
- 基础路由：支持 /mcp 全局访问
- 进程管理：Hub进程的启动、停止、监控

设计原则：
- 基于现有服务缓存，不重复实现服务注册
- 使用FastMCP作为MCP服务器实现
- 完全独立的进程隔离
- 与现有MCPStore架构无缝集成
"""

from .builder import HubServicesBuilder, HubToolsBuilder
from .package import HubPackage
from .process import HubProcess
from .server import HubServerGenerator
from .types import HubConfig, HubStatus

__all__ = [
    # Core classes
    'HubServicesBuilder',
    'HubToolsBuilder', 
    'HubPackage',
    'HubProcess',
    'HubServerGenerator',
    
    # Types
    'HubConfig',
    'HubStatus'
]

__version__ = "1.0.0"
__description__ = "MCPStore Hub Module - Distributed service packaging and management"
