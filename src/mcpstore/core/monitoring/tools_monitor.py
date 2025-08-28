"""
Tool Update Monitor
Supports FastMCP notification mechanism + polling backup strategy
"""

import asyncio
import logging
import time
from datetime import datetime, timedelta
from typing import Dict, List, Optional, Any, Set

from .message_handler import MCPStoreMessageHandler, FASTMCP_AVAILABLE

logger = logging.getLogger(__name__)


class ToolsUpdateMonitor:
    """
    Hybrid tool list update monitor
    Supports FastMCP notification mechanism + polling backup strategy
    """

    def __init__(self, orchestrator):
        self.orchestrator = orchestrator
        self.registry = orchestrator.registry

        # Configuration parameters (obtained from orchestrator configuration)
        timing_config = orchestrator.config.get("timing", {})
        self.tools_update_interval = timing_config.get("tools_update_interval_seconds", 7200)  # Default 2 hours
        self.enable_tools_update = timing_config.get("enable_tools_update", True)
        self.update_tools_on_reconnection = timing_config.get("update_tools_on_reconnection", True)
        self.detect_tools_changes = timing_config.get("detect_tools_changes", False)

        # New: notification-related configuration
        notification_config = orchestrator.config.get("notifications", {})
        self.enable_notifications = notification_config.get("enable_notifications", True) and FASTMCP_AVAILABLE
        self.notification_debounce_seconds = notification_config.get("debounce_seconds", 5)
        self.notification_timeout_seconds = notification_config.get("timeout_seconds", 30)
        self.fallback_to_polling = notification_config.get("fallback_to_polling", True)

        # Status tracking
        self.last_update_times: Dict[str, float] = {}  # service_name -> timestamp
        self.last_notification_times: Dict[str, float] = {}  # Notification debouncing
        self.update_task: Optional[asyncio.Task] = None
        self.is_running = False

        # FastMCP message handler
        self.message_handler = None
        if self.enable_notifications:
            self.message_handler = MCPStoreMessageHandler(self)

        logger.info(f"ToolsUpdateMonitor initialized: interval={self.tools_update_interval}s, "
                   f"enabled={self.enable_tools_update}, reconnection_update={self.update_tools_on_reconnection}, "
                   f"notifications_enabled={self.enable_notifications}")

    def _update_service_timestamp(self, service_name: str, client_id: str):
        """更新服务的时间戳（统一方法）"""
        service_key = f"{client_id}:{service_name}"
        self.last_update_times[service_key] = time.time()

    def get_message_handler(self):
        """获取FastMCP消息处理器"""
        return self.message_handler

    async def handle_notification_trigger(self, notification_type: str) -> Dict[str, Any]:
        """
        处理通知触发的工具更新

        Args:
            notification_type: 通知类型 ("tools_changed", "resources_changed", etc.)

        Returns:
            Dict: 更新结果
        """
        if not self.enable_notifications:
            logger.debug("[TOOLS_MONITOR] notification disabled ignore")
            return {"changed": False, "trigger": "notification", "reason": "disabled"}

        # 防抖处理
        current_time = time.time()
        last_notification = self.last_notification_times.get(notification_type, 0)

        if current_time - last_notification < self.notification_debounce_seconds:
            logger.debug(f"Notification debounced for {notification_type}")
            return {"changed": False, "trigger": "notification", "reason": "debounced"}

        self.last_notification_times[notification_type] = current_time

        logger.info(f"[TOOLS_MONITOR] notification trigger type='{notification_type}'")

        try:
            # 执行立即更新
            result = await self.trigger_immediate_update()
            result["trigger"] = "notification"
            result["notification_type"] = notification_type
            
            logger.info(f"[TOOLS_MONITOR] notification update_completed result={result}")
            return result

        except Exception as e:
            logger.error(f"[TOOLS_MONITOR] notification error={e}")
            return {
                "changed": False,
                "trigger": "notification",
                "notification_type": notification_type,
                "error": str(e)
            }

    async def start(self):
        """启动工具更新监控"""
        if not self.enable_tools_update:
            logger.info("Tools update monitoring is disabled")
            return

        if self.is_running:
            logger.warning("ToolsUpdateMonitor is already running")
            return

        self.is_running = True
        
        try:
            loop = asyncio.get_running_loop()
            self.update_task = loop.create_task(self._update_loop())
            self.update_task.add_done_callback(self._task_done_callback)
            logger.info("ToolsUpdateMonitor started")
        except Exception as e:
            self.is_running = False
            logger.error(f"Failed to start ToolsUpdateMonitor: {e}")
            raise

    async def stop(self):
        """停止工具更新监控"""
        self.is_running = False
        
        if self.update_task and not self.update_task.done():
            logger.debug("Cancelling tools update task...")
            self.update_task.cancel()
            try:
                await self.update_task
            except asyncio.CancelledError:
                logger.debug("Tools update task was cancelled")
            except Exception as e:
                logger.error(f"Error during tools update task cancellation: {e}")
        
        logger.info("ToolsUpdateMonitor stopped")

    def _task_done_callback(self, task):
        """更新任务完成回调"""
        if task.cancelled():
            logger.info("Tools update task was cancelled")
        elif task.exception():
            logger.error(f"Tools update task failed: {task.exception()}")
        else:
            logger.info("Tools update task completed normally")
        
        self.is_running = False

    async def _update_loop(self):
        """工具更新主循环"""
        logger.info("Starting tools update loop")
        
        while self.is_running:
            try:
                # 执行定期更新
                await self._perform_scheduled_update()
                
                # 等待下一次更新
                await asyncio.sleep(self.tools_update_interval)
                
            except asyncio.CancelledError:
                logger.info("Tools update loop was cancelled")
                break
            except Exception as e:
                logger.error(f"❌ Error in tools update loop: {e}")
                # 继续运行，不要因为单次错误而停止整个循环
                await asyncio.sleep(60)  # 错误后等待1分钟再继续
        
        logger.info("Tools update loop ended")

    async def _perform_scheduled_update(self):
        """执行定期更新"""
        if not self.enable_tools_update:
            return

        logger.debug("[TOOLS_MONITOR] scheduled_update start")
        
        try:
            result = await self.trigger_immediate_update()
            result["trigger"] = "scheduled"
            
            if result.get("changed", False):
                logger.info(f"[TOOLS_MONITOR] scheduled_update changes result={result}")
            else:
                logger.debug(f"[TOOLS_MONITOR] scheduled_update no_changes result={result}")
                
        except Exception as e:
            logger.error(f"❌ Error during scheduled update: {e}")

    async def trigger_immediate_update(self) -> Dict[str, Any]:
        """
        触发立即更新所有服务的工具列表

        Returns:
            Dict: 更新结果摘要
        """
        if not self.enable_tools_update:
            return {"changed": False, "reason": "disabled"}

        logger.debug("[TOOLS_MONITOR] immediate_update start")
        start_time = time.time()
        
        # 获取所有活跃的服务
        all_services = []
        for client_id in self.registry.sessions:
            for service_name in self.registry.sessions[client_id]:
                all_services.append((client_id, service_name))
        
        if not all_services:
            logger.debug("[TOOLS_MONITOR] no_active_services")
            return {
                "changed": False,
                "reason": "no_services",
                "duration": time.time() - start_time,
                "timestamp": datetime.now().isoformat()
            }

        logger.debug(f"Found {len(all_services)} services to update")
        
        # 并发更新所有服务
        update_tasks = []
        for client_id, service_name in all_services:
            task = asyncio.create_task(
                self._update_service_tools(client_id, service_name)
            )
            update_tasks.append(task)
        
        # 等待所有更新完成
        results = await asyncio.gather(*update_tasks, return_exceptions=True)
        
        # 分析结果
        total_services = len(all_services)
        successful_updates = 0
        failed_updates = 0
        services_with_changes = 0
        total_changes = 0
        
        for i, result in enumerate(results):
            client_id, service_name = all_services[i]
            
            if isinstance(result, Exception):
                failed_updates += 1
                logger.error(f"[TOOLS_MONITOR] update_failed service='{service_name}' client='{client_id}' error={result}")
            elif isinstance(result, dict):
                successful_updates += 1
                if result.get("changed", False):
                    services_with_changes += 1
                    total_changes += result.get("changes_count", 0)
                    logger.info(f"[TOOLS_MONITOR] updated service='{service_name}' client='{client_id}' changes={result.get('changes_count', 0)}")
                else:
                    logger.debug(f"[TOOLS_MONITOR] no_changes service='{service_name}' client='{client_id}'")
            else:
                failed_updates += 1
                logger.error(f"[TOOLS_MONITOR] unexpected_result_type service='{service_name}' client='{client_id}' type={type(result)}")
        
        duration = time.time() - start_time
        
        summary = {
            "changed": services_with_changes > 0,
            "total_services": total_services,
            "successful_updates": successful_updates,
            "failed_updates": failed_updates,
            "services_with_changes": services_with_changes,
            "total_changes": total_changes,
            "duration": duration,
            "timestamp": datetime.now().isoformat()
        }
        
        logger.info(f"[TOOLS_MONITOR] immediate_update done summary={summary}")
        return summary

    async def _update_service_tools(self, client_id: str, service_name: str) -> Dict[str, Any]:
        """
        更新单个服务的工具列表

        Args:
            client_id: 客户端ID
            service_name: 服务名称

        Returns:
            Dict: 更新结果
        """
        try:
            logger.debug(f"[TOOLS_MONITOR] updating service='{service_name}' client='{client_id}'")

            # 获取客户端会话（统一从Registry缓存获取）
            client = self.registry.get_session(client_id, service_name)
            if not client:
                return {
                    "changed": False,
                    "error": f"No active session found for {service_name}",
                    "service_name": service_name,
                    "client_id": client_id
                }

            # 获取当前工具列表
            old_tools = set(self.registry.get_tools_for_service(client_id, service_name))

            # 从服务获取最新工具列表
            try:
                tools_response = await client.list_tools()
                new_tools = {tool.name for tool in tools_response}
            except Exception as e:
                logger.error(f"[TOOLS_MONITOR] list_tools_failed service='{service_name}' error={e}")
                return {
                    "changed": False,
                    "error": f"Failed to list tools: {str(e)}",
                    "service_name": service_name,
                    "client_id": client_id
                }

            # 比较工具列表
            added_tools = new_tools - old_tools
            removed_tools = old_tools - new_tools

            changes_count = len(added_tools) + len(removed_tools)

            if changes_count > 0:
                # 有变化，更新注册表
                logger.info(f" Tools changed for {service_name}: +{len(added_tools)} -{len(removed_tools)}")

                # 更新工具注册
                session = self.registry.sessions.get(client_id, {}).get(service_name)
                if session:
                    # 移除旧工具（映射）
                    for tool_name in removed_tools:
                        if client_id in self.registry.tool_to_session_map and tool_name in self.registry.tool_to_session_map[client_id]:
                            del self.registry.tool_to_session_map[client_id][tool_name]

                    # 添加新工具（映射）
                    if client_id not in self.registry.tool_to_session_map:
                        self.registry.tool_to_session_map[client_id] = {}

                    for tool_name in added_tools:
                        self.registry.tool_to_session_map[client_id][tool_name] = session

                    # 触发全量工具定义刷新，确保缓存定义同步
                    try:
                        await self.orchestrator.content_manager.force_update_service_content(client_id, service_name)
                    except Exception as refresh_err:
                        logger.warning(f"[TOOLS_MONITOR] content_refresh_failed service='{service_name}' error={refresh_err}")

                # 更新时间戳
                self._update_service_timestamp(service_name, client_id)

                return {
                    "changed": True,
                    "changes_count": changes_count,
                    "added_tools": list(added_tools),
                    "removed_tools": list(removed_tools),
                    "service_name": service_name,
                    "client_id": client_id,
                    "timestamp": datetime.now().isoformat()
                }
            else:
                # 无变化
                logger.debug(f"[TOOLS_MONITOR] no_tool_changes service='{service_name}'")
                return {
                    "changed": False,
                    "changes_count": 0,
                    "service_name": service_name,
                    "client_id": client_id
                }

        except Exception as e:
            logger.error(f"[TOOLS_MONITOR] update_error service='{service_name}' error={e}")
            return {
                "changed": False,
                "error": str(e),
                "service_name": service_name,
                "client_id": client_id
            }

    async def update_service_on_reconnection(self, client_id: str, service_name: str) -> Dict[str, Any]:
        """
        在服务重连后更新工具列表

        Args:
            client_id: 客户端ID
            service_name: 服务名称

        Returns:
            Dict: 更新结果
        """
        if not self.update_tools_on_reconnection:
            logger.debug(f"[TOOLS_MONITOR] reconnection_update_disabled service='{service_name}'")
            return {"changed": False, "reason": "disabled"}

        logger.info(f"[TOOLS_MONITOR] reconnection_update service='{service_name}'")

        try:
            result = await self._update_service_tools(client_id, service_name)
            result["trigger"] = "reconnection"

            if result.get("changed", False):
                logger.info(f"[TOOLS_MONITOR] reconnection_update changes result={result}")
            else:
                logger.debug(f"[TOOLS_MONITOR] reconnection_update no_changes service='{service_name}'")

            return result

        except Exception as e:
            logger.error(f"[TOOLS_MONITOR] reconnection_update_error service='{service_name}' error={e}")
            return {
                "changed": False,
                "error": str(e),
                "trigger": "reconnection",
                "service_name": service_name,
                "client_id": client_id
            }

    def get_update_status(self) -> Dict[str, Any]:
        """
        获取更新状态信息

        Returns:
            Dict: 状态信息
        """
        return {
            "is_running": self.is_running,
            "enabled": self.enable_tools_update,
            "update_interval": self.tools_update_interval,
            "notifications_enabled": self.enable_notifications,
            "fastmcp_available": FASTMCP_AVAILABLE,
            "last_update_times": dict(self.last_update_times),
            "services_count": len(self.last_update_times),
            "config": {
                "tools_update_interval": self.tools_update_interval,
                "enable_tools_update": self.enable_tools_update,
                "update_tools_on_reconnection": self.update_tools_on_reconnection,
                "detect_tools_changes": self.detect_tools_changes,
                "enable_notifications": self.enable_notifications,
                "notification_debounce_seconds": self.notification_debounce_seconds,
                "notification_timeout_seconds": self.notification_timeout_seconds,
                "fallback_to_polling": self.fallback_to_polling
            }
        }

    def get_notification_stats(self) -> Dict[str, Any]:
        """
        获取通知统计信息

        Returns:
            Dict: 通知统计
        """
        if self.message_handler:
            return self.message_handler.get_notification_stats()
        else:
            return {"fastmcp_available": False, "message_handler": None}

    def update_config(self, new_config: Dict[str, Any]):
        """
        更新监控配置

        Args:
            new_config: 新配置
        """
        timing_config = new_config.get("timing", {})
        notification_config = new_config.get("notifications", {})

        # 更新timing配置
        if "tools_update_interval_seconds" in timing_config:
            self.tools_update_interval = timing_config["tools_update_interval_seconds"]
        if "enable_tools_update" in timing_config:
            self.enable_tools_update = timing_config["enable_tools_update"]
        if "update_tools_on_reconnection" in timing_config:
            self.update_tools_on_reconnection = timing_config["update_tools_on_reconnection"]
        if "detect_tools_changes" in timing_config:
            self.detect_tools_changes = timing_config["detect_tools_changes"]

        # 更新notification配置
        if "enable_notifications" in notification_config:
            self.enable_notifications = notification_config["enable_notifications"] and FASTMCP_AVAILABLE
        if "debounce_seconds" in notification_config:
            self.notification_debounce_seconds = notification_config["debounce_seconds"]
        if "timeout_seconds" in notification_config:
            self.notification_timeout_seconds = notification_config["timeout_seconds"]
        if "fallback_to_polling" in notification_config:
            self.fallback_to_polling = notification_config["fallback_to_polling"]

        logger.info(f"[TOOLS_MONITOR] config_updated")

    def cleanup(self):
        """清理资源"""
        logger.debug("[TOOLS_MONITOR] cleanup start")

        # 清理状态数据
        self.last_update_times.clear()
        self.last_notification_times.clear()

        # 清理消息处理器
        if self.message_handler:
            self.message_handler.clear_notification_history()

        logger.info("[TOOLS_MONITOR] cleanup completed")
