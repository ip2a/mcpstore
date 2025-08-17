"""
MCPOrchestrator Monitoring Tasks Module
Monitoring tasks module - contains monitoring loops and task management
"""

import asyncio
import logging
from typing import Dict, List, Any, Optional, Tuple

from mcpstore.core.lifecycle import HealthStatus

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

            # 旧的健康状态更新已废弃，现在完全由生命周期管理器处理

            # 通知生命周期管理器处理健康检查结果
            await self.lifecycle_manager.handle_health_check_result(
                agent_id=client_id,
                service_name=name,
                success=is_healthy,
                response_time=health_result.response_time,
                error_message=health_result.error_message
            )

            if is_healthy:
                logger.debug(f"Health check SUCCESS for: {name} (client_id={client_id})")
                return True
            else:
                logger.debug(f"Health check FAILED for {name} (client_id={client_id}): {health_result.error_message}")
                return False

        except Exception as e:
            logger.warning(f"Health check error for {name} (client_id={client_id}): {e}")
            # 通知生命周期管理器处理错误
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

