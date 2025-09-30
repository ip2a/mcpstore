"""
统一服务状态管理器
提供统一的状态管理接口，简化组件间的状态操作
"""

import logging
from datetime import datetime
from typing import Optional, Dict, Any

from mcpstore.core.lifecycle.health_manager import HealthCheckResult
from mcpstore.core.lifecycle.health_bridge import HealthStatusBridge
from mcpstore.core.models.service import ServiceConnectionState, ServiceStateMetadata

logger = logging.getLogger(__name__)


class UnifiedServiceStateManager:
    """统一服务状态管理器"""
    
    def __init__(self, registry):
        """
        初始化统一状态管理器
        
        Args:
            registry: ServiceRegistry 实例
        """
        self.registry = registry
        self.health_bridge = HealthStatusBridge()
        
        logger.info("UnifiedServiceStateManager initialized")
    
    def set_service_state_with_health_info(self, agent_id: str, service_name: str, 
                                         health_result: HealthCheckResult) -> ServiceConnectionState:
        """
        根据健康检查结果设置服务状态
        
        Args:
            agent_id: Agent ID
            service_name: Service name
            health_result: 健康检查结果
            
        Returns:
            ServiceConnectionState: 实际设置的生命周期状态
            
        Raises:
            ValueError: 当健康状态无法映射时
        """
        try:
            # 映射健康状态到生命周期状态
            lifecycle_state = self.health_bridge.map_health_result_to_lifecycle(health_result)
            
            # 设置状态
            self.registry.set_service_state(agent_id, service_name, lifecycle_state)
            
            # 更新元数据
            self._update_metadata_from_health_result(agent_id, service_name, health_result)
            
            logger.debug(f" [UNIFIED_STATE] 状态更新: {service_name} → {lifecycle_state.value} (基于 {health_result.status.value})")
            
            return lifecycle_state
            
        except Exception as e:
            logger.error(f" [UNIFIED_STATE] 状态设置失败: {service_name}, error: {e}")
            # 发生错误时，设置为DISCONNECTED状态作为安全回退
            fallback_state = ServiceConnectionState.DISCONNECTED
            self.registry.set_service_state(agent_id, service_name, fallback_state)
            logger.warning(f"⚠️ [UNIFIED_STATE] 使用安全回退状态: {service_name} → {fallback_state.value}")
            return fallback_state
    
    def set_service_state_direct(self, agent_id: str, service_name: str, 
                               state: ServiceConnectionState, 
                               error_message: Optional[str] = None) -> None:
        """
        直接设置服务状态（用于非健康检查的状态变更）
        
        Args:
            agent_id: Agent ID
            service_name: Service name
            state: 目标状态
            error_message: 错误信息（可选）
        """
        self.registry.set_service_state(agent_id, service_name, state)
        
        # 更新基本元数据
        metadata = self.registry.get_service_metadata(agent_id, service_name)
        if metadata:
            metadata.state_entered_time = datetime.now()
            if error_message:
                metadata.error_message = error_message
        
        logger.debug(f" [UNIFIED_STATE] 直接状态更新: {service_name} → {state.value}")
    
    def get_service_state_info(self, agent_id: str, service_name: str) -> Dict[str, Any]:
        """
        获取服务的完整状态信息
        
        Args:
            agent_id: Agent ID
            service_name: Service name
            
        Returns:
            Dict: 完整的状态信息
        """
        state = self.registry.get_service_state(agent_id, service_name)
        metadata = self.registry.get_service_metadata(agent_id, service_name)
        
        info = {
            "service_name": service_name,
            "agent_id": agent_id,
            "state": state.value if state else "unknown",
            "state_enum": state,
            "healthy": self._is_state_healthy(state),
            "available": self._is_state_available(state),
        }
        
        if metadata:
            info.update({
                "last_health_check": metadata.last_health_check,
                "last_response_time": metadata.last_response_time,
                "consecutive_failures": metadata.consecutive_failures,
                "consecutive_successes": metadata.consecutive_successes,
                "error_message": metadata.error_message,
                "state_entered_time": metadata.state_entered_time,
                "reconnect_attempts": metadata.reconnect_attempts,
            })
        
        return info
    
    def transition_service_state(self, agent_id: str, service_name: str, 
                               target_state: ServiceConnectionState,
                               reason: Optional[str] = None) -> bool:
        """
        执行状态转换（带验证）
        
        Args:
            agent_id: Agent ID
            service_name: Service name
            target_state: 目标状态
            reason: 转换原因
            
        Returns:
            bool: 转换是否成功
        """
        current_state = self.registry.get_service_state(agent_id, service_name)
        
        if current_state == target_state:
            logger.debug(f" [UNIFIED_STATE] 状态无需转换: {service_name} 已在 {target_state.value}")
            return True
        
        # 验证转换是否合理
        if self._is_valid_transition(current_state, target_state):
            self.set_service_state_direct(agent_id, service_name, target_state, reason)
            logger.info(f" [UNIFIED_STATE] 状态转换成功: {service_name} {current_state.value if current_state else 'None'} → {target_state.value}")
            return True
        else:
            logger.warning(f"⚠️ [UNIFIED_STATE] 无效状态转换: {service_name} {current_state.value if current_state else 'None'} → {target_state.value}")
            return False
    
    def reset_service_state(self, agent_id: str, service_name: str) -> None:
        """
        重置服务状态到初始状态
        
        Args:
            agent_id: Agent ID
            service_name: Service name
        """
        self.set_service_state_direct(
            agent_id, service_name, 
            ServiceConnectionState.INITIALIZING, 
            "状态重置"
        )
        
        # 重置元数据
        metadata = self.registry.get_service_metadata(agent_id, service_name)
        if metadata:
            metadata.consecutive_failures = 0
            metadata.consecutive_successes = 0
            metadata.reconnect_attempts = 0
            metadata.error_message = None
        
        logger.info(f" [UNIFIED_STATE] 服务状态已重置: {service_name}")
    
    def _update_metadata_from_health_result(self, agent_id: str, service_name: str, 
                                          health_result: HealthCheckResult) -> None:
        """根据健康检查结果更新元数据"""
        metadata = self.registry.get_service_metadata(agent_id, service_name)
        if not metadata:
            return
        
        # 更新基本信息
        metadata.last_health_check = datetime.now()
        metadata.last_response_time = health_result.response_time
        metadata.error_message = health_result.error_message
        
        # 更新成功/失败计数
        is_positive = self.health_bridge.is_health_status_positive(health_result.status)
        if is_positive:
            metadata.consecutive_successes += 1
            metadata.consecutive_failures = 0
        else:
            metadata.consecutive_failures += 1
            metadata.consecutive_successes = 0
    
    def _is_state_healthy(self, state: Optional[ServiceConnectionState]) -> bool:
        """判断状态是否为健康状态"""
        if not state:
            return False
        return state in [ServiceConnectionState.HEALTHY, ServiceConnectionState.WARNING]
    
    def _is_state_available(self, state: Optional[ServiceConnectionState]) -> bool:
        """判断状态是否为可用状态"""
        if not state:
            return False
        return state in [
            ServiceConnectionState.HEALTHY, 
            ServiceConnectionState.WARNING,
            ServiceConnectionState.INITIALIZING
        ]
    
    def _is_valid_transition(self, from_state: Optional[ServiceConnectionState], 
                           to_state: ServiceConnectionState) -> bool:
        """验证状态转换是否合理"""
        # 基本转换规则（可以根据需要扩展）
        
        # 从 None 状态只能转换到 INITIALIZING 或 DISCONNECTED
        if from_state is None:
            return to_state in [ServiceConnectionState.INITIALIZING, ServiceConnectionState.DISCONNECTED]
        
        # 任何状态都可以转换到 DISCONNECTED 和 INITIALIZING（强制转换）
        if to_state in [ServiceConnectionState.DISCONNECTED, ServiceConnectionState.INITIALIZING]:
            return True
        
        # 其他转换规则
        valid_transitions = {
            ServiceConnectionState.INITIALIZING: [
                ServiceConnectionState.HEALTHY, 
                ServiceConnectionState.RECONNECTING, 
                ServiceConnectionState.DISCONNECTED
            ],
            ServiceConnectionState.HEALTHY: [
                ServiceConnectionState.WARNING, 
                ServiceConnectionState.RECONNECTING,
                ServiceConnectionState.DISCONNECTING
            ],
            ServiceConnectionState.WARNING: [
                ServiceConnectionState.HEALTHY, 
                ServiceConnectionState.RECONNECTING,
                ServiceConnectionState.DISCONNECTING
            ],
            ServiceConnectionState.RECONNECTING: [
                ServiceConnectionState.HEALTHY, 
                ServiceConnectionState.WARNING,
                ServiceConnectionState.UNREACHABLE,
                ServiceConnectionState.DISCONNECTED
            ],
            ServiceConnectionState.UNREACHABLE: [
                ServiceConnectionState.RECONNECTING,
                ServiceConnectionState.HEALTHY,
                ServiceConnectionState.DISCONNECTED
            ],
            ServiceConnectionState.DISCONNECTING: [
                ServiceConnectionState.DISCONNECTED
            ],
            ServiceConnectionState.DISCONNECTED: [
                ServiceConnectionState.INITIALIZING
            ]
        }
        
        allowed_transitions = valid_transitions.get(from_state, [])
        return to_state in allowed_transitions
    
    def get_statistics(self) -> Dict[str, Any]:
        """获取状态管理统计信息"""
        all_agents = self.registry.get_all_agent_ids()
        stats = {
            "total_agents": len(all_agents),
            "state_distribution": {},
            "health_summary": {
                "healthy": 0,
                "available": 0,
                "total": 0
            }
        }
        
        for agent_id in all_agents:
            service_names = self.registry.get_all_service_names(agent_id)
            for service_name in service_names:
                state = self.registry.get_service_state(agent_id, service_name)
                if state:
                    state_value = state.value
                    stats["state_distribution"][state_value] = stats["state_distribution"].get(state_value, 0) + 1
                    stats["health_summary"]["total"] += 1
                    
                    if self._is_state_healthy(state):
                        stats["health_summary"]["healthy"] += 1
                    if self._is_state_available(state):
                        stats["health_summary"]["available"] += 1
        
        return stats
