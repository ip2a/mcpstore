"""
配置管理模块
负责处理 MCPStore 的配置相关功能
"""

from typing import Optional, Dict, Any
import logging

from mcpstore.core.unified_config import UnifiedConfigManager
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
        """查询服务配置，等价于 GET /register/json"""
        if not client_id or client_id == self.client_manager.global_agent_store_id:
            config = self.config.load_config()
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
        直接读取并返回 mcp.json 文件的内容

        Returns:
            Dict[str, Any]: mcp.json 文件的内容
        """
        return self.config.load_config()

    async def _sync_discovered_agents_to_files(self, agents_discovered: set):
        """将发现的 Agent 同步到持久化文件"""
        try:
            logger.info(f"🔄 [SYNC_AGENTS] 开始同步 {len(agents_discovered)} 个 Agent 到文件...")

            # 更新 agent_clients.json
            agent_clients_data = {}

            # 包含 global_agent_store
            global_client_ids = []
            for agent_id, service_mappings in self.registry.service_to_client.items():
                if agent_id == self.client_manager.global_agent_store_id:
                    global_client_ids = list(set(service_mappings.values()))
                    break

            if global_client_ids:
                agent_clients_data[self.client_manager.global_agent_store_id] = global_client_ids

            # 包含发现的 Agent
            for agent_id in agents_discovered:
                client_ids = []
                if agent_id in self.registry.service_to_client:
                    client_ids = list(set(self.registry.service_to_client[agent_id].values()))
                if client_ids:
                    agent_clients_data[agent_id] = client_ids

            self.client_manager.save_all_agent_clients(agent_clients_data)
            logger.info(f"✅ [SYNC_AGENTS] agent_clients.json 更新完成")

            # 更新 client_services.json
            client_configs_data = {}
            for client_id, config in self.registry.client_configs.items():
                client_configs_data[client_id] = config

            # 添加新发现的 client 配置
            for agent_id in agents_discovered:
                if agent_id in self.registry.service_to_client:
                    for service_name, client_id in self.registry.service_to_client[agent_id].items():
                        if client_id not in client_configs_data:
                            # 从 mcp.json 获取配置
                            store_config = self.config.load_config()
                            global_name = self.registry.get_global_name_from_agent_service(agent_id, service_name)
                            if global_name and global_name in store_config.get("mcpServers", {}):
                                client_configs_data[client_id] = {
                                    "mcpServers": {global_name: store_config["mcpServers"][global_name]}
                                }

            self.client_manager.save_all_clients(client_configs_data)
            logger.info(f"✅ [SYNC_AGENTS] client_services.json 更新完成")

        except Exception as e:
            logger.error(f"❌ [SYNC_AGENTS] Agent 同步失败: {e}")
            raise
