"""Compatibility boundary for legacy Python Redis health checks."""

from __future__ import annotations

from typing import Any, Optional


class RedisHealthCheck:
    def __init__(self, config: Any, client: Any):
        self.config = config
        self.client = client
        self.task: Optional[Any] = None

    def start(self) -> None:
        raise NotImplementedError(
            "Python Redis health checks were removed from the Rust-backed runtime. "
            "Configure cache health through MCPStore.setup_store(..., cache=...) so Rust owns shared backend state."
        )

    async def stop(self) -> None:
        self.task = None


def start_health_check(config: Any, client: Any) -> Optional[RedisHealthCheck]:
    _ = client
    interval = getattr(config, "health_check_interval", 0)
    if interval <= 0:
        return None
    health_check = RedisHealthCheck(config, client)
    health_check.start()
    return health_check
