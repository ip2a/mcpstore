from __future__ import annotations

from typing import Any

from fastapi import APIRouter, Depends

from deps import get_store
from errors import ApiError, _raise_core_error, _success
from schemas import ApiEnvelope


router = APIRouter(prefix="/scopes", tags=["scopes"])


def _scope_info(store: Any, view: dict[str, str]) -> ApiEnvelope:
    try:
        info = store.scope_info(view)
    except Exception as exc:
        _raise_core_error(exc)
    if info is None:
        raise ApiError(404, "SCOPE_NOT_FOUND", "作用域不存在", field="scope")
    return _success("作用域详情获取成功", info)


@router.get("/list")
def scopes_list(store: Any = Depends(get_store)) -> ApiEnvelope:
    try:
        scopes = store.list_scopes()
    except Exception as exc:
        _raise_core_error(exc)
    return _success("作用域列表获取成功", {"scopes": scopes, "total": len(scopes)})


@router.get("/root")
def scope_info_root(store: Any = Depends(get_store)) -> ApiEnvelope:
    return _scope_info(store, {"type": "root"})


@router.get("/store")
def scope_info_store(store: Any = Depends(get_store)) -> ApiEnvelope:
    return _scope_info(store, {"type": "store"})


@router.get("/agents/{agent_id}")
def scope_info_agent(agent_id: str, store: Any = Depends(get_store)) -> ApiEnvelope:
    return _scope_info(store, {"type": "agent", "agent_id": agent_id})
