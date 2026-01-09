"""
Configuration export Mixin module
Responsible for handling MCPStore configuration export functionality
"""

import json
import logging
from typing import Optional, Dict, Any

logger = logging.getLogger(__name__)


class ConfigExportMixin:
    """Configuration export Mixin - Contains configuration export methods"""
    
    async def exportjson(self, filepath: Optional[str] = None) -> Dict[str, Any]:
        """
        Export cache data to standard MCP JSON format
        
        This method reads all services from the cache and converts them to the
        standard MCP JSON format with mcpServers structure. It can optionally
        save the data to a file.
        
        Args:
            filepath: Optional file path to save the exported data.
                     If None, only returns the data without saving.
        
        Returns:
            Dictionary containing the exported data in MCP JSON format:
            {
                "mcpServers": {
                    "service_name": {
                        "command": "...",
                        "args": [...],
                        ...
                    },
                    ...
                }
            }
        
        Example:
            # Export to file
            data = await store.exportjson("backup.json")
            
            # Get data without saving
            data = await store.exportjson()
        
        Note:
            Method name follows the "no underscores" naming convention for
            a cleaner API surface.
        
        Validates: Requirements 6.1, 6.2, 6.3, 6.4, 6.5, 6.6, 6.7, 6.8, 15.1, 15.2, 15.3
        """
        try:
            logger.info("Starting cache data export to MCP JSON format")
            
            # 1. Read all services from cache using registry
            mcp_servers = {}
            
            # Get all agent IDs
            agent_ids = await self.registry.get_all_agent_ids_async()
            logger.debug(f"Found {len(agent_ids)} agents in cache")
            
            for agent_id in agent_ids:
                # Get all client IDs for this agent - 从 pykv 获取
                client_ids = await self.registry.get_agent_clients_async(agent_id)
                logger.debug(f"Agent {agent_id} has {len(client_ids)} clients")
                
                for client_id in client_ids:
                    # Get client entity (新架构：使用 services 列表)
                    client_entity = self.registry.get_client_config_from_cache(client_id)
                    
                    if client_entity and isinstance(client_entity, dict):
                        services = client_entity.get("services", [])
                        # 从服务实体获取每个服务的配置
                        for service_name in services:
                            try:
                                service_info = await self.registry.get_complete_service_info_async(agent_id, service_name)
                                if not service_info or not service_info.get("config"):
                                    continue
                                service_config = service_info["config"]
                                
                                # For agent services, use the global name (with suffix)
                                if agent_id != self.client_manager.global_agent_store_id:
                                    # Check if this is an agent service - get global name（使用异步版本，避免 AOB 事件循环冲突）
                                    global_name = await self.registry.get_global_name_from_agent_service_async(
                                        agent_id, service_name
                                    )
                                    if global_name:
                                        mcp_servers[global_name] = service_config
                                    else:
                                        # Fallback: use service name as-is
                                        mcp_servers[service_name] = service_config
                                else:
                                    # Store service, use service name directly
                                    mcp_servers[service_name] = service_config
                            except Exception as e:
                                logger.warning(f"Failed to get service config for {service_name}: {e}")
                                continue
            
            # 2. Convert to standard MCP JSON format
            export_data = {
                "mcpServers": mcp_servers
            }
            
            logger.info(f"Exported {len(mcp_servers)} services from cache")
            
            # 3. Save to file if filepath provided
            if filepath:
                with open(filepath, 'w', encoding='utf-8') as f:
                    json.dump(export_data, f, indent=2, ensure_ascii=False)
                logger.info(f"Exported data saved to {filepath}")
            
            # 4. Return exported data dictionary
            return export_data
            
        except Exception as e:
            logger.error(f"Failed to export cache data: {e}", exc_info=True)
            raise RuntimeError(f"Cache data export failed: {e}")
    
    async def export_to_json(self, output_path: str, include_sessions: bool = False) -> None:
        """
        Export configuration from cache to JSON file

        Args:
            output_path: Output JSON file path
            include_sessions: Whether to include Session data (default False, as Session is not serializable)

        Raises:
            ValueError: If include_sessions=True (Session data is not serializable)
            RuntimeError: If export process fails
        """
        if include_sessions:
            raise ValueError(
                "Session data cannot be exported because Session objects are not serializable. "
                "Set include_sessions=False to export configuration without sessions."
            )
        
        try:
            logger.info(f"Starting configuration export to {output_path}")
            
            # Export configuration from cache
            config_data = await self._export_config_from_cache()
            
            # Write to JSON file
            with open(output_path, 'w', encoding='utf-8') as f:
                json.dump(config_data, f, indent=2, ensure_ascii=False)
            
            logger.info(f"Configuration exported successfully to {output_path}")
            
        except Exception as e:
            logger.error(f"Failed to export configuration: {e}")
            raise RuntimeError(f"Configuration export failed: {e}")
    
    async def _export_config_from_cache(self) -> dict:
        """
        Export configuration data from cache (excluding Sessions)

        Returns:
            Configuration dictionary compatible with mcp.json format
        """
        try:
            # Get all agent IDs from registry
            agent_ids = await self._get_all_agent_ids_from_cache()
            
            # Build mcpServers configuration
            mcp_servers = {}
            
            for agent_id in agent_ids:
                # Get client IDs for this agent - 从 pykv 获取
                client_ids = await self.registry.get_agent_clients_async(agent_id)
                
                for client_id in client_ids:
                    # Get client entity (新架构：使用 services 列表)
                    client_entity = self.registry.get_client_config_from_cache(client_id)
                    
                    if client_entity and isinstance(client_entity, dict):
                        services = client_entity.get("services", [])
                        # 从服务实体获取每个服务的配置
                        for service_name in services:
                            try:
                                service_info = await self.registry.get_complete_service_info_async(agent_id, service_name)
                                if not service_info or not service_info.get("config"):
                                    continue
                                service_config = service_info["config"]
                                
                                # For agent services, use the global name (with suffix)
                                if agent_id != self.client_manager.global_agent_store_id:
                                    # Check if this is an agent service mapping
                                    from mcpstore.core.context.agent_service_mapper import AgentServiceMapper
                                    global_name = AgentServiceMapper.get_global_service_name(agent_id, service_name)
                                    mcp_servers[global_name] = service_config
                                else:
                                    # Store service, use service name directly
                                    mcp_servers[service_name] = service_config
                            except Exception as e:
                                logger.warning(f"Failed to get service config for {service_name}: {e}")
                                continue
            
            # Build the complete configuration
            config = {
                "mcpServers": mcp_servers
            }
            
            logger.debug(f"Exported {len(mcp_servers)} services from cache")
            
            return config
            
        except Exception as e:
            logger.error(f"Failed to export configuration from cache: {e}")
            raise
    
    async def _get_all_agent_ids_from_cache(self) -> list:
        """
        Get all Agent IDs from cache

        Returns:
            List of Agent IDs
        """
        try:
            # Use Registry API to get all Agent IDs from cache
            agent_ids = await self.registry.get_all_agent_ids_async()
            return list(agent_ids)
            
        except Exception as e:
            logger.error(f"Failed to get agent IDs from cache: {e}")
            return []
