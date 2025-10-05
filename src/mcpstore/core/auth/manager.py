"""
FastMCP Authentication Configuration Manager
认证配置管理器 - 管理FastMCP认证配置的存储和检索
"""

import json
import logging
from pathlib import Path
from typing import Dict, Optional

from .builder import generate_fastmcp_auth_config
from .types import (
    AuthProviderConfig,
    HubAuthConfig,
    FastMCPAuthConfig
)

logger = logging.getLogger(__name__)


class AuthConfigManager:
    """FastMCP认证配置管理器 - 专门用于管理FastMCP认证配置"""
    
    def __init__(self, base_dir: Optional[Path] = None):
        self.base_dir = base_dir or Path.cwd()
        self.auth_config_dir = self.base_dir / ".mcpstore" / "auth"
        self.auth_config_dir.mkdir(parents=True, exist_ok=True)
        
        # 配置文件路径
        self.provider_config_file = self.auth_config_dir / "providers.json"
        self.hub_config_file = self.auth_config_dir / "hubs.json"
        
        # 内存缓存
        self._provider_configs: Dict[str, AuthProviderConfig] = {}
        self._hub_configs: Dict[str, HubAuthConfig] = {}
        
        # 加载现有配置
        self._load_configs()
        
        logger.debug(f"AuthConfigManager initialized with config dir: {self.auth_config_dir}")
    
    def _load_configs(self):
        """加载所有认证配置"""
        try:
            # 加载认证提供者配置
            if self.provider_config_file.exists():
                with open(self.provider_config_file, 'r', encoding='utf-8') as f:
                    provider_data = json.load(f)
                    for provider_id, config_data in provider_data.items():
                        try:
                            self._provider_configs[provider_id] = AuthProviderConfig(**config_data)
                        except Exception as e:
                            logger.error(f"Failed to load provider config {provider_id}: {e}")
            
            # 加载Hub配置
            if self.hub_config_file.exists():
                with open(self.hub_config_file, 'r', encoding='utf-8') as f:
                    hub_data = json.load(f)
                    for hub_id, config_data in hub_data.items():
                        try:
                            self._hub_configs[hub_id] = HubAuthConfig(**config_data)
                        except Exception as e:
                            logger.error(f"Failed to load hub config {hub_id}: {e}")
            
            logger.debug(f"Loaded {len(self._provider_configs)} provider configs and {len(self._hub_configs)} hub configs")
            
        except Exception as e:
            logger.error(f"Error loading auth configs: {e}")
    
    def _save_provider_configs(self):
        """保存认证提供者配置"""
        try:
            provider_data = {
                provider_id: config.dict() 
                for provider_id, config in self._provider_configs.items()
            }
            with open(self.provider_config_file, 'w', encoding='utf-8') as f:
                json.dump(provider_data, f, indent=2, ensure_ascii=False, default=str)
            logger.debug(f"Saved {len(provider_data)} provider configs")
        except Exception as e:
            logger.error(f"Error saving provider configs: {e}")
    
    def _save_hub_configs(self):
        """保存Hub配置"""
        try:
            hub_data = {
                hub_id: config.dict() 
                for hub_id, config in self._hub_configs.items()
            }
            with open(self.hub_config_file, 'w', encoding='utf-8') as f:
                json.dump(hub_data, f, indent=2, ensure_ascii=False, default=str)
            logger.debug(f"Saved {len(hub_data)} hub configs")
        except Exception as e:
            logger.error(f"Error saving hub configs: {e}")
    
    # === 认证提供者管理 ===
    
    def store_provider_config(self, provider_id: str, config: AuthProviderConfig):
        """存储认证提供者配置"""
        self._provider_configs[provider_id] = config
        self._save_provider_configs()
        logger.info(f"Stored provider config: {provider_id}")
    
    def get_provider_config(self, provider_id: str) -> Optional[AuthProviderConfig]:
        """获取认证提供者配置"""
        return self._provider_configs.get(provider_id)
    
    def remove_provider_config(self, provider_id: str) -> bool:
        """移除认证提供者配置"""
        if provider_id in self._provider_configs:
            del self._provider_configs[provider_id]
            self._save_provider_configs()
            logger.info(f"Removed provider config: {provider_id}")
            return True
        return False
    
    def list_provider_configs(self) -> Dict[str, AuthProviderConfig]:
        """列出所有认证提供者配置"""
        return self._provider_configs.copy()
    
    # === Hub认证配置管理 ===
    
    def store_hub_config(self, hub_name: str, config: HubAuthConfig):
        """存储Hub认证配置"""
        self._hub_configs[hub_name] = config
        self._save_hub_configs()
        logger.info(f"Stored hub auth config: {hub_name}")
    
    def get_hub_config(self, hub_name: str) -> Optional[HubAuthConfig]:
        """获取Hub认证配置"""
        return self._hub_configs.get(hub_name)
    
    def remove_hub_config(self, hub_name: str) -> bool:
        """移除Hub认证配置"""
        if hub_name in self._hub_configs:
            del self._hub_configs[hub_name]
            self._save_hub_configs()
            logger.info(f"Removed hub auth config: {hub_name}")
            return True
        return False
    
    def list_hub_configs(self) -> Dict[str, HubAuthConfig]:
        """列出所有Hub认证配置"""
        return self._hub_configs.copy()
    
    # === FastMCP配置生成 ===
    
    def generate_fastmcp_auth_config(self, provider_id: str) -> Optional[FastMCPAuthConfig]:
        """为指定的认证提供者生成FastMCP配置"""
        try:
            provider_config = self._provider_configs.get(provider_id)
            if not provider_config:
                logger.error(f"Provider config not found: {provider_id}")
                return None
            
            fastmcp_config = generate_fastmcp_auth_config(provider_config)
            logger.info(f"Generated FastMCP auth config for provider: {provider_id}")
            return fastmcp_config
            
        except Exception as e:
            logger.error(f"Failed to generate FastMCP auth config for provider {provider_id}: {e}")
            return None
    
    def generate_hub_fastmcp_auth_config(self, hub_name: str) -> Optional[FastMCPAuthConfig]:
        """为指定的Hub生成FastMCP配置"""
        try:
            hub_config = self._hub_configs.get(hub_name)
            if not hub_config or not hub_config.auth_provider:
                logger.error(f"Hub auth config not found or no auth provider: {hub_name}")
                return None
            
            fastmcp_config = generate_fastmcp_auth_config(hub_config.auth_provider)
            logger.info(f"Generated FastMCP auth config for hub: {hub_name}")
            return fastmcp_config
            
        except Exception as e:
            logger.error(f"Failed to generate FastMCP auth config for hub {hub_name}: {e}")
            return None


# 全局实例
_global_auth_config_manager: Optional[AuthConfigManager] = None


def get_auth_config_manager(base_dir: Optional[Path] = None) -> AuthConfigManager:
    """获取全局认证配置管理器实例"""
    global _global_auth_config_manager
    if _global_auth_config_manager is None:
        _global_auth_config_manager = AuthConfigManager(base_dir)
    return _global_auth_config_manager


def reset_auth_config_manager():
    """重置全局认证配置管理器（主要用于测试）"""
    global _global_auth_config_manager
    _global_auth_config_manager = None
