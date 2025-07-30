from datetime import datetime
from enum import Enum
from typing import Optional, List, Dict, Any, Literal, Union

from pydantic import BaseModel, Field


class TransportType(str, Enum):
    STREAMABLE_HTTP = "streamable_http"
    STDIO = "stdio"
    STDIO_PYTHON = "stdio_python"
    STDIO_NODE = "stdio_node"
    STDIO_SHELL = "stdio_shell"


class ServiceConnectionState(str, Enum):
    """服务连接生命周期状态枚举"""
    INITIALIZING = "initializing"     # 初始化中：配置验证通过，正在进行首次连接
    HEALTHY = "healthy"               # 健康：连接正常，心跳成功
    WARNING = "warning"               # 警告：偶尔心跳失败，但未达重连阈值
    RECONNECTING = "reconnecting"     # 重连中：连续失败达阈值，正在重连
    UNREACHABLE = "unreachable"       # 无法访问：重连失败，进入长周期重试
    DISCONNECTING = "disconnecting"   # 断连中：正在执行优雅关闭
    DISCONNECTED = "disconnected"     # 已断连：服务已终止，等待手动删除

class ServiceStateMetadata(BaseModel):
    """服务状态元数据"""
    consecutive_failures: int = 0
    consecutive_successes: int = 0
    last_ping_time: Optional[datetime] = None
    last_success_time: Optional[datetime] = None
    last_failure_time: Optional[datetime] = None
    response_time: Optional[float] = None
    error_message: Optional[str] = None
    reconnect_attempts: int = 0
    next_retry_time: Optional[datetime] = None
    state_entered_time: Optional[datetime] = None
    disconnect_reason: Optional[str] = None


class ServiceInfo(BaseModel):
    url: str = ""
    name: str
    transport_type: TransportType
    status: ServiceConnectionState  # 使用新的7状态枚举
    tool_count: int
    keep_alive: bool
    working_dir: Optional[str] = None
    env: Optional[Dict[str, str]] = None
    last_heartbeat: Optional[datetime] = None
    command: Optional[str] = None
    args: Optional[List[str]] = None
    package_name: Optional[str] = None
    # 新增生命周期相关字段
    state_metadata: Optional[ServiceStateMetadata] = None
    last_state_change: Optional[datetime] = None
    client_id: Optional[str] = None  # 添加client_id字段

class ServiceInfoResponse(BaseModel):
    """单个服务的详细信息响应模型"""
    service: Optional[ServiceInfo] = Field(None, description="服务信息")
    tools: List[Dict[str, Any]] = Field(..., description="服务提供的工具列表")
    connected: bool = Field(..., description="服务连接状态")
    success: bool = Field(True, description="操作是否成功")
    message: Optional[str] = Field(None, description="响应消息")

class ServicesResponse(BaseModel):
    """服务列表响应模型"""
    services: List[ServiceInfo] = Field(..., description="服务列表")
    total_services: int = Field(..., description="服务总数")
    total_tools: int = Field(..., description="工具总数")
    success: bool = Field(True, description="操作是否成功")
    message: Optional[str] = Field(None, description="响应消息")

class RegisterRequestUnion(BaseModel):
    url: Optional[str] = None
    name: Optional[str] = None
    transport: Optional[str] = None
    keep_alive: Optional[bool] = None
    working_dir: Optional[str] = None
    env: Optional[Dict[str, str]] = None
    command: Optional[str] = None
    args: Optional[List[str]] = None
    package_name: Optional[str] = None

class JsonUpdateRequest(BaseModel):
    client_id: Optional[str] = None
    service_names: Optional[List[str]] = None
    config: Dict[str, Any]

# 这些响应模型已移动到 common.py 中，请直接从 common.py 导入

class ServiceConfig(BaseModel):
    """服务配置基类"""
    name: str = Field(..., description="服务名称")

class URLServiceConfig(ServiceConfig):
    """URL方式的服务配置"""
    url: str = Field(..., description="服务URL")
    transport: Optional[str] = Field("streamable-http", description="传输类型: streamable-http 或 sse")
    headers: Optional[Dict[str, str]] = Field(default=None, description="请求头")

class CommandServiceConfig(ServiceConfig):
    """本地命令方式的服务配置"""
    command: str = Field(..., description="执行命令")
    args: Optional[List[str]] = Field(default=None, description="命令参数")
    env: Optional[Dict[str, str]] = Field(default=None, description="环境变量")
    working_dir: Optional[str] = Field(default=None, description="工作目录")

class MCPServerConfig(BaseModel):
    """完整的MCP服务配置"""
    mcpServers: Dict[str, Dict[str, Any]] = Field(..., description="MCP服务配置字典")

# 支持多种配置格式
ServiceConfigUnion = Union[URLServiceConfig, CommandServiceConfig, MCPServerConfig, Dict[str, Any]]

class AddServiceRequest(BaseModel):
    """添加服务请求"""
    config: ServiceConfigUnion = Field(..., description="服务配置，支持多种格式")
    update_config: bool = Field(default=True, description="是否更新配置文件") 
