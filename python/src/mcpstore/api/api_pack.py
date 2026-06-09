"""Composable FastAPI routers for the Rust-backed MCPStore facade."""

from __future__ import annotations

from typing import Any, Dict, Optional

from mcpstore.api.api_dependencies import get_store, set_store
from mcpstore.api.api_decorators import validate_agent_id
from mcpstore.core.models import ErrorCode, ResponseBuilder, timed_response

try:
    from fastapi import APIRouter, Body, Query
except ImportError as exc:  # pragma: no cover - only hit without FastAPI installed.
    raise ImportError("mcpstore.api requires fastapi. Install fastapi to use API routers.") from exc


api_store_router = APIRouter(prefix="/for_store", tags=["MCPStore-Store"])
api_agent_router = APIRouter(prefix="/for_agent", tags=["MCPStore-Agent"])
api_cache_router = APIRouter(prefix="/cache", tags=["MCPStore-Cache"])
api_main_router = APIRouter()


def api_set_store(store: Any) -> None:
    set_store(store)


async def _execute(context: Any, value: Any) -> Any:
    if hasattr(value, "__await__"):
        return await value
    return value


@api_store_router.get("/list_services")
@api_store_router.get("/services")
@timed_response
async def store_list_services():
    context = get_store().for_store()
    services = await _execute(context, context.list_services_async())
    return ResponseBuilder.success(message="Services returned", data={"services": services})


@api_store_router.post("/add_service")
@api_store_router.post("/services")
@timed_response
async def store_add_service(payload: Any = Body(...)):
    context = get_store().for_store()
    await _execute(context, context.add_service_async(payload))
    return ResponseBuilder.success(message="Service add submitted", data={"status": "initializing"})


@api_store_router.get("/list_tools")
@api_store_router.get("/tools")
@timed_response
async def store_list_tools(service_name: Optional[str] = Query(None)):
    context = get_store().for_store()
    tools = await _execute(context, context.list_tools_async(service_name=service_name))
    return ResponseBuilder.success(message="Tools returned", data={"tools": tools})


@api_store_router.post("/call_tool")
@timed_response
async def store_call_tool(body: Dict[str, Any] = Body(...)):
    tool_name = body.get("tool_name") or body.get("tool")
    if not tool_name:
        return ResponseBuilder.error(
            code=ErrorCode.MISSING_PARAMETER,
            message="Missing tool_name",
            field="tool_name",
        )
    context = get_store().for_store()
    result = await _execute(context, context.call_tool_async(tool_name, body.get("args") or {}))
    return ResponseBuilder.success(message="Tool call completed", data=result)


@api_store_router.get("/check_services")
@timed_response
async def store_check_services():
    context = get_store().for_store()
    result = await _execute(context, context.check_services_async())
    return ResponseBuilder.success(message="Service health returned", data=result)


@api_store_router.get("/show_config")
@timed_response
async def store_show_config(scope: str = Query("all")):
    context = get_store().for_store()
    result = await _execute(context, context.show_config_async(scope))
    return ResponseBuilder.success(message="Config returned", data=result)


@api_store_router.get("/show_mcpjson")
@timed_response
async def store_show_mcpjson():
    context = get_store().for_store()
    result = await _execute(context, context.show_config_async("mcp"))
    return ResponseBuilder.success(message="MCP JSON returned", data=result)


@api_store_router.get("/setup_config")
@timed_response
async def store_setup_config():
    context = get_store().for_store()
    data = {
        "info": await _execute(context, context.get_info_async()),
        "event_capability": await _execute(context, context.event_capability_report_async()),
    }
    return ResponseBuilder.success(message="Setup config returned", data=data)


@api_store_router.post("/reset_config")
@timed_response
async def store_reset_config():
    context = get_store().for_store()
    ok = await _execute(context, context.reset_config_async())
    return ResponseBuilder.success(message="Config reset", data={"reset": bool(ok)})


@api_store_router.get("/sync_status")
@timed_response
async def store_sync_status():
    context = get_store().for_store()
    capability = await _execute(context, context.event_capability_report_async())
    return ResponseBuilder.success(
        message="Sync status returned",
        data={
            "is_running": False,
            "source": "rust_event_layer",
            "event_capability": capability,
        },
    )


