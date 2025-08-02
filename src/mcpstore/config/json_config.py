import json
import logging
import os
from datetime import datetime
from typing import List, Dict, Any, Optional

from pydantic import BaseModel, model_validator, ConfigDict

logger = logging.getLogger(__name__)

# Backup strategy: Keep at most 1 backup per file, using .bak suffix

class MCPServerModel(BaseModel):
    """
    Tolerant MCP service configuration model, supports all configuration formats of FastMCP Client
    Reference: https://docs.fastmcp.com/clients/transports
    """
    # Remote service configuration
    url: Optional[str] = None
    transport: Optional[str] = None  # Optional, Client will auto-infer
    headers: Optional[Dict[str, str]] = None

    # Local service configuration
    command: Optional[str] = None
    args: Optional[List[str]] = None
    env: Optional[Dict[str, str]] = None

    # General configuration
    name: Optional[str] = None
    description: Optional[str] = None
    keep_alive: Optional[bool] = None
    timeout: Optional[int] = None

    # Allow extra fields, maintain maximum compatibility
    model_config = ConfigDict(extra="allow")

    @model_validator(mode='before')
    @classmethod
    def validate_basic_config(cls, values):
        """Basic configuration validation: must have at least url or command"""
        if not (values.get("url") or values.get("command")):
            raise ValueError("MCP server must have either 'url' or 'command' field")
        return values

class MCPConfigModel(BaseModel):
    """
    Tolerant MCP configuration model, supports FastMCP's configuration format
    """
    mcpServers: Dict[str, Dict[str, Any]]  # Use Dict instead of strict MCPServerModel

    # Allow extra fields
    model_config = ConfigDict(extra="allow")

    @model_validator(mode='before')
    @classmethod
    def ensure_mcpServers(cls, values):
        if "mcpServers" not in values:
            values["mcpServers"] = {}
        return values

class ConfigError(Exception):
    """Base class for configuration errors"""
    pass

class ConfigValidationError(ConfigError):
    """Raised when configuration validation fails"""
    pass

class ConfigIOError(ConfigError):
    """Raised when configuration file operations fail"""
    pass

