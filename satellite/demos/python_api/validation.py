from __future__ import annotations

from typing import Any

from errors import _invalid_parameter, _invalid_request, _missing


def _require_object(payload: Any, label: str) -> dict[str, Any]:
    if not isinstance(payload, dict):
        raise _invalid_request(f"{label}必须是 JSON 对象")
    return payload


def _scope_ref(scope: str | None, agent_id: str | None) -> dict[str, str]:
    resolved = scope or "store"
    if resolved == "store":
        return {"type": "store"}
    if resolved == "agent":
        agent = (agent_id or "").strip()
        if not agent:
            raise _missing("agent_id")
        return {"type": "agent", "agent_id": agent}
    raise _invalid_parameter(f"不支持的 scope: {resolved}", "scope")


def _scope_view(scope: str | None, agent_id: str | None) -> dict[str, str]:
    if scope == "root":
        return {"type": "root"}
    return _scope_ref(scope, agent_id)


def _tool_filter(value: str | None) -> str:
    resolved = value or "available"
    if resolved not in {"all", "available", "removed"}:
        raise _invalid_parameter(f"不支持的工具过滤器: {resolved}", "filter")
    return resolved


def _tool_args(payload: dict[str, Any]) -> dict[str, Any]:
    args = payload.get("args")
    if args is None:
        return {}
    if not isinstance(args, dict):
        raise _invalid_parameter("args 必须是 JSON 对象", "args")
    return args


def _required_text(payload: dict[str, Any], field: str, alias: str | None = None) -> str:
    value = payload.get(field)
    if value is None and alias is not None:
        value = payload.get(alias)
    if not isinstance(value, str) or not value.strip():
        raise _missing(field)
    return value.strip()


def _timeout(value: str | None) -> int:
    if value is None:
        return 10
    try:
        timeout = int(value)
    except ValueError as exc:
        raise _invalid_parameter("timeout 必须是非负整数", "timeout") from exc
    if timeout < 0:
        raise _invalid_parameter("timeout 必须是非负整数", "timeout")
    return timeout
