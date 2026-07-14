"""FastAPI routers for explicit definition, scope, and instance APIs."""

from __future__ import annotations

from typing import Any, Dict, Optional

from mcpstore.api.api_dependencies import get_store, set_store
from mcpstore.api.api_decorators import validate_agent_id
from mcpstore.core.models import (
    AddServiceRequest,
    AgentOnlyServiceRequest,
    PatchServiceRequest,
    ResponseBuilder,
    ScopeDescriptor,
    UpdateServiceRequest,
    timed_response,
)

try:
    from fastapi import APIRouter, Body, Query
except ImportError as exc:  # pragma: no cover
    raise ImportError("mcpstore.api requires fastapi. Install mcpstore[api].") from exc


api_store_router = APIRouter(tags=["MCPStore-Definitions"])
api_agent_router = APIRouter(prefix="/agents", tags=["MCPStore-Agent-Definitions"])
api_instance_router = APIRouter(prefix="/instances", tags=["MCPStore-Instances"])
api_session_router = APIRouter(prefix="/sessions", tags=["MCPStore-Sessions"])
api_cache_router = APIRouter(prefix="/cache", tags=["MCPStore-Cache"])
api_main_router = APIRouter()


def api_set_store(store: Any) -> None:
    set_store(store)


@api_store_router.post("/services/{service_name}")
@timed_response
async def add_service(service_name: str, request: AddServiceRequest):
    get_store().add_service(service_name, request.config)
    return ResponseBuilder.success(
        message="Service definition created",
        data={"service_name": service_name},
    )


@api_store_router.get("/services/{service_name}")
@timed_response
async def get_service_definition(service_name: str):
    config = get_store().get_definition_config(service_name)
    return ResponseBuilder.success(
        message="Service definition returned",
        data={"service_name": service_name, "config": config},
    )


@api_store_router.put("/services/{service_name}")
@timed_response
async def update_service(service_name: str, request: UpdateServiceRequest):
    get_store().update_service(service_name, request.config)
    return ResponseBuilder.success(
        message="Service base configuration updated",
        data={"service_name": service_name},
    )


@api_store_router.patch("/services/{service_name}")
@timed_response
async def patch_service(service_name: str, request: PatchServiceRequest):
    get_store().patch_service(service_name, request.updates)
    return ResponseBuilder.success(
        message="Service base configuration patched",
        data={"service_name": service_name},
    )


@api_store_router.delete("/services/{service_name}")
@timed_response
async def remove_service(service_name: str):
    get_store().remove_service(service_name)
    return ResponseBuilder.success(
        message="Service definition removed",
        data={"service_name": service_name},
    )


@api_store_router.put("/services/{service_name}/scopes/store")
@timed_response
async def declare_store_scope(
    service_name: str,
    descriptor: ScopeDescriptor = Body(default_factory=ScopeDescriptor),
):
    instance_id = get_store().declare_service_scope(
        service_name,
        {"type": "store"},
        descriptor,
    )
    return ResponseBuilder.success(
        message="Service scope declared",
        data={"service_name": service_name, "instance_id": instance_id},
    )


@api_store_router.delete("/services/{service_name}/scopes/store")
@timed_response
async def remove_store_scope(service_name: str):
    get_store().remove_service_scope(service_name, {"type": "store"})
    return ResponseBuilder.success(
        message="Service scope removed",
        data={"service_name": service_name},
    )


@api_store_router.get("/services/{service_name}/scopes/store/effective-config")
@timed_response
async def get_store_effective_config(service_name: str):
    scope = {"type": "store"}
    config = get_store().get_effective_config(service_name, scope)
    return ResponseBuilder.success(
        message="Effective service configuration returned",
        data={"service_name": service_name, "scope": scope, "config": config},
    )


@api_store_router.put("/services/{service_name}/scopes/agents/{agent_id}")
@timed_response
async def declare_agent_scope(
    service_name: str,
    agent_id: str,
    descriptor: ScopeDescriptor = Body(default_factory=ScopeDescriptor),
):
    validate_agent_id(agent_id)
    instance_id = get_store().declare_service_scope(
        service_name,
        {"type": "agent", "agent_id": agent_id},
        descriptor,
    )
    return ResponseBuilder.success(
        message="Service scope declared",
        data={"service_name": service_name, "instance_id": instance_id},
    )


@api_store_router.delete("/services/{service_name}/scopes/agents/{agent_id}")
@timed_response
async def remove_agent_scope(service_name: str, agent_id: str):
    validate_agent_id(agent_id)
    get_store().remove_service_scope(
        service_name,
        {"type": "agent", "agent_id": agent_id},
    )
    return ResponseBuilder.success(
        message="Service scope removed",
        data={"service_name": service_name, "agent_id": agent_id},
    )


@api_store_router.get(
    "/services/{service_name}/scopes/agents/{agent_id}/effective-config"
)
@timed_response
async def get_agent_effective_config(service_name: str, agent_id: str):
    validate_agent_id(agent_id)
    scope = {"type": "agent", "agent_id": agent_id}
    config = get_store().get_effective_config(service_name, scope)
    return ResponseBuilder.success(
        message="Effective service configuration returned",
        data={"service_name": service_name, "scope": scope, "config": config},
    )


@api_agent_router.post("/{agent_id}/services/{service_name}")
@timed_response
async def add_agent_only_service(
    agent_id: str,
    service_name: str,
    request: AgentOnlyServiceRequest,
):
    validate_agent_id(agent_id)
    config = dict(request.config)
    config["_mcpstore"] = {
        "scopes": {
            "agents": {
                agent_id: request.descriptor.model_dump(
                    mode="json",
                    exclude_none=False,
                )
            }
        }
    }
    get_store().add_service(service_name, config)
    return ResponseBuilder.success(
        message="Agent-only service definition created",
        data={"service_name": service_name, "agent_id": agent_id},
    )


