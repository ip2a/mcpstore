"""
MCPStore Health Status Manager
Implements advanced health check features such as hierarchical health status and intelligent timeout adjustment
"""

import logging
import time
from collections import deque
from dataclasses import dataclass, field
from enum import Enum
from typing import Dict, List, Optional, Any

logger = logging.getLogger(__name__)

class HealthStatus(Enum):
    """Service health status enumeration"""
    HEALTHY = "healthy"         # Normal response, fast time
    WARNING = "warning"         # Normal response, but slow
    SLOW = "slow"              # Very slow response but successful
    UNHEALTHY = "unhealthy"    # Response failed or timeout
    DISCONNECTED = "disconnected"  # Disconnected
    RECONNECTING = "reconnecting"  # Reconnecting
    FAILED = "failed"          # Reconnection failed, abandoned
    UNKNOWN = "unknown"        # Status unknown

@dataclass
class HealthCheckResult:
    """Health check result"""
    status: HealthStatus
    response_time: float
    timestamp: float
    error_message: Optional[str] = None
    details: Dict[str, Any] = field(default_factory=dict)

@dataclass
class ServiceHealthConfig:
    """Service health configuration"""
    # Timeout configuration
    ping_timeout: float = 3.0
    startup_wait_time: float = 2.0
    
    # Health status thresholds
    healthy_threshold: float = 1.0      # Healthy within 1 second
    warning_threshold: float = 3.0      # Warning within 3 seconds
    slow_threshold: float = 10.0        # Slow response within 10 seconds

    # Intelligent timeout configuration
    enable_adaptive_timeout: bool = False
    adaptive_multiplier: float = 2.0
    history_size: int = 10

@dataclass
class ServiceHealthTracker:
    """服务健康状态跟踪器"""
    service_name: str
    current_status: HealthStatus = HealthStatus.UNKNOWN
    response_times: deque = field(default_factory=lambda: deque(maxlen=10))
    last_check_time: float = 0.0
    consecutive_failures: int = 0
    consecutive_successes: int = 0
    total_checks: int = 0
    total_failures: int = 0
    
    def update(self, result: HealthCheckResult):
        """更新健康状态"""
        self.current_status = result.status
        self.response_times.append(result.response_time)
        self.last_check_time = result.timestamp
        self.total_checks += 1
        
        if result.status == HealthStatus.UNHEALTHY:
            self.consecutive_failures += 1
            self.consecutive_successes = 0
            self.total_failures += 1
        else:
            self.consecutive_successes += 1
            self.consecutive_failures = 0
    
    def get_average_response_time(self) -> float:
        """获取平均响应时间"""
        if not self.response_times:
            return 0.0
        return sum(self.response_times) / len(self.response_times)
    
    def get_failure_rate(self) -> float:
        """获取失败率"""
        if self.total_checks == 0:
            return 0.0
        return self.total_failures / self.total_checks

class HealthManager:
    """健康状态管理器"""
    
    def __init__(self):
        self.config = ServiceHealthConfig()
        self.service_trackers: Dict[str, ServiceHealthTracker] = {}
        logger.info("HealthManager initialized")
    
    def update_config(self, config: Dict[str, Any]):
        """更新健康管理器配置"""
        for key, value in config.items():
            if hasattr(self.config, key):
                setattr(self.config, key, value)
        logger.info(f"Health manager config updated: {self.config}")
    
    def get_service_timeout(self, service_name: str, service_config: Dict[str, Any]) -> float:
        """获取服务的智能超时时间"""
        base_timeout = self.config.ping_timeout
        
        # 如果启用了智能超时调整
        if self.config.enable_adaptive_timeout and service_name in self.service_trackers:
            tracker = self.service_trackers[service_name]
            avg_response_time = tracker.get_average_response_time()
            
            if avg_response_time > 0:
                # 基于历史响应时间调整超时
                adaptive_timeout = avg_response_time * self.config.adaptive_multiplier
                return min(adaptive_timeout, base_timeout * 3)  # 最多3倍基础超时
        
        # 根据服务类型调整基础超时
        if service_config.get("command"):
            # 本地服务通常响应更快
            return base_timeout * 0.8
        else:
            # 远程服务可能需要更多时间
            return base_timeout
    
    def record_health_check(self, service_name: str, response_time: float, 
                          success: bool, error_message: Optional[str] = None,
                          service_config: Dict[str, Any] = None) -> HealthCheckResult:
        """记录健康检查结果并返回状态"""
        timestamp = time.time()
        
        # 确保服务跟踪器存在
        if service_name not in self.service_trackers:
            self.service_trackers[service_name] = ServiceHealthTracker(service_name)
        
        tracker = self.service_trackers[service_name]
        
        # 确定健康状态
        if not success:
            status = HealthStatus.UNHEALTHY
        elif response_time <= self.config.healthy_threshold:
            status = HealthStatus.HEALTHY
        elif response_time <= self.config.warning_threshold:
            status = HealthStatus.WARNING
        elif response_time <= self.config.slow_threshold:
            status = HealthStatus.SLOW
        else:
            status = HealthStatus.UNHEALTHY  # 太慢也认为是不健康
        
        # 创建结果
        result = HealthCheckResult(
            status=status,
            response_time=response_time,
            timestamp=timestamp,
            error_message=error_message,
            details={
                "service_config_type": "local" if service_config and service_config.get("command") else "remote",
                "consecutive_failures": tracker.consecutive_failures,
                "consecutive_successes": tracker.consecutive_successes
            }
        )
        
        # 更新跟踪器
        tracker.update(result)
        
        return result
    
    def get_service_health_summary(self, service_name: str) -> Dict[str, Any]:
        """获取服务健康状态摘要"""
        if service_name not in self.service_trackers:
            return {
                "service_name": service_name,
                "status": HealthStatus.UNKNOWN.value,
                "message": "No health data available"
            }
        
        tracker = self.service_trackers[service_name]
        return {
            "service_name": service_name,
            "status": tracker.current_status.value,
            "average_response_time": tracker.get_average_response_time(),
            "failure_rate": tracker.get_failure_rate(),
            "consecutive_failures": tracker.consecutive_failures,
            "consecutive_successes": tracker.consecutive_successes,
            "total_checks": tracker.total_checks,
            "last_check_time": tracker.last_check_time
        }
    
    def get_all_services_health(self) -> Dict[str, Dict[str, Any]]:
        """获取所有服务的健康状态"""
        return {
            service_name: self.get_service_health_summary(service_name)
            for service_name in self.service_trackers
        }
    
    def cleanup_service(self, service_name: str):
        """清理服务的健康状态数据"""
        if service_name in self.service_trackers:
            del self.service_trackers[service_name]
            logger.debug(f"Cleaned up health data for service: {service_name}")

# 全局健康管理器实例
_health_manager = None

def get_health_manager() -> HealthManager:
    """获取全局健康管理器实例"""
    global _health_manager
    if _health_manager is None:
        _health_manager = HealthManager()
    return _health_manager
