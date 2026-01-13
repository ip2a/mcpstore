"""
Service-related domain event definitions

All events are immutable (frozen=True) to ensure event integrity.
"""

import uuid
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from typing import Dict, Any, Optional, List, Tuple


class EventPriority(Enum):
    """Event priority"""
    LOW = 1
    NORMAL = 2
    HIGH = 3
    CRITICAL = 4


@dataclass(frozen=True)
class DomainEvent:
    """
    Domain event base class

    Note: Required parameters of all subclasses must be defined before base class default parameters
    """
    event_id: str = field(default_factory=lambda: str(uuid.uuid4()))
    timestamp: datetime = field(default_factory=datetime.now)
    priority: EventPriority = field(default=EventPriority.NORMAL)

    def __post_init__(self):
        """事件创建后的验证"""
        pass


@dataclass(frozen=True)
class ServiceAddRequested(DomainEvent):
    """服务添加请求事件"""
    agent_id: str = ""
    service_name: str = ""
    service_config: Dict[str, Any] = field(default_factory=dict)
    client_id: str = ""
    global_name: str = ""             # 可选：全局服务名（Agent 服务携带）
    origin_agent_id: Optional[str] = None   # 可选：原始 Agent（若 agent_id 被改写）
    origin_local_name: Optional[str] = None # 可选：原始本地名（若 service_name 被改写）
    source: str = "user"  # user, system
    wait_timeout: float = 0.0

    def __post_init__(self):
        super().__post_init__()
        if not self.service_name:
            raise ValueError("service_name cannot be empty")
        if not self.service_config:
            raise ValueError("service_config cannot be empty")


@dataclass(frozen=True)
class ServiceBootstrapRequested(DomainEvent):
    """
    服务启动重放请求事件（用于 setup/bootstrap 场景，非用户主动添加）
    """
    agent_id: str = ""
    service_name: str = ""
    service_config: Dict[str, Any] = field(default_factory=dict)
    client_id: str = ""
    global_name: str = ""
    origin_agent_id: Optional[str] = None
    origin_local_name: Optional[str] = None
    source: str = "bootstrap"  # bootstrap_mcpjson / sync_mcpjson 等

    def __post_init__(self):
        super().__post_init__()
        if not self.service_name:
            raise ValueError("service_name cannot be empty")
        if not self.service_config:
            raise ValueError("service_config cannot be empty")


@dataclass(frozen=True)
class ServiceBootstrapped(DomainEvent):
    """服务已完成启动重放的缓存构建"""
    agent_id: str = ""
    service_name: str = ""
    client_id: str = ""
    global_name: str = ""
    source: str = "bootstrap"
    service_config: Dict[str, Any] = field(default_factory=dict)


@dataclass(frozen=True)
class ServiceBootstrapFailed(DomainEvent):
    """服务启动重放失败事件"""
    agent_id: str = ""
    service_name: str = ""
    error_message: str = ""
    source: str = "bootstrap"
    original_event: Optional[DomainEvent] = None


@dataclass(frozen=True)
class ServiceCached(DomainEvent):
    """服务已缓存事件"""
    agent_id: str = ""
    service_name: str = ""
    client_id: str = ""
    cache_keys: List[str] = field(default_factory=list)  # 记录缓存的键，用于回滚


@dataclass(frozen=True)
class ServiceInitialized(DomainEvent):
    """服务生命周期已初始化事件"""
    agent_id: str = ""
    service_name: str = ""
    initial_state: str = "STARTUP"  # "startup"


@dataclass(frozen=True)
class ServiceConnectionRequested(DomainEvent):
    """服务连接请求事件"""
    agent_id: str = ""
    service_name: str = ""
    service_config: Dict[str, Any] = field(default_factory=dict)
    timeout: float = 3.0


@dataclass(frozen=True)
class ServiceConnected(DomainEvent):
    """服务连接成功事件"""
    agent_id: str = ""
    service_name: str = ""
    session: Any = None  # MCP Client session
    tools: List[Tuple[str, Dict[str, Any]]] = field(default_factory=list)
    connection_time: float = 0.0


@dataclass(frozen=True)
class ServicePersisting(DomainEvent):
    """
    服务持久化开始（缓存/关系/状态写入）
    """
    agent_id: str = ""
    service_name: str = ""
    stage: str = "cache"  # cache | config | other
    tool_count: int = 0
    source_event: Optional[DomainEvent] = None


