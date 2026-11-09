from typing import Any, Dict, List, Optional, Union

from fastapi import APIRouter, Body, Query, Request

from mcpstore.core.models import ErrorCode, ResponseBuilder, timed_response
from mcpstore.api.api_dependencies import get_store

router = APIRouter(prefix="/store", tags=["Mini-Store"])


@router.get("/sync_status")
@timed_response
async def sync_status():
    """View the sync status (if a sync manager is present)."""
    store = get_store()
    ctx = store.for_store()
    sync_manager = getattr(store.orchestrator, "sync_manager", None)
    if sync_manager:
        status = sync_manager.get_sync_status()
    else:
        status = {"is_running": False, "reason": "sync_manager_not_initialized"}
    return ResponseBuilder.success(message="Sync status", data=status)


@router.get("/services")
@timed_response
async def list_services(
    status: Optional[str] = Query(None, description="按状态过滤"),
    search: Optional[str] = Query(None, description="按名称模糊搜索"),
):
    """List store-level services (lightweight filtering)."""
    store = get_store()
    context = store.for_store()
    services = await context.bridge_execute(context.list_services_async())

    filtered = services
    if status:
        filtered = [s for s in filtered if str(s.get("status", s.status)).lower() == status.lower()]
    if search:
        search_lower = search.lower()
        filtered = [
            s for s in filtered
            if search_lower in str(s.get("name", getattr(s, "name", ""))).lower()
        ]

    return ResponseBuilder.success(
        message=f"返回 {len(filtered)} 条服务",
        data={"services": filtered, "total": len(filtered)}
    )


@router.post("/services")
@timed_response
async def add_service(
    payload: Union[Dict[str, Any], List[Dict[str, Any]], str] = Body(..., description="服务配置"),
):
    """Add a store-level service (passed straight through to the SDK)."""
    if payload is None:
        return ResponseBuilder.error(
            code=ErrorCode.MISSING_PARAMETER,
            message="缺少必填参数 payload",
            details={"expected": "服务配置"}
        )

    store = get_store()
    context = store.for_store()
    try:
        await context.bridge_execute(context.add_service_async(payload))
    except Exception as e:
        return ResponseBuilder.error(
            code=ErrorCode.SERVICE_INITIALIZATION_FAILED,
            message="服务注册失败",
            details={"error": str(e)}
        )

    return ResponseBuilder.success(
        message="服务添加请求已提交",
        data={"status": "initializing"}
    )


@router.post("/reset_service")
@timed_response
async def reset_service(body: Dict[str, Any] = Body(..., description="包含 service_name")):
    """Reset the state of the specified service."""
    service_name = body.get("service_name") or body.get("name")
    if not service_name:
        return ResponseBuilder.error(
            code=ErrorCode.VALIDATION_ERROR,
            message="缺少 service_name",
            field="service_name"
        )
    store = get_store()
    ctx = store.for_store()
    ok = await ctx.bridge_execute(ctx.reset_service_async(service_name))
    if not ok:
        return ResponseBuilder.error(
            code=ErrorCode.SERVICE_OPERATION_FAILED,
            message=f"服务 {service_name} 重置失败",
            field="service_name"
        )
    return ResponseBuilder.success(
        message=f"服务 {service_name} 已重置",
        data={"service_name": service_name, "status": "reset"}
    )


@router.get("/tools")
@timed_response
async def list_tools(
    service_name: Optional[str] = Query(None, description="按服务过滤"),
    limit: Optional[int] = Query(None, ge=1, le=200, description="可选 limit"),
):
    """List store-level tools."""
    store = get_store()
    ctx = store.for_store()
    tools = await ctx.bridge_execute(ctx.list_tools_async())

    filtered = tools
    if service_name:
        filtered = [t for t in tools if t.get("service") == service_name or t.get("service_name") == service_name]
    if limit:
        filtered = filtered[:limit]

    return ResponseBuilder.success(
        message=f"返回 {len(filtered)} 个工具",
        data={"tools": filtered, "total": len(filtered)}
    )


@router.get("/check_services")
@timed_response
async def check_services():
    """Service health check."""
    store = get_store()
    ctx = store.for_store()
    result = await ctx.bridge_execute(ctx.check_services_async())
    return ResponseBuilder.success(message="健康检查结果", data=result)


@router.post("/call_tool")
@timed_response
async def call_tool(body: Dict[str, Any] = Body(..., description="工具调用参数")):
    """Call the specified tool."""
    tool_name = body.get("tool_name") or body.get("tool")
    args = body.get("args") or {}
    if not tool_name:
        return ResponseBuilder.error(
            code=ErrorCode.MISSING_PARAMETER,
            message="缺少 tool_name",
            field="tool_name"
        )
    store = get_store()
    ctx = store.for_store()
    result = await ctx.bridge_execute(ctx.call_tool_async(tool_name, args))
    return ResponseBuilder.success(message="工具调用完成", data=result)


@router.put("/service/{service_name}")
@timed_response
async def update_service(service_name: str, body: Dict[str, Any]):
    """Update a service's configuration (patch-style)."""
    store = get_store()
    ctx = store.for_store()
    ok = await ctx.bridge_execute(ctx.update_service_async(service_name, body))
    if not ok:
        return ResponseBuilder.error(
            code=ErrorCode.SERVICE_OPERATION_FAILED,
            message=f"服务 {service_name} 更新失败",
            field="service_name"
        )
    return ResponseBuilder.success(
        message=f"服务 {service_name} 已更新",
        data={"service_name": service_name}
    )


