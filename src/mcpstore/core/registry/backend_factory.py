from __future__ import annotations

from typing import Any, Dict, Optional
import logging


from .cache_backend import CacheBackend
from .memory_backend import MemoryCacheBackend
from .redis_backend import RedisCacheBackend
from .key_builder import KeyBuilder
logger = logging.getLogger(__name__)

from .normalizer import DefaultToolNormalizer


def make_cache_backend(config: Optional[Dict[str, Any]], registry) -> CacheBackend:
    """Factory for cache backends.

    Example config structure (for future use):
    {
      "backend": "memory" | "redis",
      "redis": {
        "namespace": "mcpstore",
        "dataspace": "default",
        "client": None  # A pre-initialized redis-like client (optional at this stage)
      }
    }
    If config is None or backend is not "redis", defaults to MemoryCacheBackend.
    """
    if not config or config.get("backend") != "redis":
        return MemoryCacheBackend(registry)

    redis_cfg = config.get("redis", {}) if config else {}

    # Determine client: prefer explicitly provided, else try building from URL
    client = redis_cfg.get("client")
    if client is None:
        url = redis_cfg.get("url")
        if url:
            try:
                import redis as _redis  # optional dependency, may be absent
                kwargs: Dict[str, Any] = {}
                if redis_cfg.get("password") is not None:
                    kwargs["password"] = redis_cfg.get("password")
                if redis_cfg.get("socket_timeout") is not None:
                    kwargs["socket_timeout"] = redis_cfg.get("socket_timeout")
                if redis_cfg.get("healthcheck_interval") is not None:
                    kwargs["health_check_interval"] = redis_cfg.get("healthcheck_interval")
                client = _redis.Redis.from_url(url, **kwargs)
                redis_cfg["client"] = client
            except Exception as e:
                logger.debug(f"Redis client creation skipped/fallback (import/connect issue): {e}")
                client = None
    # Enforce fail-fast: require a usable client
    if redis_cfg.get("client") is None:
        raise RuntimeError("Redis backend requested but no usable client is available")

    # Build Redis backend with attached client
    kb = KeyBuilder(
        namespace=redis_cfg.get("namespace", "default"),
        dataspace=redis_cfg.get("dataspace", "default"),
    )
    backend = RedisCacheBackend(key_builder=kb, normalizer=DefaultToolNormalizer())
    # Attach and validate connectivity (if ping exists)
    backend.attach_client(redis_cfg["client"])  # type: ignore[index]
    ping = getattr(redis_cfg["client"], "ping", None)
    if callable(ping):
        ping()  # raise on failure
    return backend

