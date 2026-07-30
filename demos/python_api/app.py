"""
FastAPI backend demo for the mcpstore PyPI package.

Shows how to wrap the sync MCPStore / StoreContext into HTTP endpoints
with a uniform response envelope. mcpstore itself ships no FastAPI
dependency; this demo is intentionally minimal.
"""

from __future__ import annotations

from typing import Any, Dict, Optional

from fastapi import FastAPI, HTTPException
from pydantic import BaseModel

from mcpstore import MCPServerConfig, MCPStore, StoreContext


# ===== Bootstrap: one shared store with a mutable scope holder =====

class AppState:
    """Hold the store and the current StoreContext across requests.

    StoreContext is immutable: chain methods return a new context, so we
    keep the latest one here rather than reaching for module globals.
    """

    def __init__(self) -> None:
        self.store = MCPStore.setup_store()
        self.ctx: StoreContext = self.store.for_store()

    def replace(self, new_ctx: StoreContext) -> None:
        self.ctx = new_ctx


state = AppState()
app = FastAPI(title="mcpstore FastAPI demo")


# ===== Schemas =====

class ApiResponse(BaseModel):
    """Uniform HTTP response envelope (demo layer only)."""

    ok: bool = True
    data: Optional[Any] = None
    error: Optional[str] = None
    message: str = "ok"


class ToolCallRequest(BaseModel):
    args: Dict[str, Any] = {}


# ===== Routes =====

@app.get("/health")
def health() -> ApiResponse:
    return ApiResponse(data={"status": "up"})


@app.get("/config")
def show_config() -> ApiResponse:
    return ApiResponse(data=state.ctx.show_config())


@app.post("/services")
def add_services(config: MCPServerConfig) -> ApiResponse:
    state.replace(state.ctx.add_service(config.model_dump()))
    return ApiResponse(data={"services": list(config.mcpServers.keys())})


@app.get("/services")
def list_services() -> ApiResponse:
    return ApiResponse(data=[s.info() for s in state.ctx.list_services()])


@app.get("/services/{service_name}")
def find_service(service_name: str) -> ApiResponse:
    try:
        info = state.ctx.find_service(service_name=service_name).info()
    except Exception as exc:
        raise HTTPException(status_code=404, detail=str(exc))
    return ApiResponse(data=info)


@app.delete("/services/{service_name}")
def remove_service(service_name: str) -> ApiResponse:
    removed = state.ctx.remove_service(service_name=service_name)
    return ApiResponse(data={"removed": removed})


@app.post("/services/{service_name}/restart")
def restart_service(service_name: str) -> ApiResponse:
    state.replace(state.ctx.restart_service(service_name=service_name))
    return ApiResponse()


@app.get("/tools")
def list_tools() -> ApiResponse:
    return ApiResponse(data=[t.info() for t in state.ctx.list_tools()])


@app.get("/tools/{tool_name}")
def find_tool(tool_name: str) -> ApiResponse:
    try:
        info = state.ctx.find_tool(tool_name).info()
    except Exception as exc:
        raise HTTPException(status_code=404, detail=str(exc))
    return ApiResponse(data=info)


@app.post("/tools/{tool_name}/call")
def call_tool(tool_name: str, request: ToolCallRequest) -> ApiResponse:
    try:
        result = state.ctx.call_tool(tool_name, request.args)
    except Exception as exc:
        raise HTTPException(status_code=500, detail=str(exc))
    return ApiResponse(data=result)
