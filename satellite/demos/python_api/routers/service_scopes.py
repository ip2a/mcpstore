from __future__ import annotations

from typing import Any

from fastapi import APIRouter, Body, Depends

from deps import get_store
from errors import _invalid_request, _raise_core_error, _success
from schemas import ApiEnvelope
from validation import _require_object


router = APIRouter(prefix="/services/{service_name}/scopes", tags=["service-scopes"])


@router.put("/store")
def declare_store_scope(
    service_name: str,
    store: Any = Depends(get_store),
    payload: Any = Body(...),
) -> ApiEnvelope:
    descriptor = _require_object(payload, "作用域描述")
    try:
        store.declare_service_scope(service_name, {"type": "store"}, descriptor)
    except (TypeError, ValueError) as exc:
        raise _invalid_request(f"作用域描述无效: {exc}") from exc
    except Exception as exc:
        _raise_core_error(exc, service_name=service_name)
    return _success("Store 作用域已声明", {"status": "ok"})


@router.delete("/store")
def remove_store_scope(service_name: str, store: Any = Depends(get_store)) -> ApiEnvelope:
    try:
        store.remove_service_scope(service_name, {"type": "store"})
    except Exception as exc:
        _raise_core_error(exc, service_name=service_name)
    return _success("Store 作用域已删除", {"status": "ok"})


@router.put("/agents/{agent_id}")
def declare_agent_scope(
    service_name: str,
    agent_id: str,
    store: Any = Depends(get_store),
    payload: Any = Body(...),
) -> ApiEnvelope:
    descriptor = _require_object(payload, "作用域描述")
    try:
        store.declare_service_scope(
            service_name,
            {"type": "agent", "agent_id": agent_id},
            descriptor,
        )
    except (TypeError, ValueError) as exc:
        raise _invalid_request(f"作用域描述无效: {exc}") from exc
    except Exception as exc:
        _raise_core_error(exc, service_name=service_name)
    return _success("Agent 作用域已声明", {"status": "ok"})


@router.delete("/agents/{agent_id}")
def remove_agent_scope(
    service_name: str, agent_id: str, store: Any = Depends(get_store)
) -> ApiEnvelope:
    try:
        store.remove_service_scope(
            service_name, {"type": "agent", "agent_id": agent_id}
        )
    except Exception as exc:
        _raise_core_error(exc, service_name=service_name)
    return _success("Agent 作用域已删除", {"status": "ok"})
