"""
状态变化事件处理器
实现响应式状态管理，状态变化时立即触发处理
"""

import asyncio
import logging
from typing import Dict, Callable, Optional
from mcpstore.core.models.service import ServiceConnectionState

logger = logging.getLogger(__name__)


class StateChangeEventProcessor:
    """状态变化事件处理器"""
    
    def __init__(self, lifecycle_manager):
        self.lifecycle_manager = lifecycle_manager
        
        # 事件处理器映射
        self.event_handlers: Dict[ServiceConnectionState, Callable] = {
            ServiceConnectionState.INITIALIZING: self._handle_initializing_event,
            ServiceConnectionState.RECONNECTING: self._handle_reconnecting_event,
            ServiceConnectionState.UNREACHABLE: self._handle_unreachable_event,
        }
        
        logger.info("StateChangeEventProcessor initialized")
    
    async def on_state_change(self, agent_id: str, service_name: str, 
                            old_state: ServiceConnectionState, 
                            new_state: ServiceConnectionState):
        """状态变化事件处理入口"""
        logger.debug(f" [EVENT] 服务{service_name}状态变化: {old_state} → {new_state}")
        
        # 立即处理需要快速响应的状态
        if new_state in self.event_handlers:
            # 异步处理，不阻塞状态转换
            asyncio.create_task(
                self.event_handlers[new_state](agent_id, service_name, old_state)
            )
    
    async def _handle_initializing_event(self, agent_id: str, service_name: str, old_state: ServiceConnectionState):
        """处理INITIALIZING状态事件"""
        logger.debug(f"🚀 [EVENT_INIT] 响应INITIALIZING状态变化: {service_name}")
        
        # 触发快速处理器立即处理
        if hasattr(self.lifecycle_manager, 'initializing_processor'):
            await self.lifecycle_manager.initializing_processor.trigger_immediate_processing(
                agent_id, service_name
            )
        else:
            # 回退到直接处理
            logger.debug(f"🔧 [EVENT_INIT] 快速处理器不可用，使用直接处理: {service_name}")
            asyncio.create_task(
                self._direct_initializing_processing(agent_id, service_name)
            )
    
    async def _handle_reconnecting_event(self, agent_id: str, service_name: str, old_state: ServiceConnectionState):
        """处理RECONNECTING状态事件"""
        logger.debug(f" [EVENT_RECONNECT] 响应RECONNECTING状态变化: {service_name}")
        
        # 添加到生命周期管理器的处理队列
        self.lifecycle_manager.state_change_queue.add((agent_id, service_name))
    
    async def _handle_unreachable_event(self, agent_id: str, service_name: str, old_state: ServiceConnectionState):
        """处理UNREACHABLE状态事件"""
        logger.debug(f" [EVENT_UNREACHABLE] 响应UNREACHABLE状态变化: {service_name}")
        
        # 添加到生命周期管理器的处理队列
        self.lifecycle_manager.state_change_queue.add((agent_id, service_name))
    
    async def _direct_initializing_processing(self, agent_id: str, service_name: str):
        """直接处理INITIALIZING状态（回退方案）"""
        try:
            logger.debug(f"🔧 [EVENT_DIRECT] 直接处理INITIALIZING: {service_name}")
            
            await asyncio.wait_for(
                self.lifecycle_manager._attempt_initial_connection(agent_id, service_name),
                timeout=3.0
            )
            
        except asyncio.TimeoutError:
            logger.warning(f"⏰ [EVENT_DIRECT] {service_name}连接超时，转为DISCONNECTED")
            await self.lifecycle_manager._transition_to_state(
                agent_id, service_name, ServiceConnectionState.DISCONNECTED
            )
        except Exception as e:
            logger.error(f"❌ [EVENT_DIRECT] {service_name}连接失败: {e}")
            await self.lifecycle_manager._transition_to_state(
                agent_id, service_name, ServiceConnectionState.DISCONNECTED
            )
