"""
MCPStore Advanced Features Module
Implementation of advanced feature-related operations
"""

import logging
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .base_context import MCPStoreContext

logger = logging.getLogger(__name__)

class AdvancedFeaturesMixin:
    """Advanced features mixin class"""
    
    def import_api(self, api_url: str, api_name: str = None) -> 'MCPStoreContext':
        """
        导入 OpenAPI 服务（同步）
        
        Args:
            api_url: API 规范 URL
            api_name: API 名称（可选）
            
        Returns:
            MCPStoreContext: 支持链式调用
        """
        return self._run_async_via_bridge(
            self.import_api_async(api_url, api_name),
            op_name="advanced_features.import_api"
        )

    async def import_api_async(self, api_url: str, api_name: str = None) -> 'MCPStoreContext':
        """
        导入 OpenAPI 服务（异步）

        Args:
            api_url: API 规范 URL
            api_name: API 名称（可选）

        Returns:
            MCPStoreContext: 支持链式调用
        """
        try:
            import time
            api_name = api_name or f"api_{int(time.time())}"
            result = await self._openapi_manager.import_openapi_service(
                name=api_name,
                spec_url=api_url
            )
            logger.info(f"[{self._context_type.value}] Imported API {api_name}: {result.get('total_endpoints', 0)} endpoints")
            return self
        except Exception as e:
            logger.error(f"[{self._context_type.value}] Failed to import API {api_url}: {e}")
            return self


    def reset_mcp_json_file(self) -> bool:
        """重置MCP JSON配置文件（同步版本）- 缓存优先模式"""
        return self._run_async_via_bridge(
            self.reset_mcp_json_file_async(),
            op_name="advanced_features.reset_mcp_json_file",
            timeout=60.0
        )

    async def reset_mcp_json_file_async(self, scope: str = "all") -> bool:
        """
        重置MCP JSON配置文件（异步版本）- 单一数据源架构

        Args:
            scope: 重置范围
                - "all": 重置整个mcp.json（清空所有服务）
                - "global_agent_store": 只清空Store级别的服务，保留Agent服务
                - agent_id: 只清空指定Agent的服务

        新架构逻辑：
        1. 根据scope确定要清理的缓存范围
        2. 同步更新mcp.json文件
        3. 触发缓存重新同步（可选）
        """
        try:
            logger.info(f" [MCP_RESET] Starting MCP JSON file reset with scope: {scope}")

            # 使用 UnifiedConfigManager 读取配置（从缓存）
            current_config = self._store._unified_config.get_mcp_config()
            mcp_servers = current_config.get("mcpServers", {})
            
            if scope == "all":
                # 重置整个mcp.json
                logger.info(" [MCP_RESET] Clearing all services from mcp.json")
                
                # 1. 清空所有缓存（通过Registry异步API）
                try:
                    agent_ids = await self._store.registry.get_all_agent_ids_async()
                except Exception:
                    agent_ids = []
                for agent_id in agent_ids:
                    try:
                        await self._store.registry.clear_async(agent_id)
                    except Exception as e:
                        logger.warning(f"Failed to clear agent {agent_id}: {e}")
                
                # 2. 重置mcp.json为空
                new_config = {"mcpServers": {}}
                
            elif scope == "global_agent_store":
                # 只清空Store级别的服务，保留Agent服务
                logger.info(" [MCP_RESET] Clearing Store services, preserving Agent services")
                
                # 1. 清空global_agent_store缓存（使用异步版本）
                global_agent_store_id = self._store.client_manager.global_agent_store_id
                await self._store.registry.clear_async(global_agent_store_id)
                
                # 2. 从mcp.json中移除非Agent服务（不带@后缀的服务）
                preserved_services = {}
                for service_name, service_config in mcp_servers.items():
                    if "@" in service_name:  # Agent服务（带@agent_id后缀）
                        preserved_services[service_name] = service_config
                
                new_config = {"mcpServers": preserved_services}
                logger.info(f" [MCP_RESET] Preserved {len(preserved_services)} Agent services")
                
            else:
                # 清空指定Agent的服务
                agent_id = scope
                logger.info(f" [MCP_RESET] Clearing services for Agent: {agent_id}")
                
                # 1. 清空该Agent的缓存（使用异步版本）
                await self._store.registry.clear_async(agent_id)
                
                # 2. 从mcp.json中移除该Agent的服务
                preserved_services = {}
                agent_suffix = f"@{agent_id}"
                
                for service_name, service_config in mcp_servers.items():
                    if not service_name.endswith(agent_suffix):
                        preserved_services[service_name] = service_config
                
                new_config = {"mcpServers": preserved_services}
                removed_count = len(mcp_servers) - len(preserved_services)
                logger.info(f" [MCP_RESET] Removed {removed_count} services for Agent {agent_id}")

            # 3. 保存更新后的mcp.json（使用 UnifiedConfigManager 自动刷新缓存）
            mcp_success = self._store._unified_config.update_mcp_config(new_config)
            
            if mcp_success:
                logger.info(f"[MCP_RESET] [COMPLETE] MCP JSON file reset completed for scope: {scope}, cache synchronized")

                # 4. 触发重新同步（可选）
                if hasattr(self._store.orchestrator, 'sync_manager') and self._store.orchestrator.sync_manager:
                    logger.info(" [MCP_RESET] Triggering cache resync from mcp.json")
                    await self._store.orchestrator.sync_manager.sync_global_agent_store_from_mcp_json()
            else:
                logger.error(f" [MCP_RESET] Failed to save mcp.json for scope: {scope}")
            
            return mcp_success

        except Exception as e:
            logger.error(f" [MCP_RESET] Failed to reset MCP JSON file with scope {scope}: {e}")
            return False
