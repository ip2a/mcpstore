"""
Utility helpers for creating temporary FastMCP clients using async context managers.
These helpers centralize config processing and ensure proper lifecycle (async with).
"""

from __future__ import annotations

from contextlib import asynccontextmanager
from typing import AsyncIterator, Dict

from fastmcp import Client

from mcpstore.core.configuration.config_processor import ConfigProcessor


@asynccontextmanager
async def temp_client_for_service(service_name: str, service_config: Dict) -> AsyncIterator[Client]:
    """Create a temporary FastMCP Client for a single service and yield it inside an async-with.

    - Processes user service_config via ConfigProcessor to build a valid FastMCP client config
    - Ensures the client is properly connected within an async-with block
    - Closes the client automatically on exit
    """
    # Build a minimal fastmcp config for this one service
    user_config = {"mcpServers": {service_name: service_config or {}}}
    fastmcp_config = ConfigProcessor.process_user_config_for_fastmcp(user_config)

    # If the service was removed by the processor due to validation errors, raise
    if service_name not in fastmcp_config.get("mcpServers", {}):
        raise ValueError(f"Invalid service configuration for {service_name}")

    client = Client(fastmcp_config)
    try:
        async with client:
            yield client
    finally:
        try:
            await client.close()
        except Exception:
            pass

