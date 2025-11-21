"""
Service Lifecycle Configuration
"""

from dataclasses import dataclass

@dataclass
class ServiceLifecycleConfig:
    """Service lifecycle configuration (single source of truth)"""
    # State transition thresholds (failure count)
    warning_failure_threshold: int = 1          # First failure in HEALTHY enters WARNING
    reconnecting_failure_threshold: int = 2     # Two consecutive failures in WARNING enter RECONNECTING
    max_reconnect_attempts: int = 10            # Maximum reconnection attempts

    # Reconnection backoff
    base_reconnect_delay: float = 1.0           # Base reconnection delay (seconds)
    max_reconnect_delay: float = 60.0           # Maximum reconnection delay (seconds)
    long_retry_interval: float = 300.0          # Long retry interval (seconds)

    # Health check (period/threshold/timeout)
    normal_heartbeat_interval: float = 30.0     # Normal heartbeat interval (seconds)
    warning_heartbeat_interval: float = 10.0    # Warning state heartbeat interval (seconds)
    health_check_ping_timeout: float = 10.0     # Health check ping timeout (seconds)

    # Timeout configuration
    initialization_timeout: float = 300.0       # Initialization timeout (seconds)
    disconnection_timeout: float = 10.0         # Disconnection timeout (seconds)
