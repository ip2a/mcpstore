"""
Default configuration values for MCPStore.

This module contains all the default configuration values that are used
when TOML configuration is not provided or contains invalid values.
"""

from dataclasses import dataclass
from typing import Dict, Any, Optional


@dataclass
class ServerConfigDefaults:
    """Default server configuration."""
    host: str = "0.0.0.0"
    port: int = 18200
    reload: bool = False
    auto_open_browser: bool = False
    show_startup_info: bool = True


@dataclass
class HealthCheckConfigDefaults:
    """Default health check configuration."""
    enabled: bool = True
    warning_failure_threshold: int = 1
    reconnecting_failure_threshold: int = 2
    max_reconnect_attempts: int = 10
    base_reconnect_delay: float = 1.0
    max_reconnect_delay: float = 60.0
    long_retry_interval: float = 300.0
    normal_heartbeat_interval: float = 30.0
    warning_heartbeat_interval: float = 10.0
    health_check_ping_timeout: float = 10.0
    initialization_timeout: float = 300.0
    disconnection_timeout: float = 10.0


@dataclass
class ServiceLifecycleConfigDefaults:
    """Default service lifecycle timeouts and lifecycle-related settings.

    These values complement HealthCheckConfigDefaults by providing higher-level
    lifecycle timeouts and behavior controls (initialization/termination/shutdown
    and restart behavior). They are used by ServiceLifecycleConfig in both
    config_dataclasses.py and core.lifecycle.config.
    """
    # Lifecycle timeouts (seconds)
    initialization_timeout: float = 300.0
    termination_timeout: float = 60.0
    shutdown_timeout: float = 30.0

    # Retry and restart behavior
    restart_delay_seconds: float = 5.0
    max_restart_attempts: int = 3

    # Logging and monitoring toggles
    enable_detailed_logging: bool = True
    collect_startup_metrics: bool = True
    collect_runtime_metrics: bool = True
    collect_shutdown_metrics: bool = True


@dataclass
class ContentUpdateConfigDefaults:
    """Default content update configuration."""
    tools_update_interval: float = 300.0      # 5 minutes
    resources_update_interval: float = 600.0  # 10 minutes
    prompts_update_interval: float = 600.0    # 10 minutes
    max_concurrent_updates: int = 3
    update_timeout: float = 30.0              # 30 seconds
    max_consecutive_failures: int = 3
    failure_backoff_multiplier: float = 2.0


@dataclass
class MonitoringConfigDefaults:
    """Default monitoring configuration."""
    health_check_seconds: int = 30
    tools_update_hours: float = 2.0
    reconnection_seconds: int = 60
    cleanup_hours: float = 24.0
    enable_tools_update: bool = True
    enable_reconnection: bool = True
    update_tools_on_reconnection: bool = True
    detect_tools_changes: bool = False
    local_service_ping_timeout: int = 3
    remote_service_ping_timeout: int = 5
    startup_wait_time: int = 2
    healthy_response_threshold: float = 1.0
    warning_response_threshold: float = 3.0
    slow_response_threshold: float = 10.0
    enable_adaptive_timeout: bool = True
    adaptive_timeout_multiplier: float = 2.0
    response_time_history_size: int = 10


@dataclass
class CacheMemoryConfigDefaults:
    """Default memory cache configuration."""
    timeout: float = 2.0
    retry_attempts: int = 3
    health_check: bool = True
    max_size: Optional[int] = None
    cleanup_interval: int = 300


@dataclass
class CacheRedisConfigDefaults:
    """Default Redis cache configuration (excluding sensitive info)."""
    timeout: float = 2.0
    retry_attempts: int = 3
    health_check: bool = True
    max_connections: int = 50
    retry_on_timeout: bool = True
    socket_keepalive: bool = True
    socket_connect_timeout: float = 5.0
    socket_timeout: float = 5.0
    health_check_interval: int = 30


