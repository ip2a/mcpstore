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
    """Default health check configuration (new model, no backward compatibility)."""
    enabled: bool = True
    # 探针周期与超时
    startup_interval: float = 1.0
    startup_timeout: float = 30.0
    startup_hard_timeout: float = 120.0
    readiness_interval: float = 5.0
    readiness_success_threshold: int = 1
    readiness_failure_threshold: int = 1
    liveness_interval: float = 10.0
    liveness_failure_threshold: int = 2
    ping_timeout_http: float = 20.0
    ping_timeout_sse: float = 20.0
    ping_timeout_stdio: float = 40.0
    warning_ping_timeout: float = 30.0  # 在 degraded/circuit_open/half_open 放宽
    # 滑动窗口判定
    window_size: int = 20          # 样本窗大小
    window_min_calls: int = 5
    error_rate_threshold: float = 0.3
    latency_p95_warn: float = 2.0
    latency_p99_critical: float = 5.0
    # 退避与熔断
    max_reconnect_attempts: int = 10
    backoff_base: float = 1.0
    backoff_max: float = 60.0
    backoff_jitter: float = 0.1
    backoff_max_duration: float = 600.0
    # 半开试探
    half_open_max_calls: int = 3
    half_open_success_rate_threshold: float = 0.6
    # 硬超时
    reconnect_hard_timeout: float = 900.0
    # 租约
    lease_ttl: float = 60.0
    lease_renew_interval: float = 20.0


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
    tools_update_hours: float = 2.0
    reconnection_seconds: int = 60
    cleanup_hours: float = 24.0
    enable_tools_update: bool = True
    enable_reconnection: bool = True
    update_tools_on_reconnection: bool = True
    detect_tools_changes: bool = False
    local_service_ping_timeout: int = 3
    remote_service_ping_timeout: int = 5
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

    Note: Cache, wrapper, sync, transaction, api, tool_set, and logging configurations
    are removed as they are not managed via TOML configuration files.

    Returns:
        Dictionary containing all default configurations grouped by section
    """
    server = ServerConfigDefaults()
    health_check = HealthCheckConfigDefaults()
    service_lifecycle = ServiceLifecycleConfigDefaults()
    content_update = ContentUpdateConfigDefaults()
    monitoring = MonitoringConfigDefaults()
    standalone = StandaloneConfigDefaults()

    return {
        "server": {
            "host": server.host,
            "port": server.port,
            "reload": server.reload,
            "auto_open_browser": server.auto_open_browser,
            "show_startup_info": server.show_startup_info,
        },
        "health_check": {
            "enabled": health_check.enabled,
            "startup_interval": health_check.startup_interval,
            "startup_timeout": health_check.startup_timeout,
            "startup_hard_timeout": health_check.startup_hard_timeout,
            "readiness_interval": health_check.readiness_interval,
            "readiness_success_threshold": health_check.readiness_success_threshold,
            "readiness_failure_threshold": health_check.readiness_failure_threshold,
            "liveness_interval": health_check.liveness_interval,
            "liveness_failure_threshold": health_check.liveness_failure_threshold,
            "ping_timeout_http": health_check.ping_timeout_http,
            "ping_timeout_sse": health_check.ping_timeout_sse,
            "ping_timeout_stdio": health_check.ping_timeout_stdio,
            "warning_ping_timeout": health_check.warning_ping_timeout,
            "window_size": health_check.window_size,
            "window_min_calls": health_check.window_min_calls,
            "error_rate_threshold": health_check.error_rate_threshold,
            "latency_p95_warn": health_check.latency_p95_warn,
            "latency_p99_critical": health_check.latency_p99_critical,
            "max_reconnect_attempts": health_check.max_reconnect_attempts,
            "backoff_base": health_check.backoff_base,
            "backoff_max": health_check.backoff_max,
            "backoff_jitter": health_check.backoff_jitter,
            "backoff_max_duration": health_check.backoff_max_duration,
            "half_open_max_calls": health_check.half_open_max_calls,
            "half_open_success_rate_threshold": health_check.half_open_success_rate_threshold,
            "reconnect_hard_timeout": health_check.reconnect_hard_timeout,
            "lease_ttl": health_check.lease_ttl,
            "lease_renew_interval": health_check.lease_renew_interval,
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
            "tools_update_hours": monitoring.tools_update_hours,
            "reconnection_seconds": monitoring.reconnection_seconds,
            "cleanup_hours": monitoring.cleanup_hours,
            "enable_tools_update": monitoring.enable_tools_update,
            "enable_reconnection": monitoring.enable_reconnection,
            "update_tools_on_reconnection": monitoring.update_tools_on_reconnection,
            "detect_tools_changes": monitoring.detect_tools_changes,
            "local_service_ping_timeout": monitoring.local_service_ping_timeout,
            "remote_service_ping_timeout": monitoring.remote_service_ping_timeout,
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
        # Note: Removed configurations not managed via TOML:
        # - logging: Controlled by setup_store(debug=...) parameter
        # - cache: Controlled by setup_store(cache=...) parameter
        # - wrapper: Uses WrapperConfigDefaults in code
        # - sync: Hardcoded in unified_sync_manager.py
        # - transaction: Hardcoded in cache_manager.py
        # - api: Not actually used
        # - tool_set: Not actually used
    }
