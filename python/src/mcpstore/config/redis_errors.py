"""Redis compatibility errors for the Rust-backed cache boundary."""

from __future__ import annotations

from typing import Any, Dict


class RedisConnectionFailure(Exception):
    def __init__(
        self,
        message: str,
        connection_info: Dict[str, Any],
        original_error: Exception,
        troubleshooting_steps: list[str],
    ):
        self.message = message
        self.connection_info = connection_info
        self.original_error = original_error
        self.troubleshooting_steps = troubleshooting_steps
        super().__init__(self._build_error_message())

    def _build_error_message(self) -> str:
        lines = ["Redis Connection Failure", f"Error: {self.message}", "Connection Details:"]
        lines.extend(f"  {key}: {value}" for key, value in self.connection_info.items())
        lines.append(f"Original Error: {type(self.original_error).__name__}: {self.original_error}")
        lines.append("Troubleshooting Steps:")
        lines.extend(f"  {index}. {step}" for index, step in enumerate(self.troubleshooting_steps, 1))
        return "\n".join(lines)


def mask_password_in_url(url: str) -> str:
    if not url or "@" not in url or "://" not in url:
        return url
    try:
        protocol, rest = url.split("://", 1)
        auth_part, host_part = rest.split("@", 1)
        if ":" not in auth_part:
            return url
        user, _password = auth_part.split(":", 1)
        return f"{protocol}://{user}:***@{host_part}"
    except ValueError:
        return url


def get_connection_info(config: Any) -> Dict[str, Any]:
    info: Dict[str, Any] = {}
    url = getattr(config, "url", None)
    if url:
        info["url"] = mask_password_in_url(url)
    else:
        info["host"] = getattr(config, "host", None) or "localhost"
        info["port"] = getattr(config, "port", None) or 6379
        info["db"] = getattr(config, "db", None) or 0
        if getattr(config, "password", None):
            info["password"] = "***"
    info["namespace"] = getattr(config, "namespace", None) or "mcpstore"
    info["max_connections"] = getattr(config, "max_connections", 50)
    info["socket_timeout"] = f"{getattr(config, 'socket_timeout', 5.0)}s"
    info["socket_connect_timeout"] = f"{getattr(config, 'socket_connect_timeout', 5.0)}s"
    return info


def handle_redis_connection_error(error: Exception, config: Any) -> RedisConnectionFailure:
    return RedisConnectionFailure(
        message=f"Redis connection is managed by the Rust cache backend: {error}",
        connection_info=get_connection_info(config),
        original_error=error,
        troubleshooting_steps=[
            "Configure Redis through MCPStore.setup_store(..., cache=RedisConfig(...)).",
            "Use store.switch_cache(...) for runtime backend migration.",
            "Do not pass Python Redis client instances when cross-process sharing is required.",
        ],
    )


async def test_redis_connection(config: Any) -> None:
    raise handle_redis_connection_error(
        NotImplementedError("Python Redis connection tests were removed from the Rust-backed runtime"),
        config,
    )
