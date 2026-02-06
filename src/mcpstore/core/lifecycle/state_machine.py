"""
Service Lifecycle State Machine
Responsible for handling service state transition logic
"""

import logging
from datetime import datetime

from mcpstore.core.models.service import ServiceConnectionState, ServiceStateMetadata
from .config import ServiceLifecycleConfig

logger = logging.getLogger(__name__)


class ServiceStateMachine:
    """Service lifecycle state machine"""
    
    def __init__(self, config: ServiceLifecycleConfig):
        self.config = config
    
    async def transition_to_state(self, agent_id: str, service_name: str,
                                 new_state: ServiceConnectionState,
                                 get_state_func, get_metadata_func, 
                                 set_state_func, on_state_entered_func):
        """Execute state transition"""
        old_state = get_state_func(agent_id, service_name)
        logger.debug(f"[STATE_TRANSITION] attempting service='{service_name}' from={old_state} to={new_state}")

        if old_state == new_state:
            logger.debug(f"[STATE_TRANSITION] No change needed for {service_name}: already in {new_state}")
            return

        # Update state
        logger.debug(f"[STATE_TRANSITION] updating service='{service_name}' from={old_state} to={new_state}")
        set_state_func(agent_id, service_name, new_state)
        metadata = get_metadata_func(agent_id, service_name)
        if metadata:
            metadata.state_entered_time = datetime.now()
            logger.debug(f"[STATE_TRANSITION] updated_state_entered_time service='{service_name}'")
        else:
            logger.warning(f"[STATE_TRANSITION] no_metadata service='{service_name}' during_transition=True")

        # Execute state entry handling
        logger.debug(f"[STATE_TRANSITION] calling_on_state_entered service='{service_name}'")
        await on_state_entered_func(agent_id, service_name, new_state, old_state)

        logger.info(f"[STATE_TRANSITION] transitioned service='{service_name}' agent='{agent_id}' from={old_state} to={new_state}")
    
    async def on_state_entered(self, agent_id: str, service_name: str, 
                              new_state: ServiceConnectionState, old_state: ServiceConnectionState,
                              enter_reconnecting_func, enter_unreachable_func,
                              enter_disconnecting_func, enter_healthy_func):
        """State entry handling logic"""
        if new_state == ServiceConnectionState.CIRCUIT_OPEN:
            await enter_reconnecting_func(agent_id, service_name)
        elif new_state == ServiceConnectionState.DISCONNECTED:
            await enter_unreachable_func(agent_id, service_name)
        elif new_state == ServiceConnectionState.DISCONNECTED:
            await enter_disconnecting_func(agent_id, service_name)
        elif new_state == ServiceConnectionState.HEALTHY:
            await enter_healthy_func(agent_id, service_name)
    
    def should_retry_now(self, metadata: ServiceStateMetadata) -> bool:
        """Determine if should retry immediately"""
        if not metadata.next_retry_time:
            return True
        return datetime.now() >= metadata.next_retry_time
