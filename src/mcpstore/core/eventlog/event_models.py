"""
事件模型与序列化辅助工具。

提供 EventRecord 数据结构以及 DomainEvent 与记录之间的转换。
"""

from __future__ import annotations

import dataclasses
import time
from dataclasses import dataclass
from datetime import datetime
from typing import Any, Dict, Optional, Type

from mcpstore.core.events.service_events import DomainEvent, EventPriority


def _serialize_timestamp(value: Any) -> Any:
    if isinstance(value, datetime):
        return value.isoformat()
    return value


def _deserialize_timestamp(value: Any) -> Any:
    if isinstance(value, str):
        try:
            return datetime.fromisoformat(value)
        except Exception:
            return value
    return value


def _serialize_priority(value: Any) -> Any:
    if isinstance(value, EventPriority):
        return value.name
    return value


def _deserialize_priority(value: Any) -> Any:
    if isinstance(value, str):
        try:
            return EventPriority[value]
        except KeyError:
            return EventPriority.NORMAL
    return value


@dataclass
class EventRecord:
    """持久化事件记录。"""

    id: str
    type: str
    payload: Dict[str, Any]
    source: str
    created_at: float
    dedup_key: Optional[str] = None
    trace_id: Optional[str] = None

    def to_dict(self) -> Dict[str, Any]:
        return {
            "id": self.id,
            "type": self.type,
            "payload": self.payload,
            "source": self.source,
            "created_at": self.created_at,
            "dedup_key": self.dedup_key,
            "trace_id": self.trace_id,
        }

    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "EventRecord":
        return cls(
            id=str(data.get("id", "")),
            type=str(data.get("type", "")),
            payload=data.get("payload") or {},
            source=str(data.get("source", "")),
            created_at=float(data.get("created_at", time.time())),
            dedup_key=data.get("dedup_key"),
            trace_id=data.get("trace_id"),
        )


def domain_event_to_record(
    event: DomainEvent,
    *,
    source: str,
    dedup_key: Optional[str] = None,
    trace_id: Optional[str] = None,
    record_id: Optional[str] = None,
) -> EventRecord:
    """
    将 DomainEvent 转换为 EventRecord，便于写入事件队列。
    """
    payload = dataclasses.asdict(event)
    # 序列化非基本类型
    payload["timestamp"] = _serialize_timestamp(payload.get("timestamp"))
    payload["priority"] = _serialize_priority(payload.get("priority"))

    return EventRecord(
        id=record_id or "",
        type=event.__class__.__name__,
        payload=payload,
        source=source,
        created_at=time.time(),
        dedup_key=dedup_key,
        trace_id=trace_id,
    )


def record_to_domain_event(
    record: EventRecord,
    event_mapping: Dict[str, Type[DomainEvent]],
) -> Optional[DomainEvent]:
    """
    将 EventRecord 转回具体的 DomainEvent。
    """
    cls = event_mapping.get(record.type)
    if cls is None:
        return None

    payload = dict(record.payload)
    if "timestamp" in payload:
        payload["timestamp"] = _deserialize_timestamp(payload["timestamp"])
    if "priority" in payload:
        payload["priority"] = _deserialize_priority(payload["priority"])
    return cls(**payload)

