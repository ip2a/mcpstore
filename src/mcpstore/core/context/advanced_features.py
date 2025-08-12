"""
MCPStore Advanced Features Module
Implementation of advanced feature-related operations
"""

import logging
from typing import Dict, List, Optional, Any, Union

from .types import ContextType

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

    def switch_environment(self, environment: str) -> 'MCPStoreContext':
        """
        切换运行环境
        
        Args:
            environment: 环境名称（如 "development", "production"）
            
        Returns:
            MCPStoreContext: 支持链式调用
        """
        try:
            result = self._component_manager.switch_environment(environment)
            logger.info(f"[{self._context_type.value}] Switched to environment: {environment}")
            return self
        except Exception as e:
            logger.error(f"[{self._context_type.value}] Failed to switch environment to {environment}: {e}")
            return self

    def create_custom_environment(self, name: str, allowed_categories: List[str]) -> 'MCPStoreContext':
        """
        创建自定义环境
        
        Args:
            name: 环境名称
            allowed_categories: 允许的工具类别
            
        Returns:
            MCPStoreContext: 支持链式调用
        """
        try:
            result = self._component_manager.create_custom_environment(
                name=name,
                allowed_categories=allowed_categories
            )
            logger.info(f"[{self._context_type.value}] Created custom environment: {name}")
            return self
        except Exception as e:
            logger.error(f"[{self._context_type.value}] Failed to create custom environment {name}: {e}")
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

    def enable_caching(self, patterns: Dict[str, int] = None) -> 'MCPStoreContext':
        """
        启用缓存（工具结果缓存功能已移除）

        Args:
            patterns: 缓存模式配置（已废弃，工具结果缓存已移除）

        Returns:
            MCPStoreContext: 支持链式调用
        """
        try:
            logger.warning(f"[{self._context_type.value}] Tool result caching has been removed. This method is deprecated.")
            logger.info(f"[{self._context_type.value}] Only service discovery caching is still available.")
            result = self._performance_optimizer.enable_caching(patterns)
            return self
        except Exception as e:
            logger.error(f"[{self._context_type.value}] Failed to enable caching: {e}")
            return self

    def get_performance_report(self) -> Dict[str, Any]:
        """
        获取性能报告
        
        Returns:
            Dict: 性能统计信息
        """
        try:
            return self._performance_optimizer.get_performance_report()
        except Exception as e:
            logger.error(f"[{self._context_type.value}] Failed to get performance report: {e}")
            return {"error": str(e)}

    def setup_auth(self, auth_type: str = "bearer", enabled: bool = True) -> 'MCPStoreContext':
        """
        设置认证
        
        Args:
            auth_type: 认证类型
            enabled: 是否启用
            
        Returns:
            MCPStoreContext: 支持链式调用
        """
        try:
            result = self._auth_manager.setup_auth(
                auth_type=auth_type,
                enabled=enabled
            )
            logger.info(f"[{self._context_type.value}] Setup auth: {auth_type}, enabled: {enabled}")
            return self
        except Exception as e:
            logger.error(f"[{self._context_type.value}] Failed to setup auth: {e}")
            return self

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

    async def reset_mcp_json_file_async(self) -> bool:
        """
        重置MCP JSON配置文件（异步版本）- 缓存优先模式

        新逻辑：
        1. 清空global_agent_store在缓存中的数据
        2. 重置mcp.json文件
        3. 触发缓存同步到映射文件

        注意：这个方法只影响global_agent_store，不影响其他Agent
        """
        try:
            logger.info("🔄 Starting MCP JSON file reset with cache-first logic")

            # 1. 清空global_agent_store在缓存中的数据
            logger.info("Step 1: Clearing global_agent_store cache")
            global_agent_store_id = self._store.client_manager.global_agent_store_id
            self._store.registry.clear(global_agent_store_id)

            # 2. 重置mcp.json文件
            logger.info("Step 2: Resetting mcp.json file")
            default_config = {"mcpServers": {}}
            mcp_success = self._store.config.save_config(default_config)

            # 3. 触发缓存同步到映射文件
            logger.info("Step 3: Syncing cache to mapping files")
            if hasattr(self._store, 'cache_manager'):
                self._store.cache_manager.sync_to_client_manager(self._store.client_manager)
            else:
                self._store.registry.sync_to_client_manager(self._store.client_manager)

            logger.info("✅ MCP JSON file reset completed with cache-first logic")
            return mcp_success

        except Exception as e:
            logger.error(f"Failed to reset MCP JSON file with cache-first logic: {e}")
            return False

    def reset_client_services_file(self) -> bool:
        """直接重置client_services.json文件（同步版本）"""
        return self._sync_helper.run_async(self.reset_client_services_file_async(), timeout=60.0)

    async def reset_client_services_file_async(self) -> bool:
        """
        重置client_services.json文件（缓存优先逻辑）

        新逻辑：
        1. 先清空相关缓存
        2. 缓存自动同步到文件（应该清空文件）
        3. 保险起见，再直接清空文件
        """
        try:
            logger.info("🔄 Starting client_services file reset with cache-first logic")

            # 1. 清空相关缓存
            logger.info("Step 1: Clearing client configs cache")
            self._store.registry.client_configs.clear()

            # 2. 触发缓存到文件的同步（应该会清空文件）
            logger.info("Step 2: Syncing empty cache to file")
            if hasattr(self._store, 'cache_manager'):
                self._store.cache_manager.sync_to_client_manager(self._store.client_manager)
            else:
                # 备用方案
                self._store.registry.sync_to_client_manager(self._store.client_manager)

            # 3. 保险起见，直接清空文件
            logger.info("Step 3: Direct file reset as safety measure")
            file_success = self._store.client_manager.reset_client_services_file()

            logger.info("✅ Client services file reset completed with cache-first logic")
            return file_success

        except Exception as e:
            logger.error(f"Failed to reset client_services file with cache-first logic: {e}")
            return False

    def reset_agent_clients_file(self) -> bool:
        """直接重置agent_clients.json文件（同步版本）"""
        return self._sync_helper.run_async(self.reset_agent_clients_file_async(), timeout=60.0)

    async def reset_agent_clients_file_async(self) -> bool:
        """
        重置agent_clients.json文件（缓存优先逻辑）

        新逻辑：
        1. 先清空相关缓存
        2. 缓存自动同步到文件（应该清空文件）
        3. 保险起见，再直接清空文件
        """
        try:
            logger.info("🔄 Starting agent_clients file reset with cache-first logic")

            # 1. 清空相关缓存
            logger.info("Step 1: Clearing agent-client mappings cache")
            self._store.registry.agent_clients.clear()

            # 2. 触发缓存到文件的同步（应该会清空文件）
            logger.info("Step 2: Syncing empty cache to file")
            if hasattr(self._store, 'cache_manager'):
                self._store.cache_manager.sync_to_client_manager(self._store.client_manager)
            else:
                # 备用方案
                self._store.registry.sync_to_client_manager(self._store.client_manager)

            # 3. 保险起见，直接清空文件
            logger.info("Step 3: Direct file reset as safety measure")
            file_success = self._store.client_manager.reset_agent_clients_file()

            logger.info("✅ Agent clients file reset completed with cache-first logic")
            return file_success

        except Exception as e:
            logger.error(f"Failed to reset agent_clients file with cache-first logic: {e}")
            return False
