"""
Local Service Adapter
Provides backward compatibility while transitioning from LocalServiceManager to FastMCP.
"""

import logging
from pathlib import Path
from typing import Dict, Any, Optional, Tuple

from .fastmcp_integration import FastMCPServiceManager

logger = logging.getLogger(__name__)

class LocalServiceManagerAdapter:
    """
    LocalServiceManager Adapter

    Provides the same interface as the original LocalServiceManager, but internally uses FastMCP implementation.
    This ensures backward compatibility while gradually migrating to FastMCP.
    """
    
    def __init__(self, base_work_dir: str = None):
        """
        Initialize adapter

        Args:
            base_work_dir: Base working directory
        """
        self.base_work_dir = Path(base_work_dir or Path.cwd())
        
        # Use FastMCP service manager as underlying implementation
        self.fastmcp_manager = FastMCPServiceManager(self.base_work_dir)
        
        # Health check configuration
        self.health_check_interval = 30
        self.max_restart_attempts = 3
        self.restart_delay = 5

        # Monitoring tasks
        self._health_check_task = None
        self._monitor_started = False
        
        logger.info(f"LocalServiceManagerAdapter initialized (using FastMCP backend)")
    
    async def start_local_service(self, name: str, config: Dict[str, Any]) -> Tuple[bool, str]:
        """
        Start local service (compatible with LocalServiceManager interface)
        
        Args:
            name: Service name
            config: Service configuration
            
        Returns:
            Tuple[bool, str]: (Success, message)
        """
        logger.info(f"[Adapter] Starting local service {name} via FastMCP")
        
        # Delegate to FastMCP manager
        return await self.fastmcp_manager.start_local_service(name, config)
    
    async def stop_local_service(self, name: str) -> Tuple[bool, str]:
        """
        Stop local service (compatible with LocalServiceManager interface)
        
        Args:
            name: Service name
            
        Returns:
            Tuple[bool, str]: (Success, message)
        """
        logger.info(f"[Adapter] Stopping local service {name} via FastMCP")
        
        # Delegate to FastMCP manager
        return await self.fastmcp_manager.stop_local_service(name)
    
    def get_service_status(self, name: str) -> Dict[str, Any]:
        """
        Get service status (compatible with LocalServiceManager interface)
        
        Args:
            name: Service name
            
        Returns:
            Dict[str, Any]: Service status information
        """
        # Delegate to FastMCP manager
        status = self.fastmcp_manager.get_service_status(name)
        
        # Convert to original LocalServiceManager status format
        if status.get("status") == "not_found":
            return {"status": "not_found"}
        elif status.get("status") == "error":
            return {"status": "stopped", "error": status.get("error")}
        else:
            return {
                "status": "running",
                "pid": 0,  # Process managed by FastMCP, PID not exposed
                "start_time": status.get("start_time", 0),
                "restart_count": 0,  # FastMCP handles restarts automatically
                "uptime": status.get("uptime", 0),
                "managed_by": "fastmcp"
            }
    
    def list_services(self) -> Dict[str, Dict[str, Any]]:
        """
        List all service statuses (compatible with LocalServiceManager interface)
        
        Returns:
            Dict[str, Dict[str, Any]]: Status information of all services
        """
        # Delegate to FastMCP manager and convert format
        fastmcp_services = self.fastmcp_manager.list_services()
        
        # Convert to original LocalServiceManager format
        result = {}
        for name, status in fastmcp_services.items():
            result[name] = self.get_service_status(name)
        
        return result
    
    async def cleanup(self):
        """
        Clean up all services (compatible with LocalServiceManager interface)
        """
        logger.info("[Adapter] Cleaning up services via FastMCP")
        
        # Stop health monitoring (compatibility)
        if self._health_check_task:
            self._health_check_task.cancel()
        
        # Delegate to FastMCP manager
        await self.fastmcp_manager.cleanup()
    
    # Health monitoring, process checking, service restart and other features are now fully handled by FastMCP automatically

    async def start_health_monitoring(self):
        """Start health monitoring (FastMCP handles automatically)"""
        logger.info("[Adapter] Health monitoring delegated to FastMCP")
        self._monitor_started = True
    
    # _prepare_environment and _resolve_working_dir methods have been removed
    # Environment variable and working directory handling are now fully handled by FastMCP configuration normalization

# Global instance (maintains same interface as original LocalServiceManager)
_local_service_manager_adapter: Optional[LocalServiceManagerAdapter] = None


def get_local_service_manager() -> LocalServiceManagerAdapter:
    """
    Get global local service manager instance (adapter version)

    This function replaces the original get_local_service_manager but returns an adapter instance.
    The adapter provides the same interface but uses FastMCP implementation internally.

    Returns:
        LocalServiceManagerAdapter: Global adapter instance
    """
    global _local_service_manager_adapter
    if _local_service_manager_adapter is None:
        _local_service_manager_adapter = LocalServiceManagerAdapter()
    return _local_service_manager_adapter


def set_local_service_manager_work_dir(base_work_dir: str):
    """
    Set working directory for local service manager (used for data space mode)

    Args:
        base_work_dir: Base working directory
    """
    global _local_service_manager_adapter
    _local_service_manager_adapter = LocalServiceManagerAdapter(base_work_dir)
    logger.info(f"LocalServiceManagerAdapter work directory set to: {base_work_dir}")

# Export adapter class
LocalServiceManager = LocalServiceManagerAdapter

# LocalServiceProcess class (for type compatibility)
from dataclasses import dataclass
import subprocess

@dataclass
class LocalServiceProcess:
    """Local service process information"""
    name: str
    process: Optional[subprocess.Popen] = None
    config: Dict[str, Any] = None
    start_time: float = 0
    pid: int = 0
    status: str = "running"
    restart_count: int = 0
    last_health_check: float = 0