@api_store_router.get("/list_agents")
@timed_response
async def store_list_agents():
    context = get_store().for_store()
    agents = await _execute(context, context.list_agents_async())
    return ResponseBuilder.success(message="Agents returned", data={"agents": agents, "total": len(agents)})


@api_store_router.get("/tool_records")
@timed_response
async def store_tool_records(limit: int = Query(50, ge=1, le=1000)):
    context = get_store().for_store()
    events = await _execute(context, context.event_history_async(limit))
    return ResponseBuilder.success(
        message="Tool records returned",
        data={
            "records": events,
            "total": len(events),
            "source": "rust_event_history",
        },
    )


@api_store_router.get("/service_status/{service_name}")
@timed_response
async def store_service_status(service_name: str):
    context = get_store().for_store()
    result = await _execute(context, context.service_status_async(service_name))
    return ResponseBuilder.success(message="Service status returned", data=result)


@api_store_router.get("/service_info/{service_name}")
@timed_response
async def store_service_info(service_name: str):
    context = get_store().for_store()
    result = await _execute(context, context.service_info_async(service_name))
    return ResponseBuilder.success(message="Service info returned", data=result)


@api_store_router.put("/update_config/{service_name}")
@api_store_router.put("/service/{service_name}")
@timed_response
async def store_update_service(service_name: str, payload: Dict[str, Any] = Body(...)):
    context = get_store().for_store()
    ok = await _execute(context, context.update_service_async(service_name, payload))
    return ResponseBuilder.success(message="Service updated", data={"service_name": service_name, "ok": bool(ok)})


@api_store_router.delete("/delete_config/{service_name}")
@api_store_router.delete("/service/{service_name}")
@timed_response
async def store_delete_service(service_name: str):
    context = get_store().for_store()
    ok = await _execute(context, context.delete_service_async(service_name))
    return ResponseBuilder.success(message="Service deleted", data={"service_name": service_name, "ok": bool(ok)})


@api_store_router.post("/restart_service")
@timed_response
async def store_restart_service(body: Dict[str, Any] = Body(...)):
    service_name = body.get("service_name") or body.get("name")
    if not service_name:
        return ResponseBuilder.error(
            code=ErrorCode.MISSING_PARAMETER,
            message="Missing service_name",
            field="service_name",
        )
    context = get_store().for_store()
    ok = await _execute(context, context.restart_service_async(service_name))
    return ResponseBuilder.success(message="Service restarted", data={"service_name": service_name, "ok": bool(ok)})


@api_store_router.post("/disconnect_service")
@timed_response
async def store_disconnect_service(body: Dict[str, Any] = Body(...)):
    service_name = body.get("service_name") or body.get("name")
    if not service_name:
        return ResponseBuilder.error(
            code=ErrorCode.MISSING_PARAMETER,
            message="Missing service_name",
            field="service_name",
        )
    context = get_store().for_store()
    ok = await _execute(context, context.disconnect_service_async(service_name))
    return ResponseBuilder.success(message="Service disconnected", data={"service_name": service_name, "ok": bool(ok)})


@api_store_router.post("/wait_service")
@timed_response
async def store_wait_service(body: Dict[str, Any] = Body(...)):
    service_name = body.get("service_name") or body.get("name")
    if not service_name:
        return ResponseBuilder.error(
            code=ErrorCode.MISSING_PARAMETER,
            message="Missing service_name",
            field="service_name",
        )
    context = get_store().for_store()
    result = await _execute(
        context,
        context.wait_service_async(
            service_name,
            status=body.get("status"),
            timeout=body.get("timeout", 10.0),
        ),
    )
    return ResponseBuilder.success(message="Service ready status returned", data=result)


@api_agent_router.get("/{agent_id}/services")
@timed_response
async def agent_list_services(agent_id: str):
    validate_agent_id(agent_id)
    context = get_store().for_agent(agent_id)
    services = await _execute(context, context.list_services_async())
    return ResponseBuilder.success(message="Agent services returned", data={"services": services})


