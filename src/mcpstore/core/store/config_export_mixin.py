"""
配置导出 Mixin 模块
负责处理 MCPStore 的配置导出功能
"""

import json
import logging
from typing import Optional, Dict, Any

logger = logging.getLogger(__name__)


class ConfigExportMixin:
    """配置导出 Mixin - 包含配置导出方法"""
    
    async def exportjson(self, filepath: Optional[str] = None) -> Dict[str, Any]:
        """
        将缓存数据导出为标准 MCP JSON 格式
        
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
            agent_ids = self.registry.get_all_agent_ids()
            logger.debug(f"Found {len(agent_ids)} agents in cache")
            
            for agent_id in agent_ids:
                # Get all client IDs for this agent
                client_ids = self.registry.get_agent_clients_from_cache(agent_id)
                logger.debug(f"Agent {agent_id} has {len(client_ids)} clients")
                
                for client_id in client_ids:
                    # Get client configuration
                    client_config = self.registry.get_client_config_from_cache(client_id)
                    
                    if client_config and "mcpServers" in client_config:
                        # Extract mcpServers from this client
                        for service_name, service_config in client_config["mcpServers"].items():
                            # For agent services, use the global name (with suffix)
                            if agent_id != self.client_manager.global_agent_store_id:
                                # Check if this is an agent service - get global name
                                global_name = self.registry.get_global_name_from_agent_service(
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
        从缓存导出配置到 JSON 文件
        
        Args:
            output_path: 输出 JSON 文件路径
            include_sessions: 是否包含 Session 数据（默认 False，因为 Session 不可序列化）
            
        Raises:
            ValueError: 如果 include_sessions=True（Session 数据不可序列化）
            RuntimeError: 如果导出过程失败
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
        从缓存导出配置数据（不包括 Session）
        
        Returns:
            配置字典，格式与 mcp.json 兼容
        """
        try:
            # Get all agent IDs from registry
            agent_ids = await self._get_all_agent_ids_from_cache()
            
            # Build mcpServers configuration
            mcp_servers = {}
            
            for agent_id in agent_ids:
                # Get client IDs for this agent
                client_ids = self.registry.get_agent_clients_from_cache(agent_id)
                
                for client_id in client_ids:
                    # Get client configuration
                    client_config = self.registry.get_client_config_from_cache(client_id)
                    
                    if client_config and "mcpServers" in client_config:
                        # Merge mcpServers from this client
                        for service_name, service_config in client_config["mcpServers"].items():
                            # For agent services, use the global name (with suffix)
                            if agent_id != self.client_manager.global_agent_store_id:
                                # Check if this is an agent service mapping
                                from mcpstore.core.context.agent_service_mapper import AgentServiceMapper
                                global_name = AgentServiceMapper.get_global_service_name(agent_id, service_name)
                                mcp_servers[global_name] = service_config
                            else:
                                # Store service, use service name directly
                                mcp_servers[service_name] = service_config
            
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
        从缓存获取所有 Agent ID
        
        Returns:
            Agent ID 列表
        """
        try:
            # Get all agent IDs from agent_clients mapping
            agent_ids = set()
            
            # Access agent_clients from registry
            if hasattr(self.registry, 'agent_clients'):
                agent_ids.update(self.registry.agent_clients.keys())
            
            # Also check service_to_client mapping
            if hasattr(self.registry, 'service_to_client'):
                agent_ids.update(self.registry.service_to_client.keys())
            
            return list(agent_ids)
            
        except Exception as e:
            logger.error(f"Failed to get agent IDs from cache: {e}")
            return []