@api_instance_router.get("")
@timed_response
async def list_instances():
    instances = get_store().list_instances()
    return ResponseBuilder.success(
        message="Instances returned",
        data={"instances": instances, "total": len(instances)},
    )


@api_instance_router.get("/{instance_id}")
@timed_response
async def get_instance(instance_id: str):
    instance = get_store().find_instance(instance_id)
    return ResponseBuilder.success(
        message="Instance returned",
        data={"instance": instance},
    )


@api_instance_router.get("/{instance_id}/status")
@timed_response
async def get_instance_status(instance_id: str):
    status = get_store().instance_status(instance_id)
    return ResponseBuilder.success(message="Instance status returned", data=status)


@api_instance_router.post("/{instance_id}/connect")
@timed_response
async def connect_instance(instance_id: str):
    get_store().connect_service(instance_id)
    return ResponseBuilder.success(
        message="Instance connected",
        data={"instance_id": instance_id},
    )


@api_instance_router.post("/{instance_id}/disconnect")
@timed_response
async def disconnect_instance(instance_id: str):
    get_store().disconnect_service(instance_id)
    return ResponseBuilder.success(
        message="Instance disconnected",
        data={"instance_id": instance_id},
    )


@api_instance_router.post("/{instance_id}/restart")
@timed_response
async def restart_instance(instance_id: str):
    get_store().restart_service(instance_id)
    return ResponseBuilder.success(
        message="Instance restarted",
        data={"instance_id": instance_id},
    )


@api_instance_router.get("/{instance_id}/wait")
@timed_response
async def wait_instance(instance_id: str, timeout_secs: int = Query(10, ge=0)):
    status = get_store().wait_instance_ready(instance_id, timeout_secs)
    return ResponseBuilder.success(message="Instance readiness returned", data=status)


@api_instance_router.get("/{instance_id}/tools")
@timed_response
async def list_instance_tools(instance_id: str):
    tools = get_store().list_tools(instance_id)
    return ResponseBuilder.success(
        message="Instance tools returned",
        data={"instance_id": instance_id, "tools": tools, "total": len(tools)},
    )


@api_instance_router.post("/{instance_id}/tools/{tool_name}/call")
@timed_response
async def call_instance_tool(
    instance_id: str,
    tool_name: str,
    args: Dict[str, Any] = Body(default_factory=dict),
):
    result = get_store().call_tool(instance_id, tool_name, args)
    return ResponseBuilder.success(message="Tool call completed", data={"result": result})


@api_instance_router.get("/{instance_id}/resources")
@timed_response
async def list_instance_resources(instance_id: str):
    resources = get_store().list_resources(instance_id)
    return ResponseBuilder.success(
        message="Instance resources returned",
        data={"resources": resources, "total": len(resources)},
    )


@api_instance_router.get("/{instance_id}/resource-templates")
@timed_response
async def list_instance_resource_templates(instance_id: str):
    templates = get_store().list_resource_templates(instance_id)
    return ResponseBuilder.success(
        message="Instance resource templates returned",
        data={"resource_templates": templates, "total": len(templates)},
    )


@api_instance_router.get("/{instance_id}/resources/read")
@timed_response
async def read_instance_resource(instance_id: str, uri: str = Query(...)):
    resource = get_store().read_resource(instance_id, uri)
    return ResponseBuilder.success(message="Instance resource returned", data=resource)


@api_instance_router.get("/{instance_id}/prompts")
@timed_response
async def list_instance_prompts(instance_id: str):
    prompts = get_store().list_prompts(instance_id)
    return ResponseBuilder.success(
        message="Instance prompts returned",
        data={"prompts": prompts, "total": len(prompts)},
    )


@api_instance_router.post("/{instance_id}/prompts/{prompt_name}")
@timed_response
async def get_instance_prompt(
    instance_id: str,
    prompt_name: str,
    arguments: Dict[str, Any] = Body(default_factory=dict),
):
    prompt = get_store().get_prompt(instance_id, prompt_name, arguments)
    return ResponseBuilder.success(message="Instance prompt returned", data=prompt)


@api_instance_router.get("/{instance_id}/export")
@timed_response
async def export_instance_config(
    instance_id: str,
    format: Optional[str] = Query(None),
):
    config = get_store().export_instance_config(instance_id, format)
    return ResponseBuilder.success(
        message="Instance configuration exported",
        data={"instance_id": instance_id, "config": config},
    )


@api_session_router.get("/snapshot")
@timed_response
async def sessions_export_snapshot():
    snapshot = get_store().export_sessions_snapshot()
    return ResponseBuilder.success(
        message="Session snapshot returned",
        data={"snapshot": snapshot},
    )


@api_session_router.post("/snapshot/import")
@timed_response
async def sessions_import_snapshot(snapshot: Dict[str, Any] = Body(...)):
    report = get_store().import_sessions_snapshot(snapshot)
    return ResponseBuilder.success(
        message="Session snapshot imported",
        data={"report": report},
    )


@api_cache_router.get("/inspect")
@timed_response
async def cache_inspect():
    return ResponseBuilder.success(
        message="Cache inspect returned",
        data=get_store().cache_inspect(),
    )


api_main_router.include_router(api_store_router)
api_main_router.include_router(api_agent_router)
api_main_router.include_router(api_instance_router)
api_main_router.include_router(api_session_router)
api_main_router.include_router(api_cache_router)


__all__ = [
    "api_agent_router",
    "api_cache_router",
    "api_instance_router",
    "api_main_router",
    "api_session_router",
    "api_set_store",
    "api_store_router",
]
