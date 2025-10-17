"""
服务生命周期配置
"""

from dataclasses import dataclass

@dataclass
class ServiceLifecycleConfig:
    """服务生命周期配置（唯一配置源）"""
    # 状态转换阈值（失败次数）
    warning_failure_threshold: int = 1          # HEALTHY 首次失败即进入 WARNING
    reconnecting_failure_threshold: int = 2     # WARNING 下连续两次失败进入 RECONNECTING
    max_reconnect_attempts: int = 10            # 最大重连尝试次数

    # 重连退避
    base_reconnect_delay: float = 1.0           # 基础重连延迟（秒）
    max_reconnect_delay: float = 60.0           # 最大重连延迟（秒）
    long_retry_interval: float = 300.0          # 长周期重试间隔（秒）

    # 健康检查（周期/阈值/超时）
    normal_heartbeat_interval: float = 30.0     # 正常心跳间隔（秒）
    warning_heartbeat_interval: float = 10.0    # 警告状态心跳间隔（秒）
    health_check_ping_timeout: float = 10.0     # 健康检查 ping 超时（秒）

    # 超时配置
    initialization_timeout: float = 300.0       # 初始化超时（秒）
    disconnection_timeout: float = 10.0         # 断连超时（秒）
