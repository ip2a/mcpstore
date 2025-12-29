"""
持久化管理器 - 负责文件持久化

职责:
1. 监听 ServiceAddRequested 事件
2. 异步持久化到文件（不阻塞）
3. 发布 ServicePersisted 事件
"""

import asyncio
import logging
from typing import Dict, Any, TYPE_CHECKING

from mcpstore.core.events.event_bus import EventBus
from mcpstore.core.events.service_events import ServiceAddRequested, ServicePersisted

if TYPE_CHECKING:
    from mcpstore.core.configuration.unified_config import UnifiedConfigManager

logger = logging.getLogger(__name__)


class PersistenceManager:
    """
    持久化管理器
    
    职责:
    1. 监听 ServiceAddRequested 事件
    2. 异步持久化到文件（不阻塞）
    3. 发布 ServicePersisted 事件
    """
    
    def __init__(self, event_bus: EventBus, config_manager: 'UnifiedConfigManager'):
        self._event_bus = event_bus
        self._config_manager = config_manager
        self._persistence_lock = asyncio.Lock()
        
        # 订阅事件（低优先级，不阻塞主流程）
        self._event_bus.subscribe(ServiceAddRequested, self._on_service_add_requested, priority=10)
        
        logger.info("PersistenceManager initialized and subscribed to events")
    
    async def _on_service_add_requested(self, event: ServiceAddRequested):
        """
        处理服务添加请求 - 异步持久化
        """
        logger.info(f"[PERSISTENCE] Persisting service: {event.service_name}")
        
        try:
            async with self._persistence_lock:
                # 持久化到 mcp.json
                await self._persist_to_mcp_json(event.service_name, event.service_config)
            
            logger.info(f"[PERSISTENCE] Service persisted: {event.service_name}")
            
            # 发布持久化完成事件
            persisted_event = ServicePersisted(
                agent_id=event.agent_id,
                service_name=event.service_name,
                file_path="mcp.json"
            )
            await self._event_bus.publish(persisted_event)
            
        except Exception as e:
            logger.error(f"[PERSISTENCE] Failed to persist {event.service_name}: {e}", exc_info=True)
            # 持久化失败不影响主流程，只记录日志
    
    async def _persist_to_mcp_json(self, service_name: str, service_config: Dict[str, Any]):
        """持久化到 mcp.json"""
        # 🆕 修复：UnifiedConfigManager 的 load_config/save_config 方法在 mcp_config 对象上
        # 读取当前配置
        current_config = self._config_manager.mcp_config.load_config()

        # 更新配置
        if "mcpServers" not in current_config:
            current_config["mcpServers"] = {}

        current_config["mcpServers"][service_name] = service_config

        # 保存配置
        success = self._config_manager.mcp_config.save_config(current_config)

        if not success:
            raise RuntimeError("Failed to save config to mcp.json")

