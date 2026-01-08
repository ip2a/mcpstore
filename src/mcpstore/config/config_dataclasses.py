"""
MCPStore Configuration Dataclasses

独立的数据类定义，避免循环导入依赖
"""

from dataclasses import dataclass

from .config_defaults import ContentUpdateConfigDefaults, HealthCheckConfigDefaults, ServiceLifecycleConfigDefaults

_content_defaults = ContentUpdateConfigDefaults()
_health_defaults = HealthCheckConfigDefaults()
_service_defaults = ServiceLifecycleConfigDefaults()


@dataclass
class ContentUpdateConfig:
    """Content update configuration dataclass."""
    tools_update_interval: float = _content_defaults.tools_update_interval      # 5 minutes
    resources_update_interval: float = _content_defaults.resources_update_interval  # 10 minutes
    prompts_update_interval: float = _content_defaults.prompts_update_interval    # 10 minutes
    max_concurrent_updates: int = _content_defaults.max_concurrent_updates
    update_timeout: float = _content_defaults.update_timeout              # 30 seconds
    max_consecutive_failures: int = _content_defaults.max_consecutive_failures
    failure_backoff_multiplier: float = _content_defaults.failure_backoff_multiplier

    enable_auto_update: bool = True
    enable_content_validation: bool = True


@dataclass
class ServiceLifecycleConfig:
    """Service lifecycle configuration (single source of truth)"""
    # State transition thresholds (failure count)
    warning_failure_threshold: int = _health_defaults.warning_failure_threshold          # First failure in HEALTHY enters WARNING
    reconnecting_failure_threshold: int = _health_defaults.reconnecting_failure_threshold     # Two consecutive failures in WARNING enter RECONNECTING
    max_reconnect_attempts: int = _health_defaults.max_reconnect_attempts            # Maximum reconnection attempts

    # Reconnection backoff
    base_reconnect_delay: float = _health_defaults.base_reconnect_delay           # Base reconnection delay (seconds)
    max_reconnect_delay: float = _health_defaults.max_reconnect_delay           # Maximum reconnection delay (seconds)
    long_retry_interval: float = _health_defaults.long_retry_interval          # Long retry interval (seconds)

    # Health check (period/threshold/timeout)
    normal_heartbeat_interval: float = _health_defaults.normal_heartbeat_interval     # Normal state heartbeat interval
    warning_heartbeat_interval: float = _health_defaults.warning_heartbeat_interval    # Warning state heartbeat interval
    health_check_ping_timeout: float = _health_defaults.health_check_ping_timeout     # Health check ping timeout
    warning_ping_timeout: float = _health_defaults.warning_ping_timeout          # Warning/Reconnecting 状态下的宽松超时
    ping_timeout_http: float = _health_defaults.ping_timeout_http               # HTTP 传输默认 ping 超时
    ping_timeout_sse: float = _health_defaults.ping_timeout_sse                # SSE 传输默认 ping 超时
    ping_timeout_stdio: float = _health_defaults.ping_timeout_stdio              # STDIO/Studio 传输默认 ping 超时
    disconnection_timeout: float = _health_defaults.disconnection_timeout         # Disconnection detection timeout

    # Lifecycle timeouts
    initialization_timeout: float = _service_defaults.initialization_timeout        # Service initialization timeout
    termination_timeout: float = _service_defaults.termination_timeout           # Service termination timeout
    shutdown_timeout: float = _service_defaults.shutdown_timeout               # Graceful shutdown timeout

    # Retry and restart behavior
    restart_delay_seconds: float = 5.0           # Delay before restart attempt
    max_restart_attempts: int = 3               # Maximum restart attempts

    # Logging and monitoring
    enable_detailed_logging: bool = True       # Enable detailed lifecycle logging
    collect_startup_metrics: bool = True      # Collect startup performance metrics
    collect_runtime_metrics: bool = True       # Collect runtime performance metrics
    collect_shutdown_metrics: bool = True      # Collect shutdown performance metrics