@api_agent_router.post("/{agent_id}/services")
@timed_response
async def agent_add_service(agent_id: str, payload: Any = Body(...)):
    validate_agent_id(agent_id)
    context = get_store().for_agent(agent_id)
    await _execute(context, context.add_service_async(payload))
    return ResponseBuilder.success(message="Agent service add submitted", data={"status": "initializing"})


@api_agent_router.get("/{agent_id}/tools")
@timed_response
async def agent_list_tools(agent_id: str, service_name: Optional[str] = Query(None)):
    validate_agent_id(agent_id)
    context = get_store().for_agent(agent_id)
    tools = await _execute(context, context.list_tools_async(service_name=service_name))
    return ResponseBuilder.success(message="Agent tools returned", data={"tools": tools})


@api_agent_router.post("/{agent_id}/call_tool")
@timed_response
async def agent_call_tool(agent_id: str, body: Dict[str, Any] = Body(...)):
    validate_agent_id(agent_id)
    tool_name = body.get("tool_name") or body.get("tool")
    if not tool_name:
        return ResponseBuilder.error(
            code=ErrorCode.MISSING_PARAMETER,
            message="Missing tool_name",
            field="tool_name",
        )
    context = get_store().for_agent(agent_id)
    result = await _execute(context, context.call_tool_async(tool_name, body.get("args") or {}))
    return ResponseBuilder.success(message="Agent tool call completed", data=result)


@api_agent_router.get("/{agent_id}/check_services")
@timed_response
async def agent_check_services(agent_id: str):
    validate_agent_id(agent_id)
    context = get_store().for_agent(agent_id)
    result = await _execute(context, context.check_services_async())
    return ResponseBuilder.success(message="Agent service health returned", data=result)


@api_agent_router.post("/{agent_id}/wait_service")
@timed_response
async def agent_wait_service(agent_id: str, body: Dict[str, Any] = Body(...)):
    validate_agent_id(agent_id)
    service_name = body.get("service_name") or body.get("name")
    if not service_name:
        return ResponseBuilder.error(
            code=ErrorCode.MISSING_PARAMETER,
            message="Missing service_name",
            field="service_name",
        )
    context = get_store().for_agent(agent_id)
    result = await _execute(
        context,
        context.wait_service_async(
            service_name,
            status=body.get("status"),
            timeout=body.get("timeout", 10.0),
        ),
    )
    return ResponseBuilder.success(message="Agent service ready status returned", data=result)


@api_agent_router.get("/{agent_id}/service_status/{service_name}")
@timed_response
async def agent_service_status(agent_id: str, service_name: str):
    validate_agent_id(agent_id)
    context = get_store().for_agent(agent_id)
    result = await _execute(context, context.service_status_async(service_name))
    return ResponseBuilder.success(message="Agent service status returned", data=result)


@api_agent_router.get("/{agent_id}/service_info/{service_name}")
@timed_response
async def agent_service_info(agent_id: str, service_name: str):
    validate_agent_id(agent_id)
    context = get_store().for_agent(agent_id)
    result = await _execute(context, context.service_info_async(service_name))
    return ResponseBuilder.success(message="Agent service info returned", data=result)


@api_agent_router.put("/{agent_id}/service/{service_name}")
@timed_response
async def agent_update_service(agent_id: str, service_name: str, payload: Dict[str, Any] = Body(...)):
    validate_agent_id(agent_id)
    context = get_store().for_agent(agent_id)
    ok = await _execute(context, context.update_service_async(service_name, payload))
    return ResponseBuilder.success(message="Agent service updated", data={"service_name": service_name, "ok": bool(ok)})


@api_agent_router.delete("/{agent_id}/service/{service_name}")
@timed_response
async def agent_delete_service(agent_id: str, service_name: str):
    validate_agent_id(agent_id)
    context = get_store().for_agent(agent_id)
    ok = await _execute(context, context.delete_service_async(service_name))
    return ResponseBuilder.success(message="Agent service deleted", data={"service_name": service_name, "ok": bool(ok)})


@api_cache_router.get("/inspect")
@timed_response
async def cache_inspect():
    cache = get_store().for_store().find_cache()
    return ResponseBuilder.success(message="Cache inspect returned", data=cache.inspect())


api_main_router.include_router(api_store_router)
api_main_router.include_router(api_agent_router)
api_main_router.include_router(api_cache_router)
