"""
配置管理模块
负责处理 MCPStore 的配置相关功能
"""

import logging
from typing import Optional, Dict, Any

from mcpstore.core.configuration.unified_config import UnifiedConfigManager
from mcpstore.core.models.common import ConfigResponse

logger = logging.getLogger(__name__)


class ConfigManagementMixin:
    """配置管理 Mixin"""
    
    def get_unified_config(self) -> UnifiedConfigManager:
        """Get unified configuration manager

        Returns:
            UnifiedConfigManager: Unified configuration manager instance
        """
        return self._unified_config

    def get_json_config(self, client_id: Optional[str] = None) -> ConfigResponse:
        """查询服务配置，等价于 GET /register/json（优化：使用缓存）"""
        if not client_id or client_id == self.client_manager.global_agent_store_id:
            # 使用 UnifiedConfigManager 读取配置（从缓存，更高效）
            config = self._unified_config.get_mcp_config()
            return ConfigResponse(
                success=True,
                client_id=self.client_manager.global_agent_store_id,
                config=config
            )
        else:
            config = self.client_manager.get_client_config(client_id)
            if not config:
                raise ValueError(f"Client configuration not found: {client_id}")
            return ConfigResponse(
                success=True,
                client_id=client_id,
                config=config
            )

    def show_mcpjson(self) -> Dict[str, Any]:
        # TODO:show_mcpjson和get_json_config是否有一定程度的重合
        """
        直接读取并返回 mcp.json 文件的内容（优化：使用缓存）

        Returns:
            Dict[str, Any]: mcp.json 文件的内容
        """
        # 使用 UnifiedConfigManager 读取配置（从缓存，更高效）
        return self._unified_config.get_mcp_config()

    async def _sync_discovered_agents_to_files(self, agents_discovered: set):
        """
         单一数据源架构：不再同步到分片文件
        
        新架构下，Agent发现只需要更新缓存，所有持久化通过mcp.json完成
        """
        try:
            # logger.info(f" [SYNC_AGENTS] 单一数据源模式：跳过分片文件同步，已发现 {len(agents_discovered)} 个 Agent")
            
            # 单一数据源模式：不再写入分片文件，仅维护缓存和mcp.json
            # logger.info(" [SYNC_AGENTS] 单一数据源模式：Agent发现完成，缓存已更新")
            pass
        except Exception as e:
            # logger.error(f" [SYNC_AGENTS] Agent 同步失败: {e}")
            raise