@dataclass
class StandaloneConfigDefaults:
    """Default standalone configuration."""
    heartbeat_interval_seconds: float = 30.0
    http_timeout_seconds: float = 10.0
    reconnection_interval_seconds: float = 60.0
    cleanup_interval_seconds: float = 300.0
    default_transport: str = "stdio"
    log_level: str = "INFO"
    log_format: str = "json"
    enable_debug: bool = False


@dataclass
class LoggingConfigDefaults:
    """Default logging configuration."""
    level: str = "INFO"
    enable_debug: bool = False
    format: str = "json"


@dataclass
class WrapperConfigDefaults:
    """Default wrapper configuration."""
    DEFAULT_MAX_ITEM_SIZE: int = 1048576  # 1MB
    DEFAULT_COMPRESSION_THRESHOLD: int = 1024  # 1KB


@dataclass
class SyncConfigDefaults:
    """Default sync configuration."""
    debounce_delay: float = 1.0
    min_sync_interval: float = 5.0


@dataclass
class TransactionConfigDefaults:
    """Default transaction configuration."""
    timeout: float = 30.0


@dataclass
class APIConfigDefaults:
    """Default API configuration."""
    enable_cors: bool = True
    cors_origins: list = None
    rate_limit_enabled: bool = False
    rate_limit_requests: int = 100
    rate_limit_window: int = 60

    def __post_init__(self):
        if self.cors_origins is None:
            self.cors_origins = ["*"]


@dataclass
class ToolSetConfigDefaults:
    """Default tool set configuration."""
    enable_tool_set: bool = True
    cache_ttl_seconds: int = 3600
    max_tools_per_service: int = 1000


