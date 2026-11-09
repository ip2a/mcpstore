from __future__ import annotations

import logging
import os
from pathlib import Path
from typing import Any

from dotenv import load_dotenv

from mcpstore import MCPStore, RedisConfig


load_dotenv(Path(__file__).with_name(".env"))

logger = logging.getLogger("mcpstore.python_api")


def build_store() -> Any:
    source = RedisConfig(
        host=os.environ["MCP_REDIS_HOST"],
        port=int(os.getenv("MCP_REDIS_PORT", "6379")),
        password=os.getenv("MCP_REDIS_PASSWORD"),
        db=int(os.getenv("MCP_REDIS_DB", "0")),
        namespace=os.getenv("MCP_NAMESPACE"),
    )
    log_level = os.getenv("MCP_LOG_LEVEL", "DEBUG")
    logger.info(
        "Initializing MCPStore: host=%s port=%s db=%s namespace=%s mode=control_plane log_level=%s",
        os.environ["MCP_REDIS_HOST"],
        os.getenv("MCP_REDIS_PORT", "6379"),
        os.getenv("MCP_REDIS_DB", "0"),
        os.getenv("MCP_NAMESPACE"),
        log_level,
    )
    store = MCPStore.setup_store(
        source=source, mode="control_plane", debug=log_level
    )
    logger.info("MCPStore initialized: type=%s", type(store))
    return store
