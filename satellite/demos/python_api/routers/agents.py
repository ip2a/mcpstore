from __future__ import annotations

from typing import Any

from fastapi import APIRouter, Depends

from deps import get_store
from errors import ApiError, _raise_core_error, _success
from schemas import ApiEnvelope


router = APIRouter(prefix="/agents", tags=["agents"])


@router.get("/list")
def list_agents(store: Any = Depends(get_store)) -> ApiEnvelope:
    try:
        agents = store.list_agents()
    except Exception as exc:
        _raise_core_error(exc)
    return _success("Agent 列表获取成功", {"agents": agents, "total": len(agents)})


@router.get("/{agent_id}")
def agent_info(agent_id: str, store: Any = Depends(get_store)) -> ApiEnvelope:
    try:
        agent = store.find_agent(agent_id)
    except Exception as exc:
        _raise_core_error(exc)
    if agent is None:
        raise ApiError(
            404,
            "AGENT_NOT_FOUND",
            f"Agent '{agent_id}' 不存在",
            field="agent_id",
        )
    return _success("Agent 详情获取成功", agent)
