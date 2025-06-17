"""
MCPStore 统一配置管理器

整合所有配置功能，提供统一的配置管理接口。
"""

import os
import logging
from typing import Dict, Any, Optional, List
from dataclasses import dataclass
from enum import Enum

# 导入现有的配置组件
from mcpstore.config.config import load_app_config
from mcpstore.plugins.json_mcp import MCPConfig, ConfigError, ConfigValidationError, ConfigIOError
from mcpstore.core.client_manager import ClientManager

logger = logging.getLogger(__name__)

class ConfigType(Enum):
    """配置类型枚举"""
    ENVIRONMENT = "environment"  # 环境变量配置
    MCP_SERVICES = "mcp_services"  # MCP服务配置
    CLIENT_SERVICES = "client_services"  # 客户端服务配置
    AGENT_CLIENTS = "agent_clients"  # Agent-Client映射配置

@dataclass
class ConfigInfo:
    """配置信息"""
    config_type: ConfigType
    source: str  # 配置来源（文件路径或环境变量）
    last_modified: Optional[str] = None
    is_valid: bool = True
    error_message: Optional[str] = None

class UnifiedConfigManager:
    """统一配置管理器
    
    整合环境变量配置、MCP服务配置、客户端配置等所有配置功能。
    提供统一的配置访问、更新、验证接口。
    """
    
    def __init__(self, 
                 mcp_config_path: Optional[str] = None,
                 client_services_path: Optional[str] = None):
        """初始化统一配置管理器
        
        Args:
            mcp_config_path: MCP配置文件路径
            client_services_path: 客户端服务配置文件路径
        """
        self.logger = logger
        
        # 初始化各个配置组件
        self.env_config = None
        self.mcp_config = MCPConfig(json_path=mcp_config_path)
        self.client_manager = ClientManager(services_path=client_services_path)
        
        # 配置缓存
        self._config_cache: Dict[ConfigType, Dict[str, Any]] = {}
        self._cache_valid: Dict[ConfigType, bool] = {}
        
        # 初始化配置
        self._initialize_configs()
        
        logger.info("UnifiedConfigManager initialized successfully")
    
    def _initialize_configs(self):
        """初始化所有配置"""
        try:
            # 加载环境变量配置
            self.env_config = load_app_config()
            self._config_cache[ConfigType.ENVIRONMENT] = self.env_config
            self._cache_valid[ConfigType.ENVIRONMENT] = True
            
            # 预加载其他配置到缓存
            self._refresh_cache(ConfigType.MCP_SERVICES)
            self._refresh_cache(ConfigType.CLIENT_SERVICES)
            self._refresh_cache(ConfigType.AGENT_CLIENTS)
            
        except Exception as e:
            logger.error(f"Failed to initialize configs: {e}")
            raise ConfigError(f"Configuration initialization failed: {e}")
    
    def _refresh_cache(self, config_type: ConfigType):
        """刷新指定类型的配置缓存"""
        try:
            if config_type == ConfigType.MCP_SERVICES:
                self._config_cache[config_type] = self.mcp_config.load_config()
            elif config_type == ConfigType.CLIENT_SERVICES:
                self._config_cache[config_type] = self.client_manager.load_all_clients()
            elif config_type == ConfigType.AGENT_CLIENTS:
                self._config_cache[config_type] = self.client_manager.load_all_agent_clients()
            
            self._cache_valid[config_type] = True
            
        except Exception as e:
            logger.error(f"Failed to refresh cache for {config_type}: {e}")
            self._cache_valid[config_type] = False
            raise
    
    def get_config(self, config_type: ConfigType, force_reload: bool = False) -> Dict[str, Any]:
        """获取指定类型的配置
        
        Args:
            config_type: 配置类型
            force_reload: 是否强制重新加载
            
        Returns:
            配置字典
        """
        if force_reload or not self._cache_valid.get(config_type, False):
            if config_type == ConfigType.ENVIRONMENT:
                self.env_config = load_app_config()
                self._config_cache[config_type] = self.env_config
            else:
                self._refresh_cache(config_type)
        
        return self._config_cache.get(config_type, {})
    
    def get_env_config(self) -> Dict[str, Any]:
        """获取环境变量配置"""
        return self.get_config(ConfigType.ENVIRONMENT)
    
    def get_mcp_config(self) -> Dict[str, Any]:
        """获取MCP服务配置"""
        return self.get_config(ConfigType.MCP_SERVICES)
    
    def get_client_config(self, client_id: str) -> Optional[Dict[str, Any]]:
        """获取指定客户端的配置
        
        Args:
            client_id: 客户端ID
            
        Returns:
            客户端配置或None
        """
        client_configs = self.get_config(ConfigType.CLIENT_SERVICES)
        return client_configs.get(client_id)
    
    def get_agent_clients(self, agent_id: str) -> List[str]:
        """获取指定Agent的客户端列表
        
        Args:
            agent_id: Agent ID
            
        Returns:
            客户端ID列表
        """
        agent_configs = self.get_config(ConfigType.AGENT_CLIENTS)
        return agent_configs.get(agent_id, [])
    
    def get_service_config(self, service_name: str) -> Optional[Dict[str, Any]]:
        """获取指定服务的配置
        
        Args:
            service_name: 服务名称
            
        Returns:
            服务配置或None
        """
        return self.mcp_config.get_service_config(service_name)
    
    def update_mcp_config(self, config: Dict[str, Any]) -> bool:
        """更新MCP配置
        
        Args:
            config: 新的MCP配置
            
        Returns:
            更新是否成功
        """
        try:
            result = self.mcp_config.save_config(config)
            if result:
                self._refresh_cache(ConfigType.MCP_SERVICES)
            return result
        except Exception as e:
            logger.error(f"Failed to update MCP config: {e}")
            return False
    
    def update_service_config(self, service_name: str, config: Dict[str, Any]) -> bool:
        """更新服务配置
        
        Args:
            service_name: 服务名称
            config: 服务配置
            
        Returns:
            更新是否成功
        """
        try:
            result = self.mcp_config.update_service(service_name, config)
            if result:
                self._refresh_cache(ConfigType.MCP_SERVICES)
            return result
        except Exception as e:
            logger.error(f"Failed to update service config for {service_name}: {e}")
            return False
    
    def add_client(self, config: Dict[str, Any], client_id: Optional[str] = None) -> str:
        """添加新的客户端配置
        
        Args:
            config: 客户端配置
            client_id: 可选的客户端ID
            
        Returns:
            使用的客户端ID
        """
        try:
            client_id = self.client_manager.add_client(config, client_id)
            self._refresh_cache(ConfigType.CLIENT_SERVICES)
            return client_id
        except Exception as e:
            logger.error(f"Failed to add client: {e}")
            raise
    
    def get_all_configs(self) -> Dict[str, Dict[str, Any]]:
        """获取所有配置
        
        Returns:
            包含所有配置类型的字典
        """
        return {
            "environment": self.get_env_config(),
            "mcp_services": self.get_mcp_config(),
            "client_services": self.get_config(ConfigType.CLIENT_SERVICES),
            "agent_clients": self.get_config(ConfigType.AGENT_CLIENTS)
        }
    
    def get_config_info(self) -> List[ConfigInfo]:
        """获取所有配置的信息
        
        Returns:
            配置信息列表
        """
        configs = []
        
        # 环境变量配置信息
        configs.append(ConfigInfo(
            config_type=ConfigType.ENVIRONMENT,
            source="Environment Variables",
            is_valid=self._cache_valid.get(ConfigType.ENVIRONMENT, False)
        ))
        
        # MCP服务配置信息
        configs.append(ConfigInfo(
            config_type=ConfigType.MCP_SERVICES,
            source=self.mcp_config.json_path,
            is_valid=self._cache_valid.get(ConfigType.MCP_SERVICES, False)
        ))
        
        # 客户端服务配置信息
        configs.append(ConfigInfo(
            config_type=ConfigType.CLIENT_SERVICES,
            source=self.client_manager.services_path,
            is_valid=self._cache_valid.get(ConfigType.CLIENT_SERVICES, False)
        ))
        
        # Agent-Client映射配置信息
        agent_clients_path = getattr(self.client_manager, 'agent_clients_path', 'Unknown')
        configs.append(ConfigInfo(
            config_type=ConfigType.AGENT_CLIENTS,
            source=agent_clients_path,
            is_valid=self._cache_valid.get(ConfigType.AGENT_CLIENTS, False)
        ))
        
        return configs
    
    def validate_all_configs(self) -> Dict[str, bool]:
        """验证所有配置
        
        Returns:
            各配置类型的验证结果
        """
        results = {}
        
        try:
            # 验证环境变量配置
            env_config = self.get_env_config()
            results["environment"] = isinstance(env_config, dict) and len(env_config) > 0
        except Exception:
            results["environment"] = False
        
        try:
            # 验证MCP配置
            mcp_config = self.get_mcp_config()
            results["mcp_services"] = "mcpServers" in mcp_config
        except Exception:
            results["mcp_services"] = False
        
        try:
            # 验证客户端配置
            client_config = self.get_config(ConfigType.CLIENT_SERVICES)
            results["client_services"] = isinstance(client_config, dict)
        except Exception:
            results["client_services"] = False
        
        try:
            # 验证Agent-Client映射
            agent_config = self.get_config(ConfigType.AGENT_CLIENTS)
            results["agent_clients"] = isinstance(agent_config, dict)
        except Exception:
            results["agent_clients"] = False
        
        return results
    
    def reload_all_configs(self):
        """重新加载所有配置"""
        logger.info("Reloading all configurations...")
        
        for config_type in ConfigType:
            try:
                self.get_config(config_type, force_reload=True)
                logger.info(f"Successfully reloaded {config_type.value} config")
            except Exception as e:
                logger.error(f"Failed to reload {config_type.value} config: {e}")
        
        logger.info("Configuration reload completed")


# 全局统一配置管理器实例
_global_config_manager: Optional[UnifiedConfigManager] = None

def get_global_config_manager() -> UnifiedConfigManager:
    """获取全局统一配置管理器实例"""
    global _global_config_manager
    if _global_config_manager is None:
        _global_config_manager = UnifiedConfigManager()
    return _global_config_manager

def set_global_config_manager(manager: UnifiedConfigManager):
    """设置全局统一配置管理器实例"""
    global _global_config_manager
    _global_config_manager = manager
