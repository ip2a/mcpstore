"""
Hub Types Module
Hub类型定义模块 - 定义Hub功能相关的数据类型和枚举
"""

from dataclasses import dataclass
from datetime import datetime
from enum import Enum
from typing import Dict, List, Any, Optional


class HubStatus(Enum):
    """Hub状态枚举"""
    INITIALIZING = "initializing"  # 初始化中
    RUNNING = "running"           # 运行中
    STOPPING = "stopping"         # 停止中
    STOPPED = "stopped"           # 已停止
    ERROR = "error"               # 错误状态


@dataclass
class HubConfig:
    """Hub配置数据类"""
    name: str                              # Hub名称
    description: Optional[str] = None      # Hub描述
    context_type: str = "store"            # 上下文类型: "store" 或 "agent"
    target_id: Optional[str] = None        # 目标ID（仅当context_type="agent"时）
    basic_routing: bool = True             # 启用基础路由
    port: Optional[int] = None             # 端口号（None为自动分配）
    filters: Dict[str, Any] = None         # 服务过滤器
    
    # 认证相关配置
    auth_enabled: bool = False             # 是否启用认证
    auth_provider_type: Optional[str] = None  # 认证提供者类型
    fastmcp_auth: Optional[Dict[str, Any]] = None  # FastMCP认证配置
    required_scopes: List[str] = None      # 必需的权限范围
    protected_tools: List[str] = None      # 受保护的工具
    public_tools: List[str] = None         # 公开的工具
    
    def __post_init__(self):
        if self.filters is None:
            self.filters = {}
        if self.required_scopes is None:
            self.required_scopes = []
        if self.protected_tools is None:
            self.protected_tools = []
        if self.public_tools is None:
            self.public_tools = []


@dataclass 
class HubProcessInfo:
    """Hub进程信息"""
    package_name: str                      # 包名
    port: int                             # 端口号
    pid: Optional[int] = None             # 进程ID
    is_running: bool = False              # 是否运行中
    start_time: Optional[datetime] = None  # 启动时间
    uptime: float = 0.0                   # 运行时长（秒）
    endpoint_url: str = ""                # 端点URL
    config_file: str = ""                 # 配置文件路径
    script_file: str = ""                 # 脚本文件路径
    
    def __post_init__(self):
        if not self.endpoint_url and self.port:
            self.endpoint_url = f"http://localhost:{self.port}/mcp"


@dataclass
class HubServiceInfo:
    """Hub中的服务信息（基于现有ServiceInfo转换）"""
    name: str                             # 服务名称
    url: Optional[str] = None             # 服务URL
    command: Optional[str] = None         # 命令
    args: Optional[List[str]] = None      # 参数
    transport_type: str = "unknown"       # 传输类型
    status: str = "unknown"               # 状态
    tools: List[Dict[str, Any]] = None    # 工具列表
    
    def __post_init__(self):
        if self.tools is None:
            self.tools = []
        if self.args is None:
            self.args = []


@dataclass
class HubRouteInfo:
    """Hub路由信息"""
    route_type: str                       # 路由类型: "basic", "service"
    path: str                            # 路由路径
    description: str                     # 描述
    service_name: Optional[str] = None   # 关联的服务名（仅service路由）


class HubStartMode(Enum):
    """Hub启动模式"""
    SUBPROCESS = "subprocess"  # 子进程模式（默认）
    THREAD = "thread"         # 线程模式（未实现）
    ASYNC = "async"           # 异步模式（未实现）