@dataclass(frozen=True)
class ServiceConnectionFailed(DomainEvent):
    """服务连接失败事件"""
    agent_id: str = ""
    service_name: str = ""
    error_message: str = ""
    error_type: str = ""  # timeout, network, auth, etc.
    retry_count: int = 0


@dataclass(frozen=True)
class ServiceStateChanged(DomainEvent):
    """服务状态变化事件"""
    agent_id: str = ""
    service_name: str = ""
    old_state: str = ""
    new_state: str = ""
    reason: str = ""
    source: str = ""  # 触发状态变化的来源


@dataclass(frozen=True)
class ServicePersisted(DomainEvent):
    """
    服务已持久化事件
    - stage 默认 config；缓存落盘完成时可使用 stage="cache" 并填写工具数量等信息
    """
    agent_id: str = ""
    service_name: str = ""
    file_path: str = ""
    stage: str = "config"  # config | cache | other
    tool_count: int = 0
    details: Dict[str, Any] = field(default_factory=dict)


@dataclass(frozen=True)
class ServiceReady(DomainEvent):
    """
    服务已就绪事件：工具状态与健康元数据落盘完成，可对外读取
    """
    agent_id: str = ""
    service_name: str = ""
    tool_count: int = 0
    health_status: str = ""


@dataclass(frozen=True)
class ServiceOperationFailed(DomainEvent):
    """服务操作失败事件（用于错误处理）"""
    agent_id: str = ""
    service_name: str = ""
    operation: str = ""  # cache, connect, persist, etc.
    error_message: str = ""
    original_event: Optional[DomainEvent] = None


# === 健康检查相关事件 ===

@dataclass(frozen=True)
class HealthCheckRequested(DomainEvent):
    """健康检查请求事件"""
    agent_id: str = ""
    service_name: str = ""
    check_type: str = "periodic"  # periodic, manual, triggered


@dataclass(frozen=True)
class HealthCheckCompleted(DomainEvent):
    """健康检查完成事件"""
    agent_id: str = ""
    service_name: str = ""
    success: bool = False
    response_time: float = 0.0
    error_message: Optional[str] = None
    suggested_state: Optional[str] = None  # HEALTHY, DEGRADED, CIRCUIT_OPEN, DISCONNECTED
    # 窗口与退避元数据
    window_error_rate: Optional[float] = None
    latency_p95: Optional[float] = None
    latency_p99: Optional[float] = None
    sample_size: Optional[int] = None
    retry_in: Optional[float] = None           # 秒
    hard_timeout_in: Optional[float] = None    # 秒
    lease_remaining: Optional[float] = None    # 秒
    next_retry_time: Optional[float] = None    # 时间戳（秒）
    hard_deadline: Optional[float] = None      # 时间戳（秒）
    lease_deadline: Optional[float] = None     # 时间戳（秒）


@dataclass(frozen=True)
class ServiceTimeout(DomainEvent):
    """服务超时事件"""
    agent_id: str = ""
    service_name: str = ""
    timeout_type: str = ""  # initialization, health_check, disconnection
    elapsed_time: float = 0.0


# === 重连相关事件 ===

@dataclass(frozen=True)
class ReconnectionRequested(DomainEvent):
    """重连请求事件"""
    agent_id: str = ""
    service_name: str = ""
    retry_count: int = 0
    reason: str = "scheduled_retry"


@dataclass(frozen=True)
class ReconnectionScheduled(DomainEvent):
    """重连已调度事件"""
    agent_id: str = ""
    service_name: str = ""
    next_retry_time: float = 0.0  # timestamp
    retry_delay: float = 0.0  # seconds


# === 工具同步相关事件 ===

@dataclass(frozen=True)
class ToolSyncStarted(DomainEvent):
    """工具同步开始事件"""
    agent_id: str = ""
    service_name: str = ""
    total_tools: int = 0
    source: str = "cache_manager"


@dataclass(frozen=True)
class ToolSyncCompleted(DomainEvent):
    """工具同步完成事件"""
    agent_id: str = ""
    service_name: str = ""
    total_tools: int = 0
    source: str = "cache_manager"