class MCPConfig:
    """Handle loading, parsing and saving of mcp.json file"""
    
    def __init__(self, json_path: str = None, client_id: str = "main"):
        """Initialize configuration manager
        
        Args:
            json_path: Path to the configuration file
            client_id: Client identifier for multi-client support
        """
        self.json_path = json_path or os.path.join(os.path.dirname(os.path.dirname(__file__)), "data", "mcp.json")
        self.client_id = client_id
        logger.info(f"MCP configuration initialized for client {client_id}, using file path: {self.json_path}")
    
    def _backup(self) -> None:
        """Create a backup of the current configuration file"""
        if not os.path.exists(self.json_path):
            return

        # Uniformly use .bak suffix, keep at most 1 backup per file
        backup_path = f"{self.json_path}.bak"
        try:
            with open(self.json_path, 'rb') as src, open(backup_path, 'wb') as dst:
                dst.write(src.read())
            logger.info(f"Backup created: {backup_path}")
        except Exception as e:
            logger.error(f"Backup failed: {e}")
            raise ConfigIOError(f"Failed to create backup: {e}")
    
    def load_config(self) -> Dict[str, Any]:
        """Load and validate configuration from file
        
        Returns:
            Dict containing the configuration
            
        Raises:
            ConfigIOError: If file operations fail
            ConfigValidationError: If configuration is invalid
        """
        if not os.path.exists(self.json_path):
            logger.warning(f"Configuration file does not exist: {self.json_path}, creating empty file")
            self.save_config({"mcpServers": {}})
            return {"mcpServers": {}}
            
        try:
            with open(self.json_path, 'r', encoding='utf-8') as f:
                data = json.load(f)

            # Basic format check, but no strict validation
            if not isinstance(data, dict):
                raise ConfigValidationError("Configuration must be a dictionary")

            if "mcpServers" in data and not isinstance(data["mcpServers"], dict):
                raise ConfigValidationError("mcpServers must be a dictionary")

            # No longer perform strict Pydantic validation, let FastMCP Client handle it
            return data

        except json.JSONDecodeError as e:
            raise ConfigIOError(f"Failed to parse configuration file: {e}")
        except Exception as e:
            raise ConfigIOError(f"Error reading configuration file: {e}")
    
    def save_config(self, config: Dict[str, Any]) -> bool:
        """Save configuration to file with validation
        
        Args:
            config: Configuration dictionary to save
            
        Returns:
            bool: True if save was successful
            
        Raises:
            ConfigValidationError: If configuration is invalid
            ConfigIOError: If file operations fail
        """
        # Basic format check, but no strict validation
        if not isinstance(config, dict):
            raise ConfigValidationError("Configuration must be a dictionary")

        if "mcpServers" in config and not isinstance(config["mcpServers"], dict):
            raise ConfigValidationError("mcpServers must be a dictionary")

        # No longer perform strict Pydantic validation, let FastMCP Client handle it
            
        self._backup()
        tmp_path = f"{self.json_path}.tmp"
        
        try:
            with open(tmp_path, 'w', encoding='utf-8') as f:
                json.dump(config, f, ensure_ascii=False, indent=2)
            os.replace(tmp_path, self.json_path)
            logger.info(f"Configuration saved successfully to {self.json_path}")
            return True
        except Exception as e:
            if os.path.exists(tmp_path):
                os.remove(tmp_path)
            raise ConfigIOError(f"Failed to save configuration: {e}")
    
    def get_service_config(self, name: str) -> Optional[Dict[str, Any]]:
        """Get configuration for a specific service
        
        Args:
            name: Service name
            
        Returns:
            Optional[Dict]: Service configuration if found, None otherwise
        """
        config = self.load_config()
        servers = config.get("mcpServers", {})
        if name in servers:
            result = dict(servers[name])
            return result
        return None
    
    def get_all_services(self) -> List[Dict[str, Any]]:
        """Get configuration for all services
        
        Returns:
            List[Dict]: List of service configurations
        """
        config = self.load_config()
        servers = config.get("mcpServers", {})
        return [{"name": name, **server_config} for name, server_config in servers.items()]
    
    def update_service(self, name: str, config: Dict[str, Any]) -> bool:
        """Update or add a service configuration
        
        Args:
            name: Service name
            config: Service configuration
            
        Returns:
            bool: True if update was successful
            
        Raises:
            ConfigValidationError: If service configuration is invalid
        """
        # Basic format check, but no strict validation
        if not isinstance(config, dict):
            raise ConfigValidationError("Service configuration must be a dictionary")

        # Check basic requirements: must have at least url or command
        if not (config.get("url") or config.get("command")):
            available_fields = list(config.keys())
            raise ConfigValidationError(
                f"Service must have either 'url' or 'command' field. "
                f"Current config has: {available_fields}. "
                f"Tip: For incremental updates, use patch_service() instead of update_service()."
            )

        # No longer perform strict Pydantic validation, let FastMCP Client handle it
            
        current_config = self.load_config()
        current_config["mcpServers"][name] = config
        return self.save_config(current_config)

    def update_service_config(self, name: str, config: Dict[str, Any]) -> bool:
        """Update service configuration (alias for update_service)

        Args:
            name: Service name
            config: Service configuration

        Returns:
            bool: True if update was successful
        """
        return self.update_service(name, config)

    def remove_service(self, name: str) -> bool:
        """Remove a service configuration
        
        Args:
            name: Service name
            
        Returns:
            bool: True if removal was successful
        """
        config = self.load_config()
        servers = config.get("mcpServers", {})
        if name in servers:
            del servers[name]
            config["mcpServers"] = servers
            return self.save_config(config)
        return False
    
    def compare_configs(self, new_config: Dict[str, Any]) -> Dict[str, Any]:
        """Compare new configuration with current configuration
        
        Args:
            new_config: New configuration to compare
            
        Returns:
            Dict containing added, removed, and modified services
        """
        current = self.load_config()
        current_servers = current.get("mcpServers", {})
        new_servers = new_config.get("mcpServers", {})
        
        added = set(new_servers.keys()) - set(current_servers.keys())
        removed = set(current_servers.keys()) - set(new_servers.keys())
        modified = {name for name in set(current_servers.keys()) & set(new_servers.keys())
                   if current_servers[name] != new_servers[name]}
        
        return {
            "added": list(added),
            "removed": list(removed),
            "modified": list(modified)
        }

    def reset_mcp_json_file(self) -> bool:
        """
        Directly reset MCP JSON configuration file
        1. Backup current configuration file
        2. Reset configuration to empty dictionary {"mcpServers": {}}

        Returns:
            Whether reset was successful
        """
        try:
            import shutil
            from datetime import datetime

            # Create backup
            backup_path = f"{self.json_path}.bak"
            shutil.copy2(self.json_path, backup_path)
            logger.info(f"Created backup at {backup_path}")

            # Reset to empty configuration
            empty_config = {"mcpServers": {}}
            self.save_config(empty_config)

            logger.info(f"Successfully reset MCP JSON configuration file: {self.json_path}")
            return True

        except Exception as e:
            logger.error(f"Failed to reset MCP JSON configuration file: {e}")
            return False


