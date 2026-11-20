from __future__ import annotations

from typing import Any

from fastapi import APIRouter, Body, Depends, Query

from deps import get_store
from errors import _invalid_request, _raise_core_error, _success
from schemas import ApiEnvelope
from store_helpers import lifecycle, service_info, wait_ready
from validation import _require_object, _scope_ref, _scope_view


router = APIRouter(prefix="/services", tags=["services"])


@router.get("/list")
def service_list_services(
    store: Any = Depends(get_store),
    scope: str | None = Query(default=None),
    agent_id: str | None = Query(default=None),
) -> ApiEnvelope:
    view = _scope_view(scope, agent_id)
    try:
        services = store.list_services_viewed(view)
    except Exception as exc:
        _raise_core_error(exc)
    return _success(
        "服务列表获取成功", {"services": services, "total": len(services)}
    )


@router.get("/{service_name}")
def service_info_route(
    service_name: str,
    store: Any = Depends(get_store),
    scope: str | None = Query(default=None),
    agent_id: str | None = Query(default=None),
) -> ApiEnvelope:
    info = service_info(store, service_name, _scope_ref(scope, agent_id))
    return _success("服务信息获取成功", info)


@router.post("/{service_name}")
def add_service_definition(
    service_name: str,
    store: Any = Depends(get_store),
    payload: Any = Body(...),
) -> ApiEnvelope:
    config = _require_object(payload, "服务配置")
    try:
        store.add_service(service_name, config)
    except (TypeError, ValueError) as exc:
        raise _invalid_request(f"服务配置无效: {exc}") from exc
    except Exception as exc:
        _raise_core_error(exc, service_name=service_name)
    return _success("服务定义添加成功", {"status": "ok"})


@router.put("/{service_name}")
def update_service_definition(
    service_name: str,
    store: Any = Depends(get_store),
    payload: Any = Body(...),
) -> ApiEnvelope:
    config = _require_object(payload, "服务配置")
    if "_mcpstore" in config:
        raise _invalid_request(
            "基础配置更新不能包含 _mcpstore；请使用作用域接口修改 scope"
        )
    try:
        store.update_service(service_name, config)
    except (TypeError, ValueError) as exc:
        raise _invalid_request(f"服务配置无效: {exc}") from exc
    except Exception as exc:
        _raise_core_error(exc, service_name=service_name)
    return _success("服务定义更新成功", {"status": "ok"})


@router.delete("/{service_name}")
def remove_service_definition(
    service_name: str, store: Any = Depends(get_store)
) -> ApiEnvelope:
    try:
        store.remove_service(service_name)
    except Exception as exc:
        _raise_core_error(exc, service_name=service_name)
    return _success("服务定义删除成功", {"status": "ok"})


@router.post("/{service_name}/connect")
def service_connect(
    service_name: str,
    store: Any = Depends(get_store),
    scope: str | None = Query(default=None),
    agent_id: str | None = Query(default=None),
) -> ApiEnvelope:
    return lifecycle(
        store,
        service_name,
        scope,
        agent_id,
        store.connect_service,
        "服务连接成功",
    )


@router.post("/{service_name}/disconnect")
def service_disconnect(
    service_name: str,
    store: Any = Depends(get_store),
    scope: str | None = Query(default=None),
    agent_id: str | None = Query(default=None),
) -> ApiEnvelope:
    return lifecycle(
        store,
        service_name,
        scope,
        agent_id,
        store.disconnect_service,
        "服务断开成功",
    )


@router.post("/{service_name}/restart")
def service_restart(
    service_name: str,
    store: Any = Depends(get_store),
    scope: str | None = Query(default=None),
    agent_id: str | None = Query(default=None),
) -> ApiEnvelope:
    return lifecycle(
        store,
        service_name,
        scope,
        agent_id,
        store.restart_service,
        "服务重启成功",
    )


@router.get("/{service_name}/wait")
def service_wait(
    service_name: str,
    store: Any = Depends(get_store),
    scope: str | None = Query(default=None),
    agent_id: str | None = Query(default=None),
    timeout: str | None = Query(default=None),
) -> ApiEnvelope:
    return wait_ready(store, service_name, scope, agent_id, timeout)