@router.delete("/service/{service_name}")
@timed_response
async def delete_service(service_name: str):
    """Delete a service."""
    store = get_store()
    ctx = store.for_store()
    ok = await ctx.bridge_execute(ctx.delete_service_async(service_name))
    if not ok:
        return ResponseBuilder.error(
            code=ErrorCode.SERVICE_NOT_FOUND,
            message=f"服务 {service_name} 删除失败",
            field="service_name"
        )
    return ResponseBuilder.success(
        message=f"服务 {service_name} 已删除",
        data={"service_name": service_name}
    )


@router.post("/disconnect_service")
@timed_response
async def disconnect_service(body: Dict[str, Any] = Body(..., description="包含 service_name, reason")):
    """Disconnect a service without modifying its configuration."""
    service_name = body.get("service_name") or body.get("name")
    reason = body.get("reason", "user_requested")
    if not service_name:
        return ResponseBuilder.error(
            code=ErrorCode.VALIDATION_ERROR,
            message="缺少 service_name",
            field="service_name"
        )
    store = get_store()
    ctx = store.for_store()
    ok = await ctx.bridge_execute(ctx.disconnect_service_async(service_name, reason=reason))
    if not ok:
        return ResponseBuilder.error(
            code=ErrorCode.SERVICE_OPERATION_FAILED,
            message=f"服务 {service_name} 断开失败",
            field="service_name"
        )
    return ResponseBuilder.success(
        message=f"服务 {service_name} 已断开",
        data={"service_name": service_name, "status": "disconnected"}
    )


@router.get("/config")
@timed_response
async def show_config():
    """View the store configuration."""
    store = get_store()
    ctx = store.for_store()
    data = await ctx.bridge_execute(ctx.show_config_async())
    if isinstance(data, dict) and "error" in data:
        return ResponseBuilder.error(
            code=ErrorCode.CONFIGURATION_ERROR,
            message=data["error"],
            details=data
        )
    return ResponseBuilder.success(message="配置获取成功", data=data)


@router.delete("/config/{client_id_or_service}")
@timed_response
async def delete_config(client_id_or_service: str):
    """Delete a configuration."""
    store = get_store()
    ctx = store.for_store()
    result = await ctx.bridge_execute(ctx.delete_config_async(client_id_or_service))
    if not result.get("success"):
        return ResponseBuilder.error(
            code=ErrorCode.CONFIGURATION_ERROR,
            message=result.get("error", "删除配置失败"),
            details=result
        )
    return ResponseBuilder.success(message="配置删除成功", data=result)


@router.put("/config/{client_id_or_service}")
@timed_response
async def update_config(client_id_or_service: str, new_config: Dict[str, Any]):
    """Update a configuration."""
    store = get_store()
    ctx = store.for_store()
    result = await ctx.bridge_execute(ctx.update_config_async(client_id_or_service, new_config))
    if not result.get("success"):
        return ResponseBuilder.error(
            code=ErrorCode.CONFIGURATION_ERROR,
            message=result.get("error", "更新配置失败"),
            details=result
        )
    return ResponseBuilder.success(message="配置更新成功", data=result)


@router.post("/config/reset")
@timed_response
async def reset_config():
    """Reset the configuration."""
    store = get_store()
    ctx = store.for_store()
    ok = await ctx.bridge_execute(ctx.reset_config_async())
    if not ok:
        return ResponseBuilder.error(
            code=ErrorCode.CONFIGURATION_ERROR,
            message="配置重置失败"
        )
    return ResponseBuilder.success(message="配置已重置", data={"reset": True})


@router.get("/service_info/{service_name}")
@timed_response
async def service_info(service_name: str):
    """Get service details (simplified)."""
    store = get_store()
    ctx = store.for_store()
    info = await ctx.bridge_execute(ctx.service_info_async(service_name))
    service = getattr(info, "service", None) if info else None
    if not service:
        return ResponseBuilder.error(
            code=ErrorCode.SERVICE_NOT_FOUND,
            message=f"服务 {service_name} 未找到",
            field="service_name"
        )
    svc = {
        "name": getattr(service, "name", ""),
        "status": getattr(service.status, "value", str(service.status)),
        "type": getattr(service.transport_type, "value", "unknown"),
        "tool_count": getattr(service, "tool_count", 0),
        "client_id": getattr(service, "client_id", ""),
    }
    return ResponseBuilder.success(message="服务详情返回", data=svc)


@router.get("/service_status/{service_name}")
@timed_response
async def service_status(service_name: str):
    """Get store-level service status."""
    store = get_store()
    context = store.for_store()
    status = await context.bridge_execute(context.service_status_async(service_name))

    if status.get("status") == "unknown" and not status.get("client_id"):
        return ResponseBuilder.error(
            code=ErrorCode.SERVICE_NOT_FOUND,
            message=f"服务 {service_name} 未找到",
            field="service_name"
        )

    return ResponseBuilder.success(
        message=f"服务 {service_name} 状态返回",
        data=status
    )


@router.get("/tool_records")
@timed_response
async def tool_records(limit: int = 50):
    """Read tool execution records."""
    store = get_store()
    ctx = store.for_store()
    data = await ctx.bridge_execute(ctx.tool_records_async(limit))
    return ResponseBuilder.success(
        message=f"返回 {len(data.get('executions', []))} 条记录",
        data=data
    )
