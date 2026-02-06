"""
MCP Integration Layer
Provides a clean interface between project code and the MCP client implementation (currently MCPStore), handling configuration normalization.
"""

import logging
import time
from pathlib import Path
from typing import Dict, Any, Optional, Tuple

from mcpstore.mcp import Client

logger = logging.getLogger(__name__)

class MCPStoreServiceManager:
    """
    MCP client Service Manager（当前使用 MCPStore 实现）

    Responsible for converting user-relaxed configuration to MCP canonical configuration, and managing MCP clients.
    This is the bridge between project code 和底层 MCP 客户端实现。
    """
    
    def __init__(self, base_work_dir: Optional[Path] = None):
        """
        Initialize MCP client service manager

        Args:
            base_work_dir: Base working directory for local services
        """
        self.base_work_dir = base_work_dir or Path.cwd()
        self.clients: Dict[str, Client] = {}
        self.service_configs: Dict[str, Dict[str, Any]] = {}
        self.service_start_times: Dict[str, float] = {}
        
        logger.info(f"MCPStoreServiceManager initialized with work_dir: {self.base_work_dir}")
    
    async def start_local_service(self, name: str, config: Dict[str, Any]) -> Tuple[bool, str]:
        """
        Start local service (replaces LocalServiceManager.start_local_service)

        Args:
            name: Service name
            config: User configuration (relaxed format)

        Returns:
            Tuple[bool, str]: (Success, message)
        """
        try:
            logger.info(f"Starting local service {name} with MCPStore")
            
            # 1. Configuration normalization: Convert user configuration to MCP canonical format
            mcp_config = self._normalize_local_service_config(name, config)
            
            # 2. Create MCPStore client
            client = Client(mcp_config)
            
            # 3. Test connection (MCPStore will automatically start process)
            try:
                async with client:
                    # MCPStore automatically handles:
                    # - Process startup (subprocess.Popen)
                    # - Environment variable setup
                    # - Working directory setup
                    # - stdin/stdout management
                    await client.ping()  # Standard MCP ping
                    
                    # Store client and configuration
                    self.clients[name] = client
                    self.service_configs[name] = config
                    self.service_start_times[name] = time.time()
                    
                    logger.info(f"Local service {name} started successfully via MCP client")
                    return True, f"Service started successfully via MCP client"
                    
            except Exception as e:
                logger.error(f"MCP client failed to start service {name}: {e}")
                return False, f"MCP client connection failed: {str(e)}"
                
        except Exception as e:
            logger.error(f"Failed to start local service {name}: {e}")
            return False, str(e)
    
    async def stop_local_service(self, name: str) -> Tuple[bool, str]:
        """
        Stop local service (replaces LocalServiceManager.stop_local_service)
        
        Args:
            name: Service name
            
        Returns:
            Tuple[bool, str]: (Success, message)
        """
        try:
            if name not in self.clients:
                return False, f"Service {name} not found"
            
            # MCP client will automatically handle process cleanup
            client = self.clients[name]
            
            # Clean up records
            del self.clients[name]
            if name in self.service_configs:
                del self.service_configs[name]
            if name in self.service_start_times:
                del self.service_start_times[name]
            
            logger.info(f"Local service {name} stopped successfully")
            return True, "Service stopped successfully"
            
        except Exception as e:
            logger.error(f"Failed to stop local service {name}: {e}")
            return False, str(e)
    
    def get_service_status(self, name: str) -> Dict[str, Any]:
        """
        Get service status (replaces LocalServiceManager.get_service_status)
        
        Args:
            name: Service name
            
        Returns:
            Dict[str, Any]: Service status information
        """
        if name not in self.clients:
            return {"status": "not_found"}
        
        try:
            # Use MCP client to check connection status
            client = self.clients[name]
            
            # Simple status check
            start_time = self.service_start_times.get(name, 0)
            uptime = time.time() - start_time if start_time > 0 else 0
            
            return {
                "status": "running",  # MCP client managed services assumed to be in running state
                "uptime": uptime,
                "start_time": start_time,
                "managed_by": "mcp"
            }
            
        except Exception as e:
            logger.error(f"Failed to get service status for {name}: {e}")
            return {"status": "error", "error": str(e)}
    
    def list_services(self) -> Dict[str, Dict[str, Any]]:
        """
        List all service statuses (replaces LocalServiceManager.list_services)
        
        Returns:
            Dict[str, Dict[str, Any]]: Status information of all services
        """
        return {name: self.get_service_status(name) for name in self.clients}
    
    async def cleanup(self):
        """
        Clean up all services (replaces LocalServiceManager.cleanup)
        """
        logger.info("Cleaning up MCP client services...")
        
        # Stop all services
        for name in list(self.clients.keys()):
            await self.stop_local_service(name)
        
        logger.info("MCP client service cleanup completed")
    
    def _normalize_local_service_config(self, name: str, config: Dict[str, Any]) -> Dict[str, Any]:
        """
        Configuration normalization: Convert user-relaxed configuration to MCP canonical configuration
        
        This is the core value of MCPStore: Allow users to input relaxed format and convert to standard format
        
        Args:
            name: Service name
            config: User configuration (relaxed format)
            
        Returns:
            Dict[str, Any]: MCP canonical configuration
        """
        # MCP canonical configuration format
        mcp_config = {
            "mcpServers": {
                name: {}
            }
        }
        
        service_config = mcp_config["mcpServers"][name]
        
        # 1. Handle required fields
        if "command" not in config:
            raise ValueError(f"Local service {name} missing required 'command' field")
        
        service_config["command"] = config["command"]
        
        # 2. Handle optional fields
        if "args" in config:
            service_config["args"] = config["args"]
        
        # 3. Environment variable handling (simplified version)
        env = {}
        if "env" in config:
            env.update(config["env"])
        
        # Ensure PYTHONPATH includes working directory
        if "PYTHONPATH" not in env:
            env["PYTHONPATH"] = str(self.base_work_dir)
        else:
            env["PYTHONPATH"] = f"{self.base_work_dir}{Path.pathsep}{env['PYTHONPATH']}"
        
        service_config["env"] = env
        
        # 4. Working directory handling
        working_dir = config.get("working_dir")
        if working_dir:
            # If relative path, relative to base_work_dir
            work_path = Path(working_dir)
            if not work_path.is_absolute():
                work_path = self.base_work_dir / work_path
            service_config["cwd"] = str(work_path.resolve())
        else:
            service_config["cwd"] = str(self.base_work_dir)
        
        logger.debug(f"Normalized config for {name}: {mcp_config}")
        return mcp_config

# Global instance (maintain same interface as LocalServiceManager)
_mcpstore_service_manager: Optional[MCPStoreServiceManager] = None

def get_mcpstore_service_manager(base_work_dir: Optional[Path] = None) -> MCPStoreServiceManager:
    """
    Get global MCPStore service manager instance (replaces get_local_service_manager)
    
    Args:
        base_work_dir: Base working directory
        
    Returns:
        MCPStoreServiceManager: Global instance
    """
    global _mcpstore_service_manager
    if _mcpstore_service_manager is None:
        _mcpstore_service_manager = MCPStoreServiceManager(base_work_dir)
    elif base_work_dir and _mcpstore_service_manager.base_work_dir != base_work_dir:
        # If working directory is different, create new instance
        _mcpstore_service_manager = MCPStoreServiceManager(base_work_dir)
    return _mcpstore_service_manager
