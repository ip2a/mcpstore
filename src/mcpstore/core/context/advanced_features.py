"""
MCPStore Advanced Features Module
Implementation of advanced feature-related operations
"""

import logging
from typing import Dict, List, Optional, Any, TYPE_CHECKING

if TYPE_CHECKING:
    from .base_context import MCPStoreContext

logger = logging.getLogger(__name__)

class AdvancedFeaturesMixin:
    """Advanced features mixin class"""
    
    def create_simple_tool(self, original_tool: str, friendly_name: Optional[str] = None) -> 'MCPStoreContext':
        """
        Create simplified version of tool

        Args:
            original_tool: Original tool name
            friendly_name: Friendly name (optional)

        Returns:
            MCPStoreContext: Supports method chaining
        """
        try:
            friendly_name = friendly_name or f"simple_{original_tool}"
            result = self._transformation_manager.create_simple_tool(
                original_tool=original_tool,
                friendly_name=friendly_name
            )
            logger.info(f"[{self._context_type.value}] Created simple tool: {friendly_name} -> {original_tool}")
            return self
        except Exception as e:
            logger.error(f"[{self._context_type.value}] Failed to create simple tool {original_tool}: {e}")
            return self

    def create_safe_tool(self, original_tool: str, validation_rules: Dict[str, Any]) -> 'MCPStoreContext':
        """
        Create secure version of tool (with validation)

        Args:
            original_tool: Original tool name
            validation_rules: Validation rules

        Returns:
            MCPStoreContext: Supports method chaining
        """
        try:
            # 创建验证函数
            validation_func = self._create_validation_function(validation_rules)
            
            result = self._transformation_manager.create_safe_tool(
                original_tool=original_tool,
                validation_func=validation_func,
                rules=validation_rules
            )
            logger.info(f"[{self._context_type.value}] Created safe tool for: {original_tool}")
            return self
        except Exception as e:
            logger.error(f"[{self._context_type.value}] Failed to create safe tool {original_tool}: {e}")
            return self


    def import_api(self, api_url: str, api_name: str = None) -> 'MCPStoreContext':
        """
        导入 OpenAPI 服务（同步）
        
        Args:
            api_url: API 规范 URL
            api_name: API 名称（可选）
            
        Returns:
            MCPStoreContext: 支持链式调用
        """
        return self._sync_helper.run_async(self.import_api_async(api_url, api_name))

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


    # Deprecated: auth subsystem removed
    # Keeping the name for compatibility but make it explicit unsupported to avoid hidden AttributeError
    def setup_auth(self, auth_type: str = "bearer", enabled: bool = True) -> 'MCPStoreContext':
        raise NotImplementedError("setup_auth is no longer supported; the auth subsystem was removed")

    def get_usage_stats(self) -> Dict[str, Any]:
        """
        获取使用统计
        
        Returns:
            Dict: 使用统计信息
        """
        try:
            return self._monitoring_manager.get_usage_stats()
        except Exception as e:
            logger.error(f"[{self._context_type.value}] Failed to get usage stats: {e}")
            return {"error": str(e)}

    def record_tool_execution(self, tool_name: str, duration: float, success: bool, error: Exception = None) -> 'MCPStoreContext':
        """
        记录工具执行情况
        
        Args:
            tool_name: 工具名称
            duration: 执行时长
            success: 是否成功
            error: 错误信息（如果有）
            
        Returns:
            MCPStoreContext: 支持链式调用
        """
        try:
            self._monitoring_manager.record_tool_execution(
                tool_name=tool_name,
                duration=duration,
                success=success,
                error=error
            )
            return self
        except Exception as e:
            logger.error(f"[{self._context_type.value}] Failed to record tool execution: {e}")
            return self

    def reset_mcp_json_file(self) -> bool:
        """重置MCP JSON配置文件（同步版本）- 缓存优先模式"""
        return self._sync_helper.run_async(self.reset_mcp_json_file_async(), timeout=60.0)

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
                
                # 1. 清空所有缓存
                self._store.registry.agent_clients.clear()
                self._store.registry.client_configs.clear()
                self._store.registry.sessions.clear()
                self._store.registry.tool_cache.clear()
                self._store.registry.tool_to_session_map.clear()
                self._store.registry.service_states.clear()
                self._store.registry.service_metadata.clear()
                self._store.registry.service_to_client.clear()
                
                # 2. 重置mcp.json为空
                new_config = {"mcpServers": {}}
                
            elif scope == "global_agent_store":
                # 只清空Store级别的服务，保留Agent服务
                logger.info(" [MCP_RESET] Clearing Store services, preserving Agent services")
                
                # 1. 清空global_agent_store缓存
                global_agent_store_id = self._store.client_manager.global_agent_store_id
                self._store.registry.clear(global_agent_store_id)
                
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
                
                # 1. 清空该Agent的缓存
                self._store.registry.clear(agent_id)
                
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
                logger.info(f" [MCP_RESET] MCP JSON file reset completed for scope: {scope}，缓存已同步")
                
                # 4. 触发重新同步（可选）
                # 4.1 强一致：触发快照更新
                try:
                    gid = self._store.client_manager.global_agent_store_id
                    self._store.registry.tools_changed(gid, aggressive=True)
                except Exception:
                    try:
                        self._store.registry.mark_tools_snapshot_dirty()
                    except Exception:
                        pass

                if hasattr(self._store.orchestrator, 'sync_manager') and self._store.orchestrator.sync_manager:
                    logger.info(" [MCP_RESET] Triggering cache resync from mcp.json")
                    await self._store.orchestrator.sync_manager.sync_global_agent_store_from_mcp_json()
            else:
                logger.error(f" [MCP_RESET] Failed to save mcp.json for scope: {scope}")
            
            return mcp_success

        except Exception as e:
            logger.error(f" [MCP_RESET] Failed to reset MCP JSON file with scope {scope}: {e}")
            return False


