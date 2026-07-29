"""Scope-first Python context over the Rust StoreContextFacade."""

from __future__ import annotations

from typing import Any, Dict, List, Optional

from mcpstore.core.models import ScopeDescriptor, ScopeRef
from mcpstore.native.records import _record_value, _scope_payload
from mcpstore.service.proxy import RustServiceProxy
from mcpstore.tools.proxy import RustToolProxy
from mcpstore.cache.proxy import RustCacheProxy


class RustStoreContext:
    def __init__(self, backend: Any, scope: ScopeRef | Dict[str, Any]):
        self._backend = backend
        self.scope = _scope_payload(scope)
        native_factory = getattr(backend._inner, "for_store", None)
        if self.scope.get("type") == "agent":
            native_factory = getattr(backend._inner, "for_agent", None)
            self._native = native_factory(self.scope["agent_id"]) if native_factory else None
        else:
            self._native = native_factory() if native_factory else None

    def add_service_config(self, service_name: str, config: Dict[str, Any]) -> str:
        if self._native is not None:
            return str(self._native.add_service_config(service_name, config))
        self._backend.add_service(service_name, config)
        return self._backend.declare_service_scope(service_name, self.scope, {})

    def add_mcp_config(self, config: Dict[str, Any]) -> List[str]:
        if self._native is not None:
            return [str(value) for value in self._native.add_mcp_config(config)]
        ids = []
        for name, service_config in config.get("mcpServers", {}).items():
            ids.append(self.add_service_config(name, service_config))
        return ids

    def wait_service(self, service_name: str, timeout_secs: int = 10) -> Dict[str, Any]:
        if self._native is not None:
            return _record_value(self._native.wait_service(service_name, timeout_secs))
        instance = next(item for item in self.list_services() if item["service_name"] == service_name)
        return self._backend.wait_instance_ready(instance["instance_id"], timeout_secs)

    def list_services(self) -> List[Dict[str, Any]]:
        if self._native is not None:
            return _record_value(self._native.list_services())
        return self._backend.list_instances_scoped(self.scope)

    async def list_services_async(self) -> List[Dict[str, Any]]:
        return self.list_services()

    def show_config(self) -> Dict[str, Any]:
        return self._backend.show_scope_config(self.scope)

    def get_effective_config(self, service_name: str) -> Optional[Dict[str, Any]]:
        return self._backend.get_effective_config(service_name, self.scope)

    def declare_service_scope(self, service_name: str, scope: ScopeRef | Dict[str, Any], descriptor: ScopeDescriptor | Dict[str, Any]) -> str:
        return self._backend.declare_service_scope(service_name, scope, descriptor)

    def remove_service_scope(self, service_name: str, scope: ScopeRef | Dict[str, Any]) -> None:
        self._backend.remove_service_scope(service_name, scope)

    def list_instances(self) -> List[Dict[str, Any]]:
        return self._backend.list_instances()

    def find_service(self, instance_id: str) -> RustServiceProxy:
        if self._backend.find_instance(instance_id) is None:
            raise KeyError(f"Instance not found: {instance_id}")
        return RustServiceProxy(self._backend, instance_id)

    def find_tool(self, instance_id: str, tool_name: str) -> RustToolProxy:
        return RustToolProxy(self._backend, instance_id, tool_name)

    def connect_service(self, instance_id: str) -> None:
        self._backend.connect_service(instance_id)

    async def connect_service_async(self, instance_id: str) -> None:
        self.connect_service(instance_id)

    def disconnect_service(self, instance_id: str) -> None:
        self._backend.disconnect_service(instance_id)

    async def disconnect_service_async(self, instance_id: str) -> None:
        self.disconnect_service(instance_id)

    def restart_service(self, instance_id: str) -> None:
        self._backend.restart_service(instance_id)

    async def restart_service_async(self, instance_id: str) -> None:
        self.restart_service(instance_id)

    def list_tools(self, instance_id: Optional[str] = None) -> List[Dict[str, Any]]:
        if instance_id is None and self._native is not None:
            return _record_value(self._native.list_tools())
        if instance_id is None:
            return [tool for service in self.list_services() for tool in service.get("tools", [])]
        return self._backend.list_tools(instance_id)

    async def list_tools_async(self, instance_id: Optional[str] = None) -> List[Dict[str, Any]]:
        return self.list_tools(instance_id)

    def call_tool(self, instance_id: str, tool_name: str, args: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        return self._backend.call_tool(instance_id, tool_name, args)

    async def call_tool_async(self, instance_id: str, tool_name: str, args: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        return self.call_tool(instance_id, tool_name, args)

    def list_resources(self, instance_id: str) -> List[Dict[str, Any]]:
        return self._backend.list_resources(instance_id)

    def read_resource(self, instance_id: str, uri: str) -> Dict[str, Any]:
        return self._backend.read_resource(instance_id, uri)

    def list_prompts(self, instance_id: str) -> List[Dict[str, Any]]:
        return self._backend.list_prompts(instance_id)

    def get_prompt(self, instance_id: str, prompt_name: str, arguments: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        return self._backend.get_prompt(instance_id, prompt_name, arguments)

    def export_instance_config(self, instance_id: str, format: Optional[str] = None) -> Dict[str, Any]:
        return self._backend.export_instance_config(instance_id, format)

    def wait_instance_ready(self, instance_id: str, timeout_secs: int = 10) -> Dict[str, Any]:
        return self._backend.wait_instance_ready(instance_id, timeout_secs)

    def find_cache(self) -> RustCacheProxy:
        return self._backend.find_cache()

    def for_langchain(self, instance_id: str, response_format: str = "text") -> Any:
        return self._backend.for_langchain(instance_id, response_format)

    def for_langgraph(self, instance_id: str, response_format: str = "text") -> Any:
        return self._backend.for_langgraph(instance_id, response_format)

    def for_openai(self, instance_id: str) -> Any:
        return self._backend.for_openai(instance_id)

    def for_autogen(self, instance_id: str) -> Any:
        return self._backend.for_autogen(instance_id)

    def for_llamaindex(self, instance_id: str) -> Any:
        return self._backend.for_llamaindex(instance_id)

    def for_crewai(self, instance_id: str) -> Any:
        return self._backend.for_crewai(instance_id)

    def for_semantic_kernel(self, instance_id: str) -> Any:
        return self._backend.for_semantic_kernel(instance_id)


StoreContext = RustStoreContext
AgentContext = RustStoreContext

__all__ = ["AgentContext", "RustStoreContext", "StoreContext"]
