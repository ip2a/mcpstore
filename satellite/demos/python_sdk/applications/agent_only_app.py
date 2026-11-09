"""
Example: A lightweight application that mounts only the Agent-related routes.

Use cases:
1. You only need the Agent-perspective API and don't want the Store/Cache routes.
2. You want minimal dependencies and a one-command uvicorn launch.

Run the example:
uv run uvicorn example.api.agent_only_app:app --host 0.0.0.0 --port 18200 --reload
"""

from example_utils import setup_example_import

# Set up the import path so mcpstore can be imported directly
setup_example_import()

from fastapi import FastAPI
from mcpstore import MCPStore
from mcpstore.api.api_pack import api_agent_router, api_set_store

app = FastAPI(title="MCPStore Agent-Only API", version="0.1.0")

# Initialize MCPStore with the default configuration
store = MCPStore.setup_store()


@app.middleware("http")
async def inject_store(request, call_next):
    """Inject MCPStore into the request context."""
    api_set_store(store)
    return await call_next(request)


@app.get("/health", tags=["System"])
async def health():
    return {"status": "ok", "agent_only": True}


# Mount the Agent routes (keeping the agent_id path parameter)
app.include_router(api_agent_router, prefix="/mcp", tags=["Agent"])


if __name__ == "__main__":
    import uvicorn

    # When running this file directly, boot from the in-memory app object to avoid module path issues.
    uvicorn.run(app, host="0.0.0.0", port=18200, reload=False)
