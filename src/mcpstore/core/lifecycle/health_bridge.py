"""
健康状态桥梁模块
HealthStatus → ServiceConnectionState 状态映射桥梁

提供完整的状态映射逻辑，确保健康检查结果能够正确转换为生命周期状态。
"""

import logging
from typing import Optional

from mcpstore.core.lifecycle.health_manager import HealthStatus, HealthCheckResult
from mcpstore.core.models.service import ServiceConnectionState

logger = logging.getLogger(__name__)


class HealthStatusBridge:
    """健康状态到生命周期状态的映射桥梁"""
    
    #  核心映射表：HealthStatus → ServiceConnectionState
    STATUS_MAPPING = {
        HealthStatus.HEALTHY: ServiceConnectionState.HEALTHY,
        HealthStatus.WARNING: ServiceConnectionState.WARNING,
        HealthStatus.SLOW: ServiceConnectionState.WARNING,  # SLOW 映射为 WARNING
        HealthStatus.UNHEALTHY: ServiceConnectionState.RECONNECTING,
        HealthStatus.DISCONNECTED: ServiceConnectionState.DISCONNECTED,
        HealthStatus.RECONNECTING: ServiceConnectionState.RECONNECTING,
        HealthStatus.FAILED: ServiceConnectionState.UNREACHABLE,
        HealthStatus.UNKNOWN: ServiceConnectionState.DISCONNECTED,
    }
    
    @classmethod
    def map_health_to_lifecycle(cls, health_status: HealthStatus) -> ServiceConnectionState:
        """
        将 HealthStatus 映射为 ServiceConnectionState
        
        Args:
            health_status: 健康检查状态
            
        Returns:
            ServiceConnectionState: 对应的生命周期状态
            
        Raises:
            ValueError: 当遇到未映射的健康状态时
        """
        if health_status not in cls.STATUS_MAPPING:
            error_msg = f"未知的健康状态，无法映射: {health_status}"
            logger.error(f"❌ [HEALTH_BRIDGE] {error_msg}")
            raise ValueError(error_msg)
        
        lifecycle_state = cls.STATUS_MAPPING[health_status]
        logger.debug(f" [HEALTH_BRIDGE] 状态映射: {health_status.value} → {lifecycle_state.value}")
        
        return lifecycle_state
    
    @classmethod
    def map_health_result_to_lifecycle(cls, health_result: HealthCheckResult) -> ServiceConnectionState:
        """
        将完整的 HealthCheckResult 映射为 ServiceConnectionState
        
        Args:
            health_result: 健康检查结果
            
        Returns:
            ServiceConnectionState: 对应的生命周期状态
        """
        return cls.map_health_to_lifecycle(health_result.status)
    
    @classmethod
    def is_health_status_positive(cls, health_status: HealthStatus) -> bool:
        """
        判断健康状态是否为正面状态（等效于之前的布尔值判断）
        
        Args:
            health_status: 健康检查状态
            
        Returns:
            bool: True表示正面状态，False表示负面状态
        """
        # 保持与原有逻辑一致：只有 UNHEALTHY 返回 False
        return health_status != HealthStatus.UNHEALTHY
    
    @classmethod
    def get_mapping_summary(cls) -> dict:
        """
        获取映射关系摘要（用于调试和文档）
        
        Returns:
            dict: 映射关系摘要
        """
        return {
            "mappings": {
                health.value: lifecycle.value 
                for health, lifecycle in cls.STATUS_MAPPING.items()
            },
            "total_mappings": len(cls.STATUS_MAPPING),
            "positive_statuses": [
                status.value for status in HealthStatus 
                if cls.is_health_status_positive(status)
            ]
        }


#  便利函数：向后兼容
def map_health_to_lifecycle(health_status: HealthStatus) -> ServiceConnectionState:
    """向后兼容的便利函数"""
    return HealthStatusBridge.map_health_to_lifecycle(health_status)


def is_health_positive(health_status: HealthStatus) -> bool:
    """向后兼容的便利函数"""
    return HealthStatusBridge.is_health_status_positive(health_status)
