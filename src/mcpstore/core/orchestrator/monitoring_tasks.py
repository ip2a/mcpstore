"""
MCPOrchestrator Monitoring Tasks Module
Monitoring tasks module - contains monitoring loops and task management
"""

import logging

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

    # 🆕 事件驱动架构：_check_single_service_health 方法已被废弃并删除
    # 健康检查功能已由 HealthMonitor 接管




    async def _restart_monitoring_tasks(self):
        """重启监控任务"""
        try:
            logger.info("Restarting monitoring tasks...")

            # 🆕 事件驱动架构：lifecycle_manager 和 content_manager 已被设置为 None
            # 这些检查会失败，不会执行重启逻辑
            # 新架构中，ServiceContainer 负责管理所有组件的生命周期

            # 重启生命周期管理器（已废弃）
            if hasattr(self, 'lifecycle_manager') and self.lifecycle_manager:
                await self.lifecycle_manager.restart()
                logger.info("Lifecycle manager restarted")

            # 重启内容管理器（已废弃）
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

