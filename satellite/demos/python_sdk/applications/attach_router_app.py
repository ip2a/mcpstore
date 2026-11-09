"""
Example: Mount the full set of MCPStore routes inside your own FastAPI application.

Use cases:
1. You already have a FastAPI app and want to reuse all MCPStore APIs (store/agent/cache).
2. You need a custom prefix, or want to keep adding custom endpoints to the same app.

Run the example:
uv run uvicorn example.api.attach_router_app:app --host 0.0.0.0 --port 18201 --reload
"""

from example_utils import setup_example_import

# Set up the import path so mcpstore can be imported directly
setup_example_import()

from fastapi import FastAPI
from mcpstore import MCPStore
from mcpstore.api.api_pack import (
    api_main_router,
    api_set_store,
)
from fastapi.exceptions import RequestValidationError
from starlette.exceptions import HTTPException as StarletteHTTPException
from mcpstore.api.api_exceptions import (
    validation_exception_handler,
    http_exception_handler,
    general_exception_handler,
)

app = FastAPI(title="MyApp with MCPStore API", version="0.1.0")

# Initialize MCPStore with the default configuration
store = MCPStore.setup_store()


@app.middleware("http")
async def inject_store(request, call_next):
    """Inject MCPStore into the request context."""
    api_set_store(store)
    return await call_next(request)


@app.get("/health", tags=["System"])
async def health():
    return {"status": "ok", "with_mcpstore": True}


# Mount the main MCPStore router; adjust the prefix as needed
app.include_router(api_main_router, prefix="/mcp", tags=["MCPStore"])

# Register the unified exception handlers (optional)
app.add_exception_handler(RequestValidationError, validation_exception_handler)
app.add_exception_handler(StarletteHTTPException, http_exception_handler)
app.add_exception_handler(Exception, general_exception_handler)


if __name__ == "__main__":
    import uvicorn

    uvicorn.run(app, host="0.0.0.0", port=18201, reload=False)
