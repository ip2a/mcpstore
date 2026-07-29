"""Python session interface over Rust session records."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, Dict, List, Optional

from mcpstore.native.records import _record_value, _plain_record_value

if TYPE_CHECKING:
    from mcpstore.store.store import RustStoreBackend

class SessionContext:
    """Session facade whose service relationships use instance IDs."""

    def __init__(self, backend: RustStoreBackend, entity: Dict[str, Any]):
        self._backend = backend
        self._entity = entity

    @property
    def session_key(self) -> str:
        return str(self._entity["session_key"])

    @property
    def session_id(self) -> str:
        return str(self._entity["session_id"])

    def to_dict(self) -> Dict[str, Any]:
        return _plain_record_value(self._entity)

    def show_config(self) -> Dict[str, Any]:
        return _record_value(self._backend._inner.show_session_config(self.session_key))

    def refresh(self) -> "SessionContext":
        entity = self._backend._inner.get_session(self.session_key)
        if entity is None:
            raise KeyError(f"Session not found: {self.session_key}")
        self._entity = _record_value(entity)
        return self

    def bind_service(self, instance_id: str) -> "SessionContext":
        self._backend._inner.bind_service_to_session(self.session_key, instance_id)
        return self

    def unbind_service(self, instance_id: str) -> "SessionContext":
        self._backend._inner.unbind_service_from_session(self.session_key, instance_id)
        return self

    def list_services(self) -> List[Dict[str, Any]]:
        return _record_value(self._backend._inner.list_session_services(self.session_key))

    def list_tools(self) -> List[Dict[str, Any]]:
        return _record_value(self._backend._inner.list_tools_in_session(self.session_key))

    async def list_tools_async(self) -> List[Dict[str, Any]]:
        return self.list_tools()

    def call_tool(
        self,
        instance_id: str,
        tool_name: str,
        args: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        return _record_value(
            self._backend._inner.call_tool_in_session(
                self.session_key,
                instance_id,
                tool_name,
                args or {},
            )
        )

    async def call_tool_async(
        self,
        instance_id: str,
        tool_name: str,
        args: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        return self.call_tool(instance_id, tool_name, args)

    def close(self, reason: Optional[str] = None) -> Dict[str, Any]:
        return _record_value(self._backend._inner.close_session(self.session_key, reason))

    def for_langchain(self, instance_id: str, response_format: str = "text") -> Any:
        from mcpstore.adapters.langchain_adapter import SessionAwareLangChainAdapter

        return SessionAwareLangChainAdapter(
            self._backend,
            self,
            instance_id,
            response_format=response_format,
        )


__all__ = ["SessionContext"]
