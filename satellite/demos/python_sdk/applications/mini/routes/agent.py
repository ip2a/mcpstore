from typing import Any, Dict, List, Optional, Union

from fastapi import APIRouter, Body, Query

from mcpstore.core.models import ErrorCode, ResponseBuilder, timed_response
from mcpstore.api.api_dependencies import get_store
from mcpstore.api.api_decorators import validate_agent_id

router = APIRouter(prefix="/agents", tags=["Mini-Agent"])


@router.get("/{agent_id}/services")
@timed_response
async def list_services(
    agent_id: str,
    status: Optional[str] = Query(None, description="按状态过滤"),
    search: Optional[str] = Query(None, description="名称模糊搜索"),
):
    """List agent-level services (lightweight filtering)."""
    validate_agent_id(agent_id)
    store = get_store()
    context = store.for_agent(agent_id)
    services = await context.bridge_execute(context.list_services_async())

    simplified: List[Dict[str, Any]] = []
    for s in services:
        simplified.append({
            "name": s.name,
            "status": s.status.value if hasattr(s.status, "value") else str(s.status),
            "type": s.transport_type.value if s.transport_type else "unknown",
            "tool_count": getattr(s, "tool_count", 0),
            "client_id": s.client_id or "",
        })

    filtered = simplified
    if status:
        filtered = [s for s in filtered if s.get("status", "").lower() == status.lower()]
    if search:
        search_lower = search.lower()
        filtered = [s for s in filtered if search_lower in s.get("name", "").lower()]

    return ResponseBuilder.success(
        message=f"返回 {len(filtered)} 条服务（agent={agent_id}）",
        data={"services": filtered, "total": len(filtered)}
    )


@router.post("/{agent_id}/services")
@timed_response
async def add_service(
    agent_id: str,
    payload: Union[Dict[str, Any], List[Dict[str, Any]], str] = Body(..., description="服务配置"),
):
    """Add services to the specified agent."""
    validate_agent_id(agent_id)
    if payload is None:
        return ResponseBuilder.error(
            code=ErrorCode.MISSING_PARAMETER,
            message="缺少必填参数 payload",
            details={"expected": "服务配置"}
        )

    store = get_store()
    context = store.for_agent(agent_id)
    try:
        await context.bridge_execute(context.add_service_async(payload))
    except Exception as e:
        return ResponseBuilder.error(
            code=ErrorCode.SERVICE_INITIALIZATION_FAILED,
            message=f"服务注册失败（agent={agent_id}）",
            details={"error": str(e)}
        )

    return ResponseBuilder.success(
        message=f"服务添加请求已提交（agent={agent_id}）",
        data={"status": "initializing"}
    )


@router.put("/{agent_id}/service/{service_name}")
@timed_response
async def update_service(agent_id: str, service_name: str, body: Dict[str, Any]):
    """Update an agent service's configuration."""
    validate_agent_id(agent_id)
    store = get_store()
    ctx = store.for_agent(agent_id)
    ok = await ctx.bridge_execute(ctx.update_service_async(service_name, body))
    if not ok:
        return ResponseBuilder.error(
            code=ErrorCode.SERVICE_OPERATION_FAILED,
            message=f"服务 {service_name} 更新失败（agent={agent_id}）",
            field="service_name"
        )
    return ResponseBuilder.success(
        message=f"服务 {service_name} 已更新（agent={agent_id}）",
        data={"service_name": service_name}
    )


@router.delete("/{agent_id}/service/{service_name}")
@timed_response
async def delete_service(agent_id: str, service_name: str):
    """Delete an agent service."""
    validate_agent_id(agent_id)
    store = get_store()
    ctx = store.for_agent(agent_id)
    ok = await ctx.bridge_execute(ctx.delete_service_async(service_name))
    if not ok:
        return ResponseBuilder.error(
            code=ErrorCode.SERVICE_NOT_FOUND,
            message=f"服务 {service_name} 删除失败（agent={agent_id}）",
            field="service_name"
        )
    return ResponseBuilder.success(
        message=f"服务 {service_name} 已删除（agent={agent_id}）",
        data={"service_name": service_name}
    )


@router.post("/{agent_id}/disconnect_service")
@timed_response
async def disconnect_service(agent_id: str, body: Dict[str, Any] = Body(..., description="包含 service_name, reason")):
    """Disconnect an agent service without modifying its configuration."""
    validate_agent_id(agent_id)
    service_name = body.get("service_name") or body.get("name")
    reason = body.get("reason", "user_requested")
    if not service_name:
        return ResponseBuilder.error(
            code=ErrorCode.VALIDATION_ERROR,
            message="缺少 service_name",
            field="service_name"
        )
    store = get_store()
    ctx = store.for_agent(agent_id)
    ok = await ctx.bridge_execute(ctx.disconnect_service_async(service_name, reason=reason))
    if not ok:
        return ResponseBuilder.error(
            code=ErrorCode.SERVICE_OPERATION_FAILED,
            message=f"服务 {service_name} 断开失败（agent={agent_id}）",
            field="service_name"
        )
    return ResponseBuilder.success(
        message=f"服务 {service_name} 已断开（agent={agent_id}）",
        data={"service_name": service_name, "status": "disconnected"}
    )


@router.get("/{agent_id}/service_status/{service_name}")
@timed_response
async def service_status(agent_id: str, service_name: str):
    """Get agent-level service status (resolves local names automatically)."""
    validate_agent_id(agent_id)
    store = get_store()
    context = store.for_agent(agent_id)
    status = await context.bridge_execute(context.service_status_async(service_name))

    if status.get("status") == "unknown" and not status.get("client_id"):
        return ResponseBuilder.error(
            code=ErrorCode.SERVICE_NOT_FOUND,
            message=f"服务 {service_name} 未找到（agent={agent_id}）",
            field="service_name"
        )

    return ResponseBuilder.success(
        message=f"服务 {service_name} 状态返回（agent={agent_id}）",
        data=status
    )


