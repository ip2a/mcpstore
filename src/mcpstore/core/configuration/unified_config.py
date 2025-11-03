"""
MCPStore Unified Configuration Manager

Integrates all configuration functions, providing a unified configuration management interface.
"""

import asyncio
import logging
from dataclasses import dataclass
from enum import Enum
from typing import Dict, Any, Optional, List

# Import existing configuration components
from mcpstore.config.config import load_app_config
from mcpstore.config.json_config import MCPConfig, ConfigError
from mcpstore.core.store.client_manager import ClientManager

logger = logging.getLogger(__name__)

class ConfigType(Enum):
    """Configuration type enumeration"""
    ENVIRONMENT = "environment"  # 环境变量配置
    MCP_SERVICES = "mcp_services"  # MCP服务配置
    CLIENT_SERVICES = "client_services"  # 客户端服务配置
    AGENT_CLIENTS = "agent_clients"  # Agent-Client映射配置

@dataclass
class ConfigInfo:
    """Configuration information"""
    config_type: ConfigType
    source: str  # Configuration source (file path or environment variable)
    last_modified: Optional[str] = None
    is_valid: bool = True
    error_message: Optional[str] = None

class UnifiedConfigManager:
    """Unified configuration manager

    Integrates all configuration functions including environment variables, MCP service configuration, client configuration, etc.
    Provides unified configuration access, update, and validation interfaces.
    """

    def __init__(self,
                 mcp_config: Optional[MCPConfig] = None):
        """Initialize unified configuration manager

        Args:
            mcp_config: MCPConfig instance (if None, creates default instance)
        """
        self.logger = logger

        # 初始化各个配置组件
        self.env_config = None
        self.mcp_config = mcp_config if mcp_config is not None else MCPConfig()
        self.client_manager = ClientManager()

        # 配置缓存
        self._config_cache: Dict[ConfigType, Dict[str, Any]] = {}
        self._cache_valid: Dict[ConfigType, bool] = {}
        
        # 并发保护锁（用于异步操作）
        self._config_lock = asyncio.Lock()

        # 初始化配置
        self._initialize_configs()

        logger.debug("UnifiedConfigManager initialized successfully")
    
    def _initialize_configs(self):
        """初始化所有配置"""
        try:
            # 加载环境变量配置
            self.env_config = load_app_config()
            self._config_cache[ConfigType.ENVIRONMENT] = self.env_config
            self._cache_valid[ConfigType.ENVIRONMENT] = True
            
            # 预加载配置到缓存（单一数据源：仅加载 MCP_SERVICES；其余返回空映射）
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
                self._cache_valid[config_type] = True
            elif config_type in (ConfigType.CLIENT_SERVICES, ConfigType.AGENT_CLIENTS):
                # 单一数据源架构：分片文件已废弃，统一返回空映射并标记为有效，避免异常
                self._config_cache[config_type] = {}
                self._cache_valid[config_type] = True
            else:
                self._cache_valid[config_type] = False
            
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
        """
         单一数据源架构：废弃方法，现已不支持
        
        新架构下，客户端配置通过mcp.json和缓存管理，不再单独管理
        """
        raise NotImplementedError(
            "add_client已废弃。单一数据源架构下，请使用MCPStore.add_service()方法添加服务，"
            "客户端配置将自动通过mcp.json和缓存管理。"
        )
    
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
        
        #  单一数据源架构：分片文件配置已废弃
        configs.append(ConfigInfo(
            config_type=ConfigType.CLIENT_SERVICES,
            source="[已废弃] 单一数据源架构下不再使用分片文件",
            is_valid=False,
            error_message="单一数据源架构：client_services.json已废弃"
        ))
        
        configs.append(ConfigInfo(
            config_type=ConfigType.AGENT_CLIENTS,
            source="[已废弃] 单一数据源架构下不再使用分片文件",
            is_valid=False,
            error_message="单一数据源架构：agent_clients.json已废弃"
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
        logger.debug("Reloading all configurations...")
        
        for config_type in ConfigType:
            try:
                self.get_config(config_type, force_reload=True)
                logger.debug(f"Successfully reloaded {config_type.value} config")
            except Exception as e:
                logger.error(f"Failed to reload {config_type.value} config: {e}")
        
        logger.debug("Configuration reload completed")
    
    # ============ 新增便捷方法（方案B：统一配置管理）============
    
    def add_service_config(self, service_name: str, config: Dict[str, Any]) -> bool:
        """添加服务配置（语义化方法）
        
        Args:
            service_name: 服务名称
            config: 服务配置
            
        Returns:
            bool: 添加是否成功
        """
        try:
            current_config = self.get_mcp_config()
            
            # 确保 mcpServers 存在
            if "mcpServers" not in current_config:
                current_config["mcpServers"] = {}
            
            # 添加服务配置
            current_config["mcpServers"][service_name] = config
            
            # 保存并自动刷新缓存
            result = self.update_mcp_config(current_config)
            
            if result:
                logger.debug(f"✅ 服务 '{service_name}' 配置已添加，缓存已同步")
            
            return result
            
        except Exception as e:
            logger.error(f"Failed to add service config for {service_name}: {e}")
            return False
    
    def remove_service_config(self, service_name: str) -> bool:
        """删除服务配置
        
        Args:
            service_name: 服务名称
            
        Returns:
            bool: 删除是否成功
        """
        try:
            current_config = self.get_mcp_config()
            
            # 如果服务存在，则删除
            if service_name in current_config.get("mcpServers", {}):
                del current_config["mcpServers"][service_name]
                
                # 保存并自动刷新缓存
                result = self.update_mcp_config(current_config)
                
                if result:
                    logger.debug(f"✅ 服务 '{service_name}' 配置已删除，缓存已同步")
                
                return result
            else:
                # 服务不存在，视为成功（幂等性）
                logger.debug(f"服务 '{service_name}' 不存在，无需删除")
                return True
            
        except Exception as e:
            logger.error(f"Failed to remove service config for {service_name}: {e}")
            return False
    
    def batch_add_services(self, services: Dict[str, Dict[str, Any]]) -> bool:
        """批量添加服务配置（原子操作，一次性保存）
        
        Args:
            services: 服务配置字典 {service_name: service_config}
            
        Returns:
            bool: 批量添加是否成功
        """
        try:
            if not services:
                logger.debug("批量添加服务：服务列表为空，无需操作")
                return True
            
            current_config = self.get_mcp_config()
            
            # 确保 mcpServers 存在
            if "mcpServers" not in current_config:
                current_config["mcpServers"] = {}
            
            # 批量合并服务配置
            for service_name, service_config in services.items():
                current_config["mcpServers"][service_name] = service_config
            
            # 一次性保存（原子操作）并自动刷新缓存
            result = self.update_mcp_config(current_config)
            
            if result:
                logger.debug(f"✅ 批量添加 {len(services)} 个服务成功，缓存已同步")
            
            return result
            
        except Exception as e:
            logger.error(f"Failed to batch add services: {e}")
            return False
    
    def batch_remove_services(self, service_names: List[str]) -> bool:
        """批量删除服务配置（原子操作，一次性保存）
        
        Args:
            service_names: 服务名称列表
            
        Returns:
            bool: 批量删除是否成功
        """
        try:
            if not service_names:
                logger.debug("批量删除服务：服务列表为空，无需操作")
                return True
            
            current_config = self.get_mcp_config()
            servers = current_config.get("mcpServers", {})
            
            # 批量删除服务
            removed_count = 0
            for service_name in service_names:
                if service_name in servers:
                    del servers[service_name]
                    removed_count += 1
            
            # 一次性保存（原子操作）并自动刷新缓存
            result = self.update_mcp_config(current_config)
            
            if result:
                logger.debug(f"✅ 批量删除 {removed_count}/{len(service_names)} 个服务成功，缓存已同步")
            
            return result
            
        except Exception as e:
            logger.error(f"Failed to batch remove services: {e}")
            return False
    
    async def update_mcp_config_async(self, config: Dict[str, Any]) -> bool:
        """更新MCP配置（异步版本，带并发保护）
        
        Args:
            config: 新的MCP配置
            
        Returns:
            bool: 更新是否成功
        """
        async with self._config_lock:
            try:
                result = self.mcp_config.save_config(config)
                if result:
                    self._refresh_cache(ConfigType.MCP_SERVICES)
                    logger.debug("✅ MCP配置已更新（异步），缓存已同步")
                return result
            except Exception as e:
                logger.error(f"Failed to update MCP config (async): {e}")
                return False
    
    async def add_service_config_async(self, service_name: str, config: Dict[str, Any]) -> bool:
        """添加服务配置（异步版本，带并发保护）
        
        Args:
            service_name: 服务名称
            config: 服务配置
            
        Returns:
            bool: 添加是否成功
        """
        async with self._config_lock:
            return self.add_service_config(service_name, config)
    
    async def batch_add_services_async(self, services: Dict[str, Dict[str, Any]]) -> bool:
        """批量添加服务配置（异步版本，带并发保护）
        
        Args:
            services: 服务配置字典
            
        Returns:
            bool: 批量添加是否成功
        """
        async with self._config_lock:
            return self.batch_add_services(services)
