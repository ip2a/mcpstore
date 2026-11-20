"""
Mini FastAPI service with hand-written routes (the official aggregated router is not used).

Features:
- Mounts only custom, lightweight Store/Agent routes.
- Reuses the official SDK and utilities (MCPStore, ResponseBuilder, dependency injection).
- Clean layout: app.py + routes.

Run the example:
uv run uvicorn example.api.mini.app:app --host 0.0.0.0 --port 18210 --reload
"""

from example.api.example_utils import setup_example_import

# Ensure mcpstore can be imported
setup_example_import()

from fastapi import FastAPI
from mcpstore import MCPStore
from mcpstore.api.api_pack import api_set_store

from .routes.store import router as store_router
from .routes.agent import router as agent_router
from .routes.kv import router as kv_router

app = FastAPI(title="MCPStore Mini API", version="0.1.0")

# Initialize MCPStore with the default configuration
store = MCPStore.setup_store()


@app.middleware("http")
async def inject_store(request, call_next):
    """Inject MCPStore into the request context."""
    api_set_store(store)
    return await call_next(request)


@app.get("/health", tags=["System"])
async def health():
    return {"status": "ok", "mini_api": True}


# Mount the custom routes (the official aggregated router is not used)
app.include_router(store_router, prefix="/mini")
app.include_router(agent_router, prefix="/mini")
app.include_router(kv_router, prefix="/mini")


if __name__ == "__main__":
    import uvicorn

    uvicorn.run(app, host="0.0.0.0", port=18210, reload=False)
