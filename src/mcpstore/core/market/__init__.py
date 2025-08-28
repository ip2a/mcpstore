"""
MCPStore Market Module
市场功能模块 - 提供从在线市场安装MCP服务的能力
"""

from .manager import MarketManager
from .service import MarketService
from .converter import MarketConfigConverter
from .types import MarketServerInfo, MarketInstallation

__all__ = [
    'MarketManager',
    'MarketService', 
    'MarketConfigConverter',
    'MarketServerInfo',
    'MarketInstallation'
]

__version__ = "0.1.0"
__description__ = "MCPStore Market Module - Install MCP services from online marketplace"