def get_all_defaults() -> Dict[str, Dict[str, Any]]:
    """
    Get all default configuration values as a dictionary.

    Returns:
        Dictionary containing all default configurations grouped by section
    """
    server = ServerConfigDefaults()
    health_check = HealthCheckConfigDefaults()
    service_lifecycle = ServiceLifecycleConfigDefaults()
    content_update = ContentUpdateConfigDefaults()
    monitoring = MonitoringConfigDefaults()
    cache_memory = CacheMemoryConfigDefaults()
    cache_redis = CacheRedisConfigDefaults()
    standalone = StandaloneConfigDefaults()
    logging = LoggingConfigDefaults()
    wrapper = WrapperConfigDefaults()
    sync = SyncConfigDefaults()
    transaction = TransactionConfigDefaults()
    api = APIConfigDefaults()
    tool_set = ToolSetConfigDefaults()

    return {
        "server": {
            "host": server.host,
            "port": server.port,
            "reload": server.reload,
            "auto_open_browser": server.auto_open_browser,
            "show_startup_info": server.show_startup_info,
        },
        "cache": {
            "type": "memory",  # Default cache type
        },
        "cache.memory": {
            "timeout": cache_memory.timeout,
            "retry_attempts": cache_memory.retry_attempts,
            "health_check": cache_memory.health_check,
            "max_size": cache_memory.max_size,
            "cleanup_interval": cache_memory.cleanup_interval,
        },
        "cache.redis": {
            "timeout": cache_redis.timeout,
            "retry_attempts": cache_redis.retry_attempts,
            "health_check": cache_redis.health_check,
            "max_connections": cache_redis.max_connections,
            "retry_on_timeout": cache_redis.retry_on_timeout,
            "socket_keepalive": cache_redis.socket_keepalive,
            "socket_connect_timeout": cache_redis.socket_connect_timeout,
            "socket_timeout": cache_redis.socket_timeout,
            "health_check_interval": cache_redis.health_check_interval,
        },
        "health_check": {
            "enabled": health_check.enabled,
            "warning_failure_threshold": health_check.warning_failure_threshold,
            "reconnecting_failure_threshold": health_check.reconnecting_failure_threshold,
            "max_reconnect_attempts": health_check.max_reconnect_attempts,
            "base_reconnect_delay": health_check.base_reconnect_delay,
            "max_reconnect_delay": health_check.max_reconnect_delay,
            "long_retry_interval": health_check.long_retry_interval,
            "normal_heartbeat_interval": health_check.normal_heartbeat_interval,
            "warning_heartbeat_interval": health_check.warning_heartbeat_interval,
            "health_check_ping_timeout": health_check.health_check_ping_timeout,
            "initialization_timeout": health_check.initialization_timeout,
            "disconnection_timeout": health_check.disconnection_timeout,
        },
        "content_update": {
            "tools_update_interval": content_update.tools_update_interval,
            "resources_update_interval": content_update.resources_update_interval,
            "prompts_update_interval": content_update.prompts_update_interval,
            "max_concurrent_updates": content_update.max_concurrent_updates,
            "update_timeout": content_update.update_timeout,
            "max_consecutive_failures": content_update.max_consecutive_failures,
            "failure_backoff_multiplier": content_update.failure_backoff_multiplier,
        },
        "monitoring": {
            "health_check_seconds": monitoring.health_check_seconds,
            "tools_update_hours": monitoring.tools_update_hours,
            "reconnection_seconds": monitoring.reconnection_seconds,
            "cleanup_hours": monitoring.cleanup_hours,
            "enable_tools_update": monitoring.enable_tools_update,
            "enable_reconnection": monitoring.enable_reconnection,
            "update_tools_on_reconnection": monitoring.update_tools_on_reconnection,
            "detect_tools_changes": monitoring.detect_tools_changes,
            "local_service_ping_timeout": monitoring.local_service_ping_timeout,
            "remote_service_ping_timeout": monitoring.remote_service_ping_timeout,
            "startup_wait_time": monitoring.startup_wait_time,
            "healthy_response_threshold": monitoring.healthy_response_threshold,
            "warning_response_threshold": monitoring.warning_response_threshold,
            "slow_response_threshold": monitoring.slow_response_threshold,
            "enable_adaptive_timeout": monitoring.enable_adaptive_timeout,
            "adaptive_timeout_multiplier": monitoring.adaptive_timeout_multiplier,
            "response_time_history_size": monitoring.response_time_history_size,
        },
        "standalone": {
            "heartbeat_interval_seconds": standalone.heartbeat_interval_seconds,
            "http_timeout_seconds": standalone.http_timeout_seconds,
            "reconnection_interval_seconds": standalone.reconnection_interval_seconds,
            "cleanup_interval_seconds": standalone.cleanup_interval_seconds,
            "default_transport": standalone.default_transport,
            "log_level": standalone.log_level,
            "log_format": standalone.log_format,
            "enable_debug": standalone.enable_debug,
        },
        "logging": {
            "level": logging.level,
            "enable_debug": logging.enable_debug,
            "format": logging.format,
        },
        "wrapper": {
            "DEFAULT_MAX_ITEM_SIZE": wrapper.DEFAULT_MAX_ITEM_SIZE,
            "DEFAULT_COMPRESSION_THRESHOLD": wrapper.DEFAULT_COMPRESSION_THRESHOLD,
        },
        "sync": {
            "debounce_delay": sync.debounce_delay,
            "min_sync_interval": sync.min_sync_interval,
        },
        "transaction": {
            "timeout": transaction.timeout,
        },
        "api": {
            "enable_cors": api.enable_cors,
            "cors_origins": api.cors_origins,
            "rate_limit_enabled": api.rate_limit_enabled,
            "rate_limit_requests": api.rate_limit_requests,
            "rate_limit_window": api.rate_limit_window,
        },
        "tool_set": {
            "enable_tool_set": tool_set.enable_tool_set,
            "cache_ttl_seconds": tool_set.cache_ttl_seconds,
            "max_tools_per_service": tool_set.max_tools_per_service,
        },
    }
