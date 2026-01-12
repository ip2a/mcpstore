"""
Hub MCP Types Module
Hub MCP 类型定义模块 - 定义 Hub MCP 相关的数据类型和枚举
"""

import socket
from dataclasses import dataclass, field
from enum import Enum
from typing import Optional, Literal, Any, Dict


class HubMCPStatus(Enum):
    """
    Hub MCP 服务器状态枚举
    
    定义 Hub MCP 服务器的所有可能状态。
    """
    
    STARTUP = "startup"  # 初始化中
    RUNNING = "running"            # 运行中
    STOPPING = "stopping"          # 停止中
    STOPPED = "stopped"            # 已停止
    ERROR = "error"                # 错误状态


@dataclass
class HubMCPConfig:
    """
    Hub MCP 配置数据类
    
    定义 Hub MCP 服务器的配置参数。
    
    Attributes:
        transport: 传输协议，可选 "http"、"sse"、"stdio"
        port: 端口号（仅 http/sse），None 为自动分配
        host: 监听地址（仅 http/sse），默认 "0.0.0.0"
        path: 端点路径（仅 http），默认 "/mcp"
        fastmcp_kwargs: 传递给 FastMCP 的其他参数
    """
    
    transport: Literal["http", "sse", "stdio"] = "http"
    port: Optional[int] = None
    host: str = "0.0.0.0"
    path: str = "/mcp"
    fastmcp_kwargs: Dict[str, Any] = field(default_factory=dict)
    
    def __post_init__(self):
        """
        初始化后处理
        
        自动分配端口（如果需要）。
        """
        # 自动分配端口
        if self.port is None and self.transport in ["http", "sse"]:
            self.port = self._find_available_port()
    
    def _find_available_port(self) -> int:
        """
        查找可用端口
        
        Returns:
            int: 可用的端口号
        """
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
            s.bind(('', 0))
            s.listen(1)
            port = s.getsockname()[1]
        return port
