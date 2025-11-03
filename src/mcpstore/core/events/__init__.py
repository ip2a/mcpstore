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
    ServiceCached,
    ServiceInitialized,
    ServiceConnectionRequested,
    ServiceConnected,
    ServiceConnectionFailed,
    ServiceStateChanged,
    ServicePersisted,
    ServiceOperationFailed,
    HealthCheckRequested,
    HealthCheckCompleted,
    ServiceTimeout,
    ReconnectionRequested,
    ReconnectionScheduled,
)

__all__ = [
    # 基础类
    "DomainEvent",
    "EventPriority",
    "EventBus",
    "EventSubscription",
    
    # 服务事件
    "ServiceAddRequested",
    "ServiceCached",
    "ServiceInitialized",
    "ServiceConnectionRequested",
    "ServiceConnected",
    "ServiceConnectionFailed",
    "ServiceStateChanged",
    "ServicePersisted",
    "ServiceOperationFailed",
    # 健康与重连事件
    "HealthCheckRequested",
    "HealthCheckCompleted",
    "ServiceTimeout",
    "ReconnectionRequested",
    "ReconnectionScheduled",
]

