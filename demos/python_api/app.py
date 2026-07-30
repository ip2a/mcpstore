"""
FastAPI backend demo for the mcpstore PyPI package.

Shows how to wrap sync MCPStore / StoreContext into HTTP endpoints.
mcpstore itself ships no FastAPI dependency; this demo is intentionally minimal.
"""

from __future__ import annotations

from typing import Any, Dict, Optional

from fastapi import FastAPI, HTTPException
from pydantic import BaseModel

from mcpstore import MCPStore, StoreContext


# ===== Boot strap: create one global store =====

store: MCPStore = MCPStore.setup_store()
ctx: StoreContext = store.for_store()


app = FastAPI(title="mcpstore FastAPI demo")


# ===== Schemas =====

class ServiceConfigIn(BaseModel):
    name: str
    config: Dict[str, Any]


class ToolCallIn(BaseModel):
    args: Optional[Dict[str, Any]] = None


# ===== Routes =====

@app.get("/health")
def health():
    return {"ok": True}


@app.get("/config")
def show_config():
    return ctx.show_config()


@app.post("/services")
def add_service(payload: ServiceConfigIn):
    """Add a service to the store scope via add_service_config(name, config)."""
    new_ctx = ctx.add_service_config(payload.name, payload.config)
    # ctx is replaced because add_service_config returns a new StoreContext (chain)
    globals()["ctx"] = new_ctx
    return {"ok": True, "name": payload.name}


@app.get("/services")
def list_services():
    return [s.info() for s in ctx.list_services()]


@app.get("/services/{service_name}")
def find_service(service_name: str):
    try:
        svc = ctx.find_service(service_name=service_name)
    except Exception as exc:
        raise HTTPException(status_code=404, detail=str(exc))
    return svc.info()


@app.delete("/services/{service_name}")
def remove_service(service_name: str):
    ok = ctx.remove_service(service_name=service_name)
    return {"ok": ok}


@app.post("/services/{service_name}/restart")
def restart_service(service_name: str):
    new_ctx = ctx.restart_service(service_name=service_name)
    globals()["ctx"] = new_ctx
    return {"ok": True}


@app.get("/tools")
def list_tools():
    return [t.info() for t in ctx.list_tools()]


@app.get("/tools/{tool_name}")
def find_tool(tool_name: str):
    try:
        tool = ctx.find_tool(tool_name)
    except Exception as exc:
        raise HTTPException(status_code=404, detail=str(exc))
    return tool.info()


@app.post("/tools/{tool_name}/call")
def call_tool(tool_name: str, payload: ToolCallIn):
    try:
        return ctx.call_tool(tool_name, payload.args)
    except Exception as exc:
        raise HTTPException(status_code=500, detail=str(exc))
