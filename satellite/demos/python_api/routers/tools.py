from __future__ import annotations

from typing import Any

from fastapi import APIRouter, Body, Depends, Query

from deps import get_store
from errors import _invalid_parameter, _missing, _success
from schemas import ApiEnvelope
from store_helpers import call_tool as invoke_tool
from store_helpers import list_tools
from validation import _require_object, _required_text


router = APIRouter(tags=["tools"])


@router.get("/services/{service_name}/tools/list")
def service_list_tools(
    service_name: str,
    store: Any = Depends(get_store),
    scope: str | None = Query(default=None),
    agent_id: str | None = Query(default=None),
    filter: str | None = Query(default=None),
) -> ApiEnvelope:
    selected, tools = list_tools(store, service_name, scope, agent_id, filter)
    return _success(
        "工具列表获取成功",
        {"filter": selected, "tools": tools, "total": len(tools)},
    )


@router.get("/tools/list")
def tools_list(
    store: Any = Depends(get_store),
    service_name: str | None = Query(default=None),
    scope: str | None = Query(default=None),
    agent_id: str | None = Query(default=None),
    filter: str | None = Query(default=None),
) -> ApiEnvelope:
    name = (service_name or "").strip()
    if not name:
        raise _missing("service_name")
    selected, tools = list_tools(store, name, scope, agent_id, filter)
    return _success(
        "工具列表获取成功",
        {
            "service_name": name,
            "filter": selected,
            "tools": tools,
            "total": len(tools),
        },
    )


@router.post("/services/{service_name}/tools/call")
def service_call_tool(
    service_name: str,
    store: Any = Depends(get_store),
    payload: Any = Body(...),
    scope: str | None = Query(default=None),
    agent_id: str | None = Query(default=None),
) -> ApiEnvelope:
    return invoke_tool(
        store,
        service_name,
        scope,
        agent_id,
        _require_object(payload, "工具调用请求"),
    )


@router.post("/tools/call")
def tools_call(store: Any = Depends(get_store), payload: Any = Body(...)) -> ApiEnvelope:
    body = _require_object(payload, "工具调用请求")
    service_name = _required_text(body, "service_name", "service")
    scope = body.get("scope")
    agent_id = body.get("agent_id")
    if scope is not None and not isinstance(scope, str):
        raise _invalid_parameter("scope 必须是字符串", "scope")
    if agent_id is not None and not isinstance(agent_id, str):
        raise _invalid_parameter("agent_id 必须是字符串", "agent_id")
    return invoke_tool(store, service_name, scope, agent_id, body)
