"""
服务生命周期配置
"""

from dataclasses import dataclass

@dataclass
class ServiceLifecycleConfig:
    """服务生命周期配置"""
    # 状态转换阈值
    warning_failure_threshold: int = 2          # 进入WARNING状态的失败次数阈值
    reconnecting_failure_threshold: int = 1     # 🔧 修复：降低阈值，首次失败即转到RECONNECTING
    max_reconnect_attempts: int = 10            # 最大重连尝试次数
    
    # 重试间隔配置
    base_reconnect_delay: float = 1.0           # 基础重连延迟（秒）
    max_reconnect_delay: float = 60.0           # 最大重连延迟（秒）
    long_retry_interval: float = 300.0          # 长周期重试间隔（5分钟）
    
    # 心跳配置
    normal_heartbeat_interval: float = 30.0     # 正常心跳间隔（秒）
    warning_heartbeat_interval: float = 10.0    # 警告状态心跳间隔（秒）
    
    # 超时配置
    initialization_timeout: float = 30.0        # 初始化超时（秒）
    disconnection_timeout: float = 10.0         # 断连超时（秒）
