"""FastAPI projection of the official MCPStore HTTP subset."""

from __future__ import annotations

import os
from contextlib import asynccontextmanager
from typing import Any

from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware

from config import build_store
from errors import register_exception_handlers
from routers import agents, health, scopes, service_scopes, services, tools


def create_app(store: Any | None = None) -> FastAPI:
    @asynccontextmanager
    async def lifespan(api: FastAPI):
        api.state.store = store if store is not None else build_store()
        yield

    api = FastAPI(title="MCPStore Python API", lifespan=lifespan)

    cors_origins = os.getenv(
        "MCP_CORS_ORIGINS",
        "http://localhost:3000,http://127.0.0.1:3000",
    )
    api.add_middleware(
        CORSMiddleware,
        allow_origins=[origin.strip() for origin in cors_origins.split(",") if origin.strip()],
        allow_credentials=True,
        allow_methods=["*"],
        allow_headers=["*"],
    )

    register_exception_handlers(api)

    api.include_router(health.router)
    api.include_router(agents.router)
    api.include_router(scopes.router)
    api.include_router(services.router)
    api.include_router(tools.router)
    api.include_router(service_scopes.router)

    return api


app = create_app()


if __name__ == "__main__":
    import uvicorn

    uvicorn.run(
        app,
        host=os.getenv("MCP_API_HOST", "0.0.0.0"),
        port=int(os.getenv("MCP_API_PORT", "1820")),
        log_level=os.getenv("UVICORN_LOG_LEVEL", "debug").lower(),
    )
