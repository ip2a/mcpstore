"""
Example: Minimal FastAPI endpoints that call the SDK directly (no shared router involved).

Use cases:
1. You just want to expose a few endpoints quickly, calling the SDK directly
   without loading the full API routes.
2. It illustrates the direct SDK usage in its simplest form.

Run the example:
uv run uvicorn example.api.minimal_sdk_app:app --host 0.0.0.0 --port 18202 --reload
"""

from example_utils import setup_example_import

# Set up the import path so mcpstore can be imported directly
setup_example_import()

from fastapi import FastAPI, HTTPException
from mcpstore import MCPStore

app = FastAPI(title="Minimal SDK API", version="0.1.0")

# Initialize MCPStore
store = MCPStore.setup_store()


@app.get("/health", tags=["System"])
async def health():
    return {"status": "ok", "minimal_sdk": True}


@app.get("/sdk/agent/{agent_id}/service_info/{service_name}", tags=["SDK"])
async def sdk_service_info(agent_id: str, service_name: str):
    """
    Minimal example: fetch service info directly via the SDK (Agent perspective).
    For demonstration only; the full response wrapper is not included.
    """
    ctx = store.for_agent(agent_id)
    info = await ctx.bridge_execute(ctx.service_info_async(service_name))
    if not info or not getattr(info, "service", None):
        raise HTTPException(status_code=404, detail="service not found")
    svc = info.service
    return {
        "name": svc.name,
        "status": getattr(svc.status, "value", str(svc.status)),
        "type": getattr(svc.transport_type, "value", "unknown"),
        "tool_count": getattr(svc, "tool_count", 0),
    }


if __name__ == "__main__":
    import uvicorn

    uvicorn.run(app, host="0.0.0.0", port=18202, reload=False)
