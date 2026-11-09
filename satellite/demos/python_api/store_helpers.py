from __future__ import annotations

from typing import Any, Callable

from errors import ApiError, _raise_core_error, _success
from schemas import ApiEnvelope
from validation import (
    _required_text,
    _scope_ref,
    _timeout,
    _tool_args,
    _tool_filter,
)


def service_info(store: Any, service_name: str, scope: dict[str, str]) -> dict[str, Any]:
    try:
        context = (
            store.for_store()
            if scope["type"] == "store"
            else store.for_agent(scope["agent_id"])
        )
        return context.find_service(service_name=service_name).info()
    except Exception as exc:
        _raise_core_error(exc, service_name=service_name)


def instance_id(store: Any, service_name: str, scope: dict[str, str]) -> str:
    info = service_info(store, service_name, scope)
    value = info.get("instance_id")
    if not isinstance(value, str) or not value:
        raise ApiError(500, "INTERNAL_ERROR", "服务信息缺少 instance_id")
    return value


def lifecycle(
    store: Any,
    service_name: str,
    scope: str | None,
    agent_id: str | None,
    method: Callable[[str], Any],
    message: str,
) -> ApiEnvelope:
    resolved = _scope_ref(scope, agent_id)
    instance = instance_id(store, service_name, resolved)
    try:
        method(instance)
    except Exception as exc:
        _raise_core_error(exc, operation=True, service_name=service_name)
    return _success(message, {"status": "ok"})


def list_tools(
    store: Any,
    service_name: str,
    scope: str | None,
    agent_id: str | None,
    filter_value: str | None,
) -> tuple[str, list[dict[str, Any]]]:
    resolved = _scope_ref(scope, agent_id)
    instance = instance_id(store, service_name, resolved)
    selected = _tool_filter(filter_value)
    try:
        tools = store.list_tool_entries(instance, filter=selected)
    except Exception as exc:
        _raise_core_error(exc, operation=True, service_name=service_name)
    return selected, tools


def call_tool(
    store: Any,
    service_name: str,
    scope: str | None,
    agent_id: str | None,
    payload: dict[str, Any],
) -> ApiEnvelope:
    resolved = _scope_ref(scope, agent_id)
    instance = instance_id(store, service_name, resolved)
    tool_name = _required_text(payload, "tool_name", "tool")
    args = _tool_args(payload)
    try:
        result = store.call_tool(instance, tool_name, args)
    except Exception as exc:
        _raise_core_error(exc, operation=True, service_name=service_name)
    return _success("工具调用完成", result)


def wait_ready(
    store: Any,
    service_name: str,
    scope: str | None,
    agent_id: str | None,
    timeout: str | None,
) -> ApiEnvelope:
    resolved = _scope_ref(scope, agent_id)
    instance = instance_id(store, service_name, resolved)
    try:
        status = store.wait_instance_ready(instance, _timeout(timeout))
    except Exception as exc:
        _raise_core_error(exc, operation=True, service_name=service_name)
    return _success("服务等待完成", status)
