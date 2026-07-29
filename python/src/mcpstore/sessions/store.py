"""Store-level session operations."""

from __future__ import annotations

from typing import Any, Dict, List, Optional

from mcpstore.native.records import _record_value
from mcpstore.sessions.session import RustSession

def create_session(
    backend,
    session_id: str,
    *,
    scope: Optional[str] = None,
    agent_id: Optional[str] = None,
    lease_seconds: Optional[int] = None,
    metadata: Optional[Dict[str, Any]] = None,
) -> "RustSession":
    entity = backend._inner.create_session(
        session_id,
        scope,
        agent_id,
        lease_seconds,
        metadata,
    )
    return RustSession(self, _record_value(entity))

def get_session(backend, session_key: str) -> Optional["RustSession"]:
    entity = backend._inner.get_session(session_key)
    return RustSession(self, _record_value(entity)) if entity else None

def find_session(
    backend,
    session_id: str,
    *,
    scope: Optional[str] = None,
    agent_id: Optional[str] = None,
) -> Optional["RustSession"]:
    entity = backend._inner.find_session(session_id, scope, agent_id)
    return RustSession(self, _record_value(entity)) if entity else None

def list_sessions(
    backend,
    *,
    scope: Optional[str] = None,
    agent_id: Optional[str] = None,
) -> List["RustSession"]:
    return [
        RustSession(self, _record_value(entity))
        for entity in backend._inner.list_sessions(scope, agent_id)
    ]

def export_sessions_snapshot(backend) -> Dict[str, Any]:
    return _record_value(backend._inner.export_sessions_snapshot())

def import_sessions_snapshot(backend, snapshot: Dict[str, Any]) -> Dict[str, Any]:
    return _record_value(backend._inner.import_sessions_snapshot(snapshot))
