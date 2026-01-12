"""
Service Lifecycle Configuration
"""

from dataclasses import dataclass

from mcpstore.config.config_defaults import (
    HealthCheckConfigDefaults,
    ServiceLifecycleConfigDefaults,
)

_health_defaults = HealthCheckConfigDefaults()
_service_defaults = ServiceLifecycleConfigDefaults()


@dataclass
class ServiceLifecycleConfig:
    """Service lifecycle configuration (single source of truth)"""
    # State transition thresholds (failure count)
    warning_failure_threshold: int = _health_defaults.warning_failure_threshold          # First failure in HEALTHY enters DEGRADED
    reconnecting_failure_threshold: int = _health_defaults.reconnecting_failure_threshold     # Two consecutive failures in DEGRADED enter CIRCUIT_OPEN
    max_reconnect_attempts: int = _health_defaults.max_reconnect_attempts            # Maximum reconnection attempts

    # Reconnection backoff
    base_reconnect_delay: float = _health_defaults.base_reconnect_delay           # Base reconnection delay (seconds)
    max_reconnect_delay: float = _health_defaults.max_reconnect_delay           # Maximum reconnection delay (seconds)
    long_retry_interval: float = _health_defaults.long_retry_interval          # Long retry interval (seconds)

    # Health check (period/threshold/timeout)
    normal_heartbeat_interval: float = _health_defaults.normal_heartbeat_interval     # Normal heartbeat interval (seconds)
    warning_heartbeat_interval: float = _health_defaults.warning_heartbeat_interval    # Warning state heartbeat interval (seconds)
    health_check_ping_timeout: float = _health_defaults.health_check_ping_timeout     # Health check ping timeout (seconds)
    warning_ping_timeout: float = _health_defaults.warning_ping_timeout         # Warning/Reconnecting 状态下的宽松超时
    ping_timeout_http: float = _health_defaults.ping_timeout_http              # HTTP 传输默认 ping 超时
    ping_timeout_sse: float = _health_defaults.ping_timeout_sse               # SSE 传输默认 ping 超时
    ping_timeout_stdio: float = _health_defaults.ping_timeout_stdio             # STDIO/Studio 传输默认 ping 超时

    # Timeout configuration
    initialization_timeout: float = _service_defaults.initialization_timeout       # Initialization timeout (seconds)
    disconnection_timeout: float = _health_defaults.disconnection_timeout         # Disconnection timeout (seconds)
