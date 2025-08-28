"""
Service Lifecycle State Machine
Responsible for handling service state transition logic
"""

import logging
from datetime import datetime, timedelta
from typing import Optional

from mcpstore.core.models.service import ServiceConnectionState, ServiceStateMetadata
from .config import ServiceLifecycleConfig

logger = logging.getLogger(__name__)


class ServiceStateMachine:
    """Service lifecycle state machine"""
    
    def __init__(self, config: ServiceLifecycleConfig):
        self.config = config
    
    async def handle_success_transition(self, agent_id: str, service_name: str,
                                       current_state: ServiceConnectionState,
                                       get_metadata_func, transition_func):
        """Handle state transitions on success"""
        logger.debug(f"[SUCCESS_TRANSITION] processing service='{service_name}' current_state={current_state}")

        if current_state in [ServiceConnectionState.INITIALIZING,
                           ServiceConnectionState.WARNING,
                           ServiceConnectionState.RECONNECTING,
                           ServiceConnectionState.UNREACHABLE]:  # 🔧 Added: UNREACHABLE can also recover to HEALTHY
            # 🔧 Fix: Reset all failure-related counters on successful transition
            metadata = get_metadata_func(agent_id, service_name)
            if metadata:
                metadata.consecutive_failures = 0
                metadata.reconnect_attempts = 0
                metadata.next_retry_time = None
                metadata.error_message = None
                logger.debug(f"[SUCCESS_TRANSITION] reset_counters service='{service_name}'")
            await transition_func(agent_id, service_name, ServiceConnectionState.HEALTHY)
        elif current_state == ServiceConnectionState.HEALTHY:
            logger.debug(f"[SUCCESS_TRANSITION] already_healthy service='{service_name}'")
        elif current_state in [ServiceConnectionState.DISCONNECTING, ServiceConnectionState.DISCONNECTED]:
            logger.debug(f"[SUCCESS_TRANSITION] no_transition service='{service_name}' reason='disconnecting/disconnected'")
        else:
            logger.debug(f"[SUCCESS_TRANSITION] no_rules state={current_state}")

        logger.debug(f"[SUCCESS_TRANSITION] completed service='{service_name}'")
    
    async def handle_failure_transition(self, agent_id: str, service_name: str,
                                       current_state: ServiceConnectionState,
                                       get_metadata_func, transition_func):
        """处理失败时的状态转换"""
        logger.debug(f"[FAILURE_TRANSITION] start service='{service_name}' current_state={current_state}")

        metadata = get_metadata_func(agent_id, service_name)
        if not metadata:
            logger.error(f"[FAILURE_TRANSITION] no_metadata service='{service_name}'")
            return

        logger.debug(f"[FAILURE_TRANSITION] metadata failures={metadata.consecutive_failures} reconnect_attempts={metadata.reconnect_attempts}")
        logger.debug(f"[FAILURE_TRANSITION] thresholds warning={self.config.warning_failure_threshold} reconnecting={self.config.reconnecting_failure_threshold} max_reconnect={self.config.max_reconnect_attempts}")

        if current_state == ServiceConnectionState.HEALTHY:
            logger.debug(f"[FAILURE_TRANSITION] healthy_processing")
            if metadata.consecutive_failures >= self.config.warning_failure_threshold:
                logger.debug(f"[FAILURE_TRANSITION] transition HEALTHY->WARNING failures={metadata.consecutive_failures} threshold={self.config.warning_failure_threshold}")
                await transition_func(agent_id, service_name, ServiceConnectionState.WARNING)
            else:
                logger.debug(f"[FAILURE_TRANSITION] not_enough_failures failures={metadata.consecutive_failures} threshold={self.config.warning_failure_threshold}")

        elif current_state == ServiceConnectionState.WARNING:
            logger.debug(f"[FAILURE_TRANSITION] warning_processing")
            if metadata.consecutive_failures >= self.config.reconnecting_failure_threshold:
                logger.debug(f"[FAILURE_TRANSITION] transition WARNING->RECONNECTING failures={metadata.consecutive_failures} threshold={self.config.reconnecting_failure_threshold}")
                await transition_func(agent_id, service_name, ServiceConnectionState.RECONNECTING)
            else:
                logger.debug(f"[FAILURE_TRANSITION] not_enough_failures failures={metadata.consecutive_failures} threshold={self.config.reconnecting_failure_threshold}")

        elif current_state == ServiceConnectionState.INITIALIZING:
            logger.debug(f"[FAILURE_TRANSITION] initializing_processing")
            # 初次连接失败应当直接进入 RECONNECTING，而不是等待阈值
            logger.debug(f"[FAILURE_TRANSITION] transition INITIALIZING->RECONNECTING reason='first_failure'")
            await transition_func(agent_id, service_name, ServiceConnectionState.RECONNECTING)

        elif current_state == ServiceConnectionState.RECONNECTING:
            logger.debug(f"[FAILURE_TRANSITION] reconnecting_processing")
            if metadata.reconnect_attempts >= self.config.max_reconnect_attempts:
                logger.debug(f"[FAILURE_TRANSITION] transition RECONNECTING->UNREACHABLE attempts={metadata.reconnect_attempts} threshold={self.config.max_reconnect_attempts}")
                await transition_func(agent_id, service_name, ServiceConnectionState.UNREACHABLE)
            else:
                logger.debug(f"[FAILURE_TRANSITION] not_enough_attempts attempts={metadata.reconnect_attempts} threshold={self.config.max_reconnect_attempts}")

        elif current_state == ServiceConnectionState.UNREACHABLE:
            logger.debug(f"[FAILURE_TRANSITION] unreachable_final_state=True")

        elif current_state in [ServiceConnectionState.DISCONNECTING, ServiceConnectionState.DISCONNECTED]:
            logger.debug(f"[FAILURE_TRANSITION] no_transition service='{service_name}' reason='disconnecting/disconnected'")

        else:
            logger.debug(f"⏸️ [FAILURE_TRANSITION] No transition rules for state {current_state}")

        logger.debug(f"[FAILURE_TRANSITION] completed service='{service_name}'")
    
    async def transition_to_state(self, agent_id: str, service_name: str,
                                 new_state: ServiceConnectionState,
                                 get_state_func, get_metadata_func, 
                                 set_state_func, on_state_entered_func):
        """执行状态转换"""
        old_state = get_state_func(agent_id, service_name)
        logger.debug(f"[STATE_TRANSITION] attempting service='{service_name}' from={old_state} to={new_state}")

        if old_state == new_state:
            logger.debug(f"⏸️ [STATE_TRANSITION] No change needed for {service_name}: already in {new_state}")
            return

        # 更新状态
        logger.debug(f"[STATE_TRANSITION] updating service='{service_name}' from={old_state} to={new_state}")
        set_state_func(agent_id, service_name, new_state)
        metadata = get_metadata_func(agent_id, service_name)
        if metadata:
            metadata.state_entered_time = datetime.now()
            logger.debug(f"[STATE_TRANSITION] updated_state_entered_time service='{service_name}'")
        else:
            logger.warning(f"[STATE_TRANSITION] no_metadata service='{service_name}' during_transition=True")

        # 执行状态进入处理
        logger.debug(f"[STATE_TRANSITION] calling_on_state_entered service='{service_name}'")
        await on_state_entered_func(agent_id, service_name, new_state, old_state)

        logger.info(f"[STATE_TRANSITION] transitioned service='{service_name}' agent='{agent_id}' from={old_state} to={new_state}")
    
    async def on_state_entered(self, agent_id: str, service_name: str, 
                              new_state: ServiceConnectionState, old_state: ServiceConnectionState,
                              enter_reconnecting_func, enter_unreachable_func,
                              enter_disconnecting_func, enter_healthy_func):
        """状态进入时的处理逻辑"""
        if new_state == ServiceConnectionState.RECONNECTING:
            await enter_reconnecting_func(agent_id, service_name)
        elif new_state == ServiceConnectionState.UNREACHABLE:
            await enter_unreachable_func(agent_id, service_name)
        elif new_state == ServiceConnectionState.DISCONNECTING:
            await enter_disconnecting_func(agent_id, service_name)
        elif new_state == ServiceConnectionState.HEALTHY:
            await enter_healthy_func(agent_id, service_name)
    
    def calculate_reconnect_delay(self, reconnect_attempts: int) -> float:
        """计算重连延迟（指数退避）"""
        delay = min(self.config.base_reconnect_delay * (2 ** reconnect_attempts), 
                   self.config.max_reconnect_delay)
        return delay
    
    def should_retry_now(self, metadata: ServiceStateMetadata) -> bool:
        """判断是否应该立即重试"""
        if not metadata.next_retry_time:
            return True
        return datetime.now() >= metadata.next_retry_time
