"""Instance runtime and MCP capability operations."""

from __future__ import annotations

from typing import Any, Dict, List, Optional

from mcpstore.core.models import ScopeRef
from mcpstore.native.records import _record_value, _scope_payload

def list_instances(backend) -> List[Dict[str, Any]]:
    return _record_value(backend._inner.list_instances())

async def list_instances_async(backend) -> List[Dict[str, Any]]:
    return backend.list_instances()

def list_instances_scoped(
    backend,
    scope: ScopeRef | Dict[str, Any],
) -> List[Dict[str, Any]]:
    return _record_value(backend._inner.list_instances_scoped(_scope_payload(scope)))

def find_instance(backend, instance_id: str) -> Optional[Dict[str, Any]]:
    return _record_value(backend._inner.find_instance(instance_id))

def instance_info(backend, instance_id: str) -> Dict[str, Any]:
    return _record_value(backend._inner.instance_info(instance_id))

def connect_service(backend, instance_id: str) -> None:
    backend._inner.connect_service(instance_id)

async def connect_service_async(backend, instance_id: str) -> None:
    backend.connect_service(instance_id)

def disconnect_service(backend, instance_id: str) -> None:
    backend._inner.disconnect_service(instance_id)

async def disconnect_service_async(backend, instance_id: str) -> None:
    backend.disconnect_service(instance_id)

def restart_service(backend, instance_id: str) -> None:
    backend._inner.restart_service(instance_id)

async def restart_service_async(backend, instance_id: str) -> None:
    backend.restart_service(instance_id)

def wait_instance_ready(backend, instance_id: str, timeout_secs: int = 10) -> Dict[str, Any]:
    return _record_value(backend._inner.wait_instance_ready(instance_id, timeout_secs))

async def wait_instance_ready_async(
    backend,
    instance_id: str,
    timeout_secs: int = 10,
) -> Dict[str, Any]:
    return backend.wait_instance_ready(instance_id, timeout_secs)

def check_instances(backend, instance_ids: List[str]) -> Dict[str, Any]:
    return _record_value(backend._inner.check_instances(instance_ids))

def service_state(backend, instance_id: str) -> Dict[str, Any]:
    return _record_value(backend._inner.service_state(instance_id))

def list_tools(backend, instance_id: str) -> List[Dict[str, Any]]:
    tools = _record_value(backend._inner.list_tools(instance_id))
    for tool in tools:
        tool["instance_id"] = instance_id
    return tools

async def list_tools_async(backend, instance_id: str) -> List[Dict[str, Any]]:
    return backend.list_tools(instance_id)

def list_tool_entries(
    backend,
    instance_id: str,
    *,
    filter: str = "all",
) -> List[Dict[str, Any]]:
    return _record_value(backend._inner.list_tool_entries(instance_id, filter=filter))

def list_changed_tools(
    backend,
    instance_id: str,
    *,
    force_refresh: bool = False,
) -> Dict[str, Any]:
    return _record_value(
        backend._inner.list_changed_tools(
            instance_id,
            force_refresh=force_refresh,
        )
    )

def call_tool(
    backend,
    instance_id: str,
    tool_name: str,
    args: Optional[Dict[str, Any]] = None,
) -> Dict[str, Any]:
    return _record_value(backend._inner.call_tool(instance_id, tool_name, args or {}))

async def call_tool_async(
    backend,
    instance_id: str,
    tool_name: str,
    args: Optional[Dict[str, Any]] = None,
) -> Dict[str, Any]:
    return backend.call_tool(instance_id, tool_name, args)

def list_resources(backend, instance_id: str) -> List[Dict[str, Any]]:
    return _record_value(backend._inner.list_resources(instance_id))

async def list_resources_async(backend, instance_id: str) -> List[Dict[str, Any]]:
    return backend.list_resources(instance_id)

def list_resource_templates(backend, instance_id: str) -> List[Dict[str, Any]]:
    return _record_value(backend._inner.list_resource_templates(instance_id))

def read_resource(backend, instance_id: str, uri: str) -> Dict[str, Any]:
    return _record_value(backend._inner.read_resource(instance_id, uri))

async def read_resource_async(backend, instance_id: str, uri: str) -> Dict[str, Any]:
    return backend.read_resource(instance_id, uri)

def list_prompts(backend, instance_id: str) -> List[Dict[str, Any]]:
    return _record_value(backend._inner.list_prompts(instance_id))

async def list_prompts_async(backend, instance_id: str) -> List[Dict[str, Any]]:
    return backend.list_prompts(instance_id)

def get_prompt(
    backend,
    instance_id: str,
    prompt_name: str,
    arguments: Optional[Dict[str, Any]] = None,
) -> Dict[str, Any]:
    return _record_value(
        backend._inner.get_prompt(instance_id, prompt_name, arguments or {})
    )

def export_instance_config(
    backend,
    instance_id: str,
    format: Optional[str] = None,
) -> Dict[str, Any]:
    return _record_value(backend._inner.export_instance_config(instance_id, format))
