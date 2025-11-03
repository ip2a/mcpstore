"""
服务相关的领域事件定义

所有事件都是不可变的（frozen=True），确保事件的完整性。
"""

import uuid
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from typing import Dict, Any, Optional, List, Tuple


class EventPriority(Enum):
    """事件优先级"""
    LOW = 1
    NORMAL = 2
    HIGH = 3
    CRITICAL = 4


@dataclass(frozen=True)
class DomainEvent:
    """
    领域事件基类

    注意：所有子类的必需参数必须在基类的默认参数之前定义
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
    source: str = "user"  # user, system
    wait_timeout: float = 0.0

    def __post_init__(self):
        super().__post_init__()
        if not self.service_name:
            raise ValueError("service_name cannot be empty")
        if not self.service_config:
            raise ValueError("service_config cannot be empty")


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
    initial_state: str = "INITIALIZING"  # "initializing"


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
    """服务已持久化事件"""
    agent_id: str = ""
    service_name: str = ""
    file_path: str = ""


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
    suggested_state: Optional[str] = None  # HEALTHY, WARNING, RECONNECTING, UNREACHABLE


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

