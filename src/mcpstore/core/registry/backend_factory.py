from __future__ import annotations

import logging
import os
from hashlib import sha1
from typing import Any, Dict, Optional

from .cache_backend import CacheBackend
from .key_builder import KeyBuilder
from .memory_backend import MemoryCacheBackend
from .redis_backend import RedisCacheBackend

logger = logging.getLogger(__name__)

from .normalizer import DefaultToolNormalizer


def _auto_generate_namespace(mcp_json_path: Optional[str]) -> str:
    """Auto-generate namespace from mcp.json path (5-char hash).

    Args:
        mcp_json_path: Path to mcp.json file

    Returns:
        5-character hash of absolute path, or "mcpstore" if path is invalid
    """
    if not mcp_json_path or mcp_json_path == ":memory:":
        return "mcpstore"

    try:
        abs_path = os.path.abspath(mcp_json_path)
        return sha1(abs_path.encode("utf-8")).hexdigest()[:5]
    except Exception as e:
        logger.warning(f"Failed to generate namespace from path: {e}")
        return "mcpstore"


def make_cache_backend(config: Optional[Dict[str, Any]], registry) -> CacheBackend:
    """Factory for cache backends with Fail-Fast error handling.

    Config structure:
    {
      "backend": "memory" | "redis",
      "redis": {
        "url": "redis://host:port/db",        # Required
        "password": "xxx",                     # Optional
        "namespace": "myapp",                  # Optional, auto-generated if not provided
        "socket_timeout": 2.0,                 # Optional, default 2.0
        "healthcheck_interval": 30,            # Optional, default 30
        "max_connections": 50,                 # Optional, default 50
        "_mcp_json_path": "/path/to/mcp.json" # Internal, for namespace auto-gen
      }
    }

    Behavior:
    - If backend != "redis": Returns MemoryCacheBackend
    - If Redis connection fails: Raises RuntimeError with clear error message (Fail-Fast)
    - No graceful degradation to Memory backend

    Args:
        config: Backend configuration dictionary
        registry: ServiceRegistry instance (for fallback)

    Returns:
        CacheBackend: MemoryCacheBackend or RedisCacheBackend

    Raises:
        RuntimeError: If Redis backend is requested but connection fails
    """
    if not config or config.get("backend") != "redis":
        return MemoryCacheBackend(registry)

    redis_cfg = config.get("redis", {})

    # 1. Check if redis package is installed
    try:
        import redis as _redis
    except ImportError as e:
        raise RuntimeError(
            "Redis backend requested but 'redis' package is not installed. "
            "Install with: pip install mcpstore[redis]"
        ) from e

    # 2. Validate URL
    url = redis_cfg.get("url")
    if not url:
        raise RuntimeError(
            "Redis backend requested but 'url' is not provided. "
            "Example: {'cache': {'type': 'redis', 'url': 'redis://localhost:6379/0'}}"
        )

    # 3. Build connection pool with reasonable defaults
    try:
        pool_kwargs = {
            "socket_timeout": redis_cfg.get("socket_timeout", 2.0),
            "socket_connect_timeout": redis_cfg.get("socket_connect_timeout", 2.0),
            "health_check_interval": redis_cfg.get("healthcheck_interval", 30),
            "max_connections": redis_cfg.get("max_connections", 50),
        }

        # Add password if provided
        if redis_cfg.get("password"):
            pool_kwargs["password"] = redis_cfg["password"]

        # Create connection pool
        pool = _redis.ConnectionPool.from_url(url, **pool_kwargs)
        client = _redis.Redis(connection_pool=pool, decode_responses=False)

        logger.debug(f"Redis connection pool created: {pool_kwargs}")

    except Exception as e:
        raise RuntimeError(
            f"Failed to create Redis connection pool: {e}. "
            f"Please check: 1) Redis server is running, 2) URL is correct, 3) Network is accessible."
        ) from e

    # 4. Validate connection immediately
    try:
        client.ping()
        logger.info(f"Redis connection successful: {url}")
    except Exception as e:
        raise RuntimeError(
            f"Redis connection failed (ping failed): {e}. "
            f"Please verify Redis server is running and accessible at {url}"
        ) from e

    # 5. Determine namespace: user-provided or auto-generated
    namespace = redis_cfg.get("namespace")
    if not namespace:
        mcp_json_path = redis_cfg.get("_mcp_json_path")
        namespace = _auto_generate_namespace(mcp_json_path)
        logger.info(f"Auto-generated namespace: {namespace} (from {mcp_json_path})")
    else:
        logger.info(f"Using provided namespace: {namespace}")

    # 6. Build Redis backend
    kb = KeyBuilder(namespace=namespace)
    backend = RedisCacheBackend(key_builder=kb, normalizer=DefaultToolNormalizer())
    backend.attach_client(client)

    # 7. Final health check
    if not backend.health_check():
        raise RuntimeError(
            f"Redis backend health check failed after successful connection. "
            f"This should not happen - please report this issue."
        )

    logger.info(f"Redis backend initialized: mcpstore:{namespace}:...")
    return backend

