"""
Configuration management module
Responsible for handling MCPStore configuration related functionality
"""

import logging
from typing import Optional, Dict, Any, Union

from mcpstore.core.configuration.unified_config import UnifiedConfigManager
from mcpstore.core.models.common import ConfigResponse

logger = logging.getLogger(__name__)


class ConfigManagementMixin:
    """Configuration management Mixin"""
    
    def get_unified_config(self) -> UnifiedConfigManager:
        """Get unified configuration manager

        Returns:
            UnifiedConfigManager: Unified configuration manager instance
        """
        return self._unified_config

    def get_json_config(self, client_id: Optional[str] = None) -> ConfigResponse:
        """Query service configuration, equivalent to GET /register/json (optimized: use cache)"""
        if not client_id or client_id == self.client_manager.global_agent_store_id:
            # Use UnifiedConfigManager to read config (from cache, more efficient)
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
        # TODO: Whether show_mcpjson and get_json_config have some overlap
        """
        Directly read and return mcp.json file content (optimized: use cache)

        Returns:
            Dict[str, Any]: Content of mcp.json file
        """
        # Use UnifiedConfigManager to read config (from cache, more efficient)
        return self._unified_config.get_mcp_config()

    async def _sync_discovered_agents_to_files(self, agents_discovered: set):
        """
        Single data source architecture: no longer sync to sharded files

        In new architecture, Agent discovery only needs to update cache, all persistence done through mcp.json
        """
        try:
            # logger.info(f" [SYNC_AGENTS] Single data source mode: Skip sharded file sync, discovered {len(agents_discovered)} agents")
            
            # Single data source mode: No longer write to sharded files, only maintain cache and mcp.json
            # logger.info(" [SYNC_AGENTS] Single data source mode: Agent discovery completed, cache updated")
            pass
        except Exception as e:
            # logger.error(f" [SYNC_AGENTS] Agent sync failed: {e}")
            raise

    async def _switch_cache_backend(self, cache_config: Union["MemoryConfig", "RedisConfig", str, Dict[str, Any]]) -> None:
        from mcpstore.config.cache_config import MemoryConfig, RedisConfig, create_kv_store_async

        parsed_config = self._parse_cache_config(cache_config, MemoryConfig, RedisConfig)
        new_kv_store = await create_kv_store_async(parsed_config, test_connection=True)
        await self.registry.switch_backend(new_kv_store)

    def _parse_cache_config(
        self,
        cache_config: Union["MemoryConfig", "RedisConfig", str, Dict[str, Any]],
        memory_cls,
        redis_cls,
    ) -> Union["MemoryConfig", "RedisConfig"]:
        if isinstance(cache_config, (memory_cls, redis_cls)):
            return cache_config

        if isinstance(cache_config, str):
            if cache_config.lower() == "memory":
                return memory_cls()
            raise ValueError(f"Unsupported cache type string: {cache_config}")

        if isinstance(cache_config, dict):
            cache_type = str(cache_config.get("type", "")).lower()

            if cache_type == "memory":
                return memory_cls(
                    max_size=cache_config.get("max_size"),
                    cleanup_interval=cache_config.get("cleanup_interval", 300),
                )

            if cache_type == "redis":
                return redis_cls(
                    url=cache_config.get("url"),
                    host=cache_config.get("host"),
                    port=cache_config.get("port"),
                    db=cache_config.get("db"),
                    password=cache_config.get("password"),
                    namespace=cache_config.get("namespace"),
                    max_connections=cache_config.get("max_connections", 50),
                    socket_timeout=cache_config.get("socket_timeout", 5.0),
                    health_check_interval=cache_config.get("health_check_interval", 30),
                )

            raise ValueError(f"Unsupported cache type: {cache_type}")

        raise ValueError(f"Invalid cache_config type: {type(cache_config)}")
