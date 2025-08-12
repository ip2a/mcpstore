"""
FastMCP Message Handler
Handles notification messages from FastMCP servers
"""

import logging
from datetime import datetime
from typing import List, Dict, Any, Optional

logger = logging.getLogger(__name__)

# Check FastMCP availability
try:
    import mcp.types
    FASTMCP_AVAILABLE = True
    logger.debug("FastMCP is available for notification handling")
except ImportError:
    logger.warning("FastMCP not available, notification features will be disabled")
    FASTMCP_AVAILABLE = False


class MCPStoreMessageHandler:
    """MCPStore-specific FastMCP message handler"""

    def __init__(self, tools_monitor):
        """
        Initialize message handler

        Args:
            tools_monitor: ToolsUpdateMonitor instance
        """
        if not FASTMCP_AVAILABLE:
            logger.warning("FastMCP not available, notification features disabled")
            return

        self.tools_monitor = tools_monitor
        self.notification_history = []
        self.max_history = 100

    async def on_tool_list_changed(self, notification: 'mcp.types.ToolListChangedNotification') -> None:
        """Handle tool list change notifications"""
        if not FASTMCP_AVAILABLE:
            return

        logger.info("🔔 Received tools/list_changed notification from FastMCP server")

        # Record notification history
        self._record_notification("tools_changed", notification)

        # Trigger immediate update
        try:
            await self.tools_monitor.handle_notification_trigger("tools_changed")
        except Exception as e:
            logger.error(f"Error handling tools/list_changed notification: {e}")

    async def on_resource_list_changed(self, notification: 'mcp.types.ResourceListChangedNotification') -> None:
        """处理资源列表变更通知"""
        if not FASTMCP_AVAILABLE:
            return

        logger.info("🔔 Received resources/list_changed notification from FastMCP server")

        # 记录通知历史
        self._record_notification("resources_changed", notification)

        # TODO: 触发资源更新 - 后续版本实现
        # 当前版本仅记录通知，不触发实际更新
        try:
            # await self.tools_monitor.handle_notification_trigger("resources_changed")
            logger.debug("Resources notification received but update not implemented yet")
        except Exception as e:
            logger.error(f"Error handling resources/list_changed notification: {e}")

    async def on_prompt_list_changed(self, notification: 'mcp.types.PromptListChangedNotification') -> None:
        """处理提示词列表变更通知"""
        if not FASTMCP_AVAILABLE:
            return

        logger.info("🔔 Received prompts/list_changed notification from FastMCP server")

        # 记录通知历史
        self._record_notification("prompts_changed", notification)

        # TODO: 触发提示词更新 - 后续版本实现
        # 当前版本仅记录通知，不触发实际更新
        try:
            # await self.tools_monitor.handle_notification_trigger("prompts_changed")
            logger.debug("Prompts notification received but update not implemented yet")
        except Exception as e:
            logger.error(f"Error handling prompts/list_changed notification: {e}")

    def _record_notification(self, notification_type: str, notification: Any):
        """记录通知历史"""
        if not FASTMCP_AVAILABLE:
            return

        record = {
            "type": notification_type,
            "timestamp": datetime.now().isoformat(),
            "notification": notification
        }

        self.notification_history.append(record)

        # 保持历史记录在限制范围内
        if len(self.notification_history) > self.max_history:
            self.notification_history = self.notification_history[-self.max_history:]

        logger.debug(f"Recorded {notification_type} notification, history size: {len(self.notification_history)}")

    def get_notification_history(self, notification_type: Optional[str] = None, limit: int = 50) -> List[Dict[str, Any]]:
        """
        获取通知历史

        Args:
            notification_type: 通知类型过滤器，None表示所有类型
            limit: 返回记录数限制

        Returns:
            List[Dict]: 通知历史记录
        """
        if not FASTMCP_AVAILABLE:
            return []

        history = self.notification_history

        # 按类型过滤
        if notification_type:
            history = [record for record in history if record["type"] == notification_type]

        # 按时间倒序排列并限制数量
        history = sorted(history, key=lambda x: x["timestamp"], reverse=True)
        return history[:limit]

    def clear_notification_history(self, notification_type: Optional[str] = None):
        """
        清理通知历史

        Args:
            notification_type: 要清理的通知类型，None表示清理所有
        """
        if not FASTMCP_AVAILABLE:
            return

        if notification_type:
            self.notification_history = [
                record for record in self.notification_history 
                if record["type"] != notification_type
            ]
            logger.debug(f"Cleared {notification_type} notification history")
        else:
            self.notification_history.clear()
            logger.debug("Cleared all notification history")

    def get_notification_stats(self) -> Dict[str, Any]:
        """
        获取通知统计信息

        Returns:
            Dict: 统计信息
        """
        if not FASTMCP_AVAILABLE:
            return {"fastmcp_available": False}

        stats = {
            "fastmcp_available": True,
            "total_notifications": len(self.notification_history),
            "by_type": {},
            "recent_activity": []
        }

        # 按类型统计
        for record in self.notification_history:
            notification_type = record["type"]
            if notification_type not in stats["by_type"]:
                stats["by_type"][notification_type] = 0
            stats["by_type"][notification_type] += 1

        # 最近活动（最近10条）
        recent = sorted(self.notification_history, key=lambda x: x["timestamp"], reverse=True)[:10]
        stats["recent_activity"] = [
            {
                "type": record["type"],
                "timestamp": record["timestamp"]
            }
            for record in recent
        ]

        return stats
