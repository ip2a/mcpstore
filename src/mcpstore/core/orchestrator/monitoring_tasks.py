"""
MCPOrchestrator Monitoring Tasks Module
Monitoring tasks module - contains monitoring loops and task management
"""

import asyncio
import logging
from typing import Dict, List, Any, Optional, Tuple

from mcpstore.core.lifecycle import HealthStatus
from mcpstore.core.lifecycle.health_bridge import HealthStatusBridge

logger = logging.getLogger(__name__)

class MonitoringTasksMixin:
    """Monitoring tasks mixin class"""

    async def cleanup(self):
        """Clean up orchestrator resources"""
        logger.info("Cleaning up MCP Orchestrator...")

        # Stop tool update monitor
        if self.tools_update_monitor:
            await self.tools_update_monitor.stop()

        # Clean up local services
        if hasattr(self, 'local_service_manager'):
            await self.local_service_manager.cleanup()

        # Close all client connections
        for name, client in self.clients.items():
            try:
                await client.close()
                logger.debug(f"Closed client connection for {name}")
            except Exception as e:
                logger.warning(f"Error closing client {name}: {e}")

        self.clients.clear()
        logger.info("MCP Orchestrator cleanup completed")

    async def start_monitoring(self):
        """
        Start monitoring tasks - refactored to use ServiceLifecycleManager
        Old heartbeat, reconnection, cleanup tasks have been replaced by lifecycle manager
        """
        logger.info("Monitoring is now handled by ServiceLifecycleManager")
        logger.info("Legacy heartbeat and reconnection tasks have been disabled")

        # Only start tool update monitor (this still needs to be retained)
        if self.tools_update_monitor:
            await self.tools_update_monitor.start()
            logger.info("Tools update monitor started")

        return True

    async def _check_single_service_health(self, name: str, client_id: str) -> bool:
        """检查单个服务的健康状态并更新生命周期状态"""
        try:
            # 执行详细健康检查
            health_result = await self.check_service_health_detailed(name, client_id)
            is_healthy = health_result.status != HealthStatus.UNHEALTHY

            # 🆕 使用增强版健康检查处理，传递完整的状态信息
            try:
                suggested_state = HealthStatusBridge.map_health_to_lifecycle(health_result.status)
                
                # 使用增强版方法传递丰富的状态信息
                await self.lifecycle_manager.handle_health_check_result_enhanced(
                    agent_id=client_id,
                    service_name=name,
                    suggested_state=suggested_state,
                    response_time=health_result.response_time,
                    error_message=health_result.error_message
                )

                if is_healthy:
                    logger.debug(f"Health check SUCCESS for: {name} (client_id={client_id}), mapped to: {suggested_state.value}")
                    return True
                else:
                    logger.debug(f"Health check FAILED for {name} (client_id={client_id}): {health_result.error_message}, mapped to: {suggested_state.value}")
                    return False
            
            except ValueError as mapping_error:
                # 状态映射失败，回退到原有方法
                logger.warning(f"Health status mapping failed for {name}: {mapping_error}, falling back to legacy method")
                await self.lifecycle_manager.handle_health_check_result(
                    agent_id=client_id,
                    service_name=name,
                    success=is_healthy,
                    response_time=health_result.response_time,
                    error_message=health_result.error_message
                )
                return is_healthy

        except Exception as e:
            logger.warning(f"Health check error for {name} (client_id={client_id}): {e}")
            # 对于异常情况，仍使用原有方法
            await self.lifecycle_manager.handle_health_check_result(
                agent_id=client_id,
                service_name=name,
                success=False,
                response_time=0.0,
                error_message=str(e)
            )
            return False




    async def _restart_monitoring_tasks(self):
        """重启监控任务"""
        try:
            logger.info("Restarting monitoring tasks...")
            
            # 重启生命周期管理器
            if hasattr(self, 'lifecycle_manager') and self.lifecycle_manager:
                await self.lifecycle_manager.restart()
                logger.info("Lifecycle manager restarted")
            
            # 重启内容管理器
            if hasattr(self, 'content_manager') and self.content_manager:
                await self.content_manager.restart()
                logger.info("Content manager restarted")
            
            # 重启工具更新监控器
            if self.tools_update_monitor:
                await self.tools_update_monitor.restart()
                logger.info("Tools update monitor restarted")
            
            logger.info("All monitoring tasks restarted successfully")
            
        except Exception as e:
            logger.error(f"Failed to restart monitoring tasks: {e}")
            raise

