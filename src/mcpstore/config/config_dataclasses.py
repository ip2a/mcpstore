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
    # 探针与就绪
    startup_interval: float = _health_defaults.startup_interval
    startup_timeout: float = _health_defaults.startup_timeout
    startup_hard_timeout: float = _health_defaults.startup_hard_timeout
    readiness_interval: float = _health_defaults.readiness_interval
    readiness_success_threshold: int = _health_defaults.readiness_success_threshold
    readiness_failure_threshold: int = _health_defaults.readiness_failure_threshold
    liveness_interval: float = _health_defaults.liveness_interval
    liveness_failure_threshold: int = _health_defaults.liveness_failure_threshold
    ping_timeout_http: float = _health_defaults.ping_timeout_http
    ping_timeout_sse: float = _health_defaults.ping_timeout_sse
    ping_timeout_stdio: float = _health_defaults.ping_timeout_stdio
    warning_ping_timeout: float = _health_defaults.warning_ping_timeout

    # 窗口判定
    window_size: int = _health_defaults.window_size
    window_min_calls: int = _health_defaults.window_min_calls
    error_rate_threshold: float = _health_defaults.error_rate_threshold
    latency_p95_warn: float = _health_defaults.latency_p95_warn
    latency_p99_critical: float = _health_defaults.latency_p99_critical

    # 退避与熔断/半开
    max_reconnect_attempts: int = _health_defaults.max_reconnect_attempts
    backoff_base: float = _health_defaults.backoff_base
    backoff_max: float = _health_defaults.backoff_max
    backoff_jitter: float = _health_defaults.backoff_jitter
    backoff_max_duration: float = _health_defaults.backoff_max_duration
    half_open_max_calls: int = _health_defaults.half_open_max_calls
    half_open_success_rate_threshold: float = _health_defaults.half_open_success_rate_threshold
    reconnect_hard_timeout: float = _health_defaults.reconnect_hard_timeout

    # 生命周期超时
    initialization_timeout: float = _service_defaults.initialization_timeout
    termination_timeout: float = _service_defaults.termination_timeout
    shutdown_timeout: float = _service_defaults.shutdown_timeout

    # 租约
    lease_ttl: float = _health_defaults.lease_ttl
    lease_renew_interval: float = _health_defaults.lease_renew_interval

    # 重启与日志
    restart_delay_seconds: float = 5.0
    max_restart_attempts: int = 3
    enable_detailed_logging: bool = True
    collect_startup_metrics: bool = True
    collect_runtime_metrics: bool = True
    collect_shutdown_metrics: bool = True
