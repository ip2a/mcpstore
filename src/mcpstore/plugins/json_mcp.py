import os
import json
import logging
from typing import List, Dict, Any, Optional
from datetime import datetime
from pydantic import BaseModel, ValidationError, root_validator

logger = logging.getLogger(__name__)

BACKUP_COUNT = 3

class MCPServerModel(BaseModel):
    """
    宽容的MCP服务配置模型，支持FastMCP Client的所有配置格式
    参考: https://docs.fastmcp.com/clients/transports
    """
    # 远程服务配置
    url: Optional[str] = None
    transport: Optional[str] = None  # 可选，Client会自动推断
    headers: Optional[Dict[str, str]] = None

    # 本地服务配置
    command: Optional[str] = None
    args: Optional[List[str]] = None
    env: Optional[Dict[str, str]] = None

    # 通用配置
    name: Optional[str] = None
    description: Optional[str] = None
    keep_alive: Optional[bool] = None
    timeout: Optional[int] = None

    # 允许额外字段，保持最大兼容性
    class Config:
        extra = "allow"  # 允许额外字段

    @root_validator(pre=True)
    def validate_basic_config(cls, values):
        """基本配置验证：至少要有url或command之一"""
        if not (values.get("url") or values.get("command")):
            raise ValueError("MCP server must have either 'url' or 'command' field")
        return values

class MCPConfigModel(BaseModel):
    """
    宽容的MCP配置模型，支持FastMCP的配置格式
    """
    mcpServers: Dict[str, Dict[str, Any]]  # 使用Dict而不是严格的MCPServerModel

    # 允许额外字段
    class Config:
        extra = "allow"

    @root_validator(pre=True)
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
        ts = datetime.now().strftime("%Y%m%d_%H%M%S")
        backup_path = f"{self.json_path}.{ts}.bak"
        try:
            with open(self.json_path, 'rb') as src, open(backup_path, 'wb') as dst:
                dst.write(src.read())
            logger.info(f"Backup created: {backup_path}")
            
            # Maintain backup rotation
            backups = sorted([f for f in os.listdir(os.path.dirname(self.json_path)) 
                            if f.startswith(os.path.basename(self.json_path)) and f.endswith('.bak')])
            if len(backups) > BACKUP_COUNT:
                for old in backups[:-BACKUP_COUNT]:
                    try:
                        os.remove(os.path.join(os.path.dirname(self.json_path), old))
                    except Exception as e:
                        logger.warning(f"Failed to remove old backup: {old}, {e}")
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

            # 基本格式检查，但不进行严格验证
            if not isinstance(data, dict):
                raise ConfigValidationError("Configuration must be a dictionary")

            if "mcpServers" in data and not isinstance(data["mcpServers"], dict):
                raise ConfigValidationError("mcpServers must be a dictionary")

            # 不再进行严格的Pydantic验证，让FastMCP Client自己处理
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
        # 基本格式检查，但不进行严格验证
        if not isinstance(config, dict):
            raise ConfigValidationError("Configuration must be a dictionary")

        if "mcpServers" in config and not isinstance(config["mcpServers"], dict):
            raise ConfigValidationError("mcpServers must be a dictionary")

        # 不再进行严格的Pydantic验证，让FastMCP Client自己处理
            
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
        # 基本格式检查，但不进行严格验证
        if not isinstance(config, dict):
            raise ConfigValidationError("Service configuration must be a dictionary")

        # 检查基本要求：至少要有url或command
        if not (config.get("url") or config.get("command")):
            available_fields = list(config.keys())
            raise ConfigValidationError(
                f"Service must have either 'url' or 'command' field. "
                f"Current config has: {available_fields}. "
                f"Tip: For incremental updates, use patch_service() instead of update_service()."
            )

        # 不再进行严格的Pydantic验证，让FastMCP Client自己处理
            
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

    def reset_json_config(self) -> bool:
        """
        重置JSON配置文件
        1. 备份当前配置文件
        2. 将配置重置为空字典

        Returns:
            是否成功重置
        """
        try:
            import shutil
            from datetime import datetime

            # 创建备份
            backup_path = f"{self.json_path}.backup.{datetime.now().strftime('%Y%m%d_%H%M%S')}"
            shutil.copy2(self.json_path, backup_path)
            logger.info(f"Created backup at {backup_path}")

            # 重置为空配置
            empty_config = {"mcpServers": {}}
            self.save_config(empty_config)

            logger.info("Successfully reset JSON configuration to empty")
            return True

        except Exception as e:
            logger.error(f"Failed to reset JSON configuration: {e}")
            return False

    def restore_default_config(self) -> bool:
        """
        恢复默认配置（高德和天气服务）

        Returns:
            是否成功恢复
        """
        try:
            default_config = {
                "mcpServers": {
                    "高德": {
                        "url": "https://mcp.amap.com/sse?key=da2c9c39f9edad643b9c53f506fb381c",
                        "transport": "sse"
                    },
                    "天气服务": {
                        "url": "http://127.0.0.1:8000/mcp"
                    }
                }
            }

            self.save_config(default_config)
            logger.info("Successfully restored default configuration")
            return True

        except Exception as e:
            logger.error(f"Failed to restore default configuration: {e}")
            return False
