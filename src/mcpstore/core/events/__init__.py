"""
事件系统模块

提供事件驱动架构的核心组件：
- 领域事件定义
- 事件总线
"""

from .event_bus import EventBus, EventSubscription
from .service_events import (
    DomainEvent,
    EventPriority,
    ServiceAddRequested,
    ServiceBootstrapRequested,
    ServiceBootstrapped,
    ServiceBootstrapFailed,
    ServiceCached,
    ServiceInitialized,
    ServiceConnectionRequested,
    ServiceConnected,
    ServiceConnectionFailed,
    ServiceStateChanged,
    ServicePersisting,
    ServicePersisted,
    ServiceReady,
    ServiceOperationFailed,
    HealthCheckRequested,
    HealthCheckCompleted,
    ServiceTimeout,
    ReconnectionRequested,
    ReconnectionScheduled,
    ToolSyncStarted,
    ToolSyncCompleted,
)

__all__ = [
    # 基础类
    "DomainEvent",
    "EventPriority",
    "EventBus",
    "EventSubscription",
    
    # 服务事件
    "ServiceAddRequested",
    "ServiceBootstrapRequested",
    "ServiceBootstrapped",
    "ServiceBootstrapFailed",
    "ServiceCached",
    "ServiceInitialized",
    "ServiceConnectionRequested",
    "ServiceConnected",
    "ServiceConnectionFailed",
    "ServiceStateChanged",
    "ServicePersisting",
    "ServicePersisted",
    "ServiceReady",
    "ServiceOperationFailed",
    # 健康与重连事件
    "HealthCheckRequested",
    "HealthCheckCompleted",
    "ServiceTimeout",
    "ReconnectionRequested",
    "ReconnectionScheduled",
    # 工具同步事件
    "ToolSyncStarted",
    "ToolSyncCompleted",
]