@router.get("/{agent_id}/service_info/{service_name}")
@timed_response
async def service_info(agent_id: str, service_name: str):
    """Get agent-level service details."""
    validate_agent_id(agent_id)
    store = get_store()
    ctx = store.for_agent(agent_id)
    info = await ctx.bridge_execute(ctx.service_info_async(service_name))
    service = getattr(info, "service", None) if info else None
    if not service:
        return ResponseBuilder.error(
            code=ErrorCode.SERVICE_NOT_FOUND,
            message=f"服务 {service_name} 未找到（agent={agent_id}）",
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


@router.get("/{agent_id}/tools")
@timed_response
async def list_tools(
    agent_id: str,
    service_name: Optional[str] = Query(None, description="按服务过滤"),
    limit: Optional[int] = Query(None, ge=1, le=200, description="可选 limit"),
):
    """List agent-level tools."""
    validate_agent_id(agent_id)
    store = get_store()
    ctx = store.for_agent(agent_id)
    tools = await ctx.bridge_execute(ctx.list_tools_async())
    filtered = tools
    if service_name:
        filtered = [t for t in tools if t.get("service") == service_name or t.get("service_name") == service_name]
    if limit:
        filtered = filtered[:limit]
    return ResponseBuilder.success(
        message=f"返回 {len(filtered)} 个工具（agent={agent_id}）",
        data={"tools": filtered, "total": len(filtered)}
    )


@router.post("/{agent_id}/call_tool")
@timed_response
async def call_tool(agent_id: str, body: Dict[str, Any] = Body(..., description="工具调用参数")):
    """Call a tool on behalf of the specified agent."""
    validate_agent_id(agent_id)
    tool_name = body.get("tool_name") or body.get("tool")
    args = body.get("args") or {}
    if not tool_name:
        return ResponseBuilder.error(
            code=ErrorCode.MISSING_PARAMETER,
            message="缺少 tool_name",
            field="tool_name"
        )
    store = get_store()
    ctx = store.for_agent(agent_id)
    result = await ctx.bridge_execute(ctx.call_tool_async(tool_name, args))
    return ResponseBuilder.success(message="工具调用完成", data=result)


@router.get("/{agent_id}/check_services")
@timed_response
async def check_services(agent_id: str):
    """Health check from the agent's perspective."""
    validate_agent_id(agent_id)
    store = get_store()
    ctx = store.for_agent(agent_id)
    result = await ctx.bridge_execute(ctx.check_services_async())
    return ResponseBuilder.success(message="健康检查结果", data=result)


@router.get("/{agent_id}/config")
@timed_response
async def show_config(agent_id: str):
    """View the agent's configuration."""
    validate_agent_id(agent_id)
    store = get_store()
    ctx = store.for_agent(agent_id)
    data = await ctx.bridge_execute(ctx.show_config_async())
    if isinstance(data, dict) and "error" in data:
        return ResponseBuilder.error(
            code=ErrorCode.CONFIGURATION_ERROR,
            message=data["error"],
            details=data
        )
    return ResponseBuilder.success(message="配置获取成功", data=data)


@router.delete("/{agent_id}/config/{client_id_or_service}")
@timed_response
async def delete_config(agent_id: str, client_id_or_service: str):
    """Delete the agent's configuration."""
    validate_agent_id(agent_id)
    store = get_store()
    ctx = store.for_agent(agent_id)
    result = await ctx.bridge_execute(ctx.delete_config_async(client_id_or_service))
    if not result.get("success"):
        return ResponseBuilder.error(
            code=ErrorCode.CONFIGURATION_ERROR,
            message=result.get("error", "删除配置失败"),
            details=result
        )
    return ResponseBuilder.success(message="配置删除成功", data=result)


@router.put("/{agent_id}/config/{client_id_or_service}")
@timed_response
async def update_config(agent_id: str, client_id_or_service: str, new_config: Dict[str, Any]):
    """Update the agent's configuration."""
    validate_agent_id(agent_id)
    store = get_store()
    ctx = store.for_agent(agent_id)
    result = await ctx.bridge_execute(ctx.update_config_async(client_id_or_service, new_config))
    if not result.get("success"):
        return ResponseBuilder.error(
            code=ErrorCode.CONFIGURATION_ERROR,
            message=result.get("error", "更新配置失败"),
            details=result
        )
    return ResponseBuilder.success(message="配置更新成功", data=result)


@router.post("/{agent_id}/config/reset")
@timed_response
async def reset_config(agent_id: str):
    """Reset the agent's configuration."""
    validate_agent_id(agent_id)
    store = get_store()
    ctx = store.for_agent(agent_id)
    ok = await ctx.bridge_execute(ctx.reset_config_async())
    if not ok:
        return ResponseBuilder.error(
            code=ErrorCode.CONFIGURATION_ERROR,
            message="配置重置失败"
        )
    return ResponseBuilder.success(message="配置已重置", data={"reset": True})


@router.get("/{agent_id}/tool_records")
@timed_response
async def tool_records(agent_id: str, limit: int = 50):
    """Read tool execution records (agent perspective)."""
    validate_agent_id(agent_id)
    store = get_store()
    ctx = store.for_agent(agent_id)
    data = await ctx.bridge_execute(ctx.tool_records_async(limit))
    return ResponseBuilder.success(
        message=f"返回 {len(data.get('executions', []))} 条记录（agent={agent_id}）",
        data=data
    )
