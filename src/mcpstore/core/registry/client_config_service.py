"""
Client Configuration Service for ServiceRegistry

Manages client configuration storage and retrieval.
Extracted from core_registry.py to reduce God Object complexity.
"""

import logging
from typing import Dict, Any, Optional, TYPE_CHECKING

from .exception_mapper import map_kv_exception

if TYPE_CHECKING:
    from key_value.aio.protocols import AsyncKeyValue
    from .state_backend import RegistryStateBackend

logger = logging.getLogger(__name__)


class ClientConfigService:
    """
    Manages client configuration data.
    
    Responsibilities:
    - Store and retrieve client configurations
    - Update and delete client configs
    - Sync configurations to KV storage
    """
    
    def __init__(self, kv_store: 'AsyncKeyValue', state_backend: 'RegistryStateBackend', kv_adapter):
        """
        Initialize Client Configuration service.

        Args:
            kv_store: AsyncKeyValue instance for data storage
            state_backend: Registry state backend for KV operations
            kv_adapter: KV storage adapter for sync operations
        """
        self._kv_store = kv_store
        self._state_backend = state_backend
        self._kv_adapter = kv_adapter

        # client_configs removed - now read from pyvk only (single source of truth)
    
    # === Client Configuration Management Methods ===
    
    def add_client_config(self, client_id: str, config: Dict[str, Any]):
        """添加 Client 配置到 pyvk (单一数据源)"""
        # Write directly to pyvk
        self._kv_adapter.sync_to_kv(
            self.set_client_config_async(client_id, config),
            f"client_config:{client_id}"
        )
        logger.debug(f"Added client config for {client_id} to pyvk")
    
    def update_client_config(self, client_id: str, updates: Dict[str, Any]):
        """更新 pyvk 中的 Client 配置"""
        # Read current config from pyvk, merge updates, write back
        helper = self._kv_adapter._ensure_sync_helper()
        try:
            current_config = helper.run_async(
                self._state_backend.get_client_config(client_id),
                timeout=5.0
            )
            if current_config:
                current_config.update(updates)
            else:
                current_config = updates

            self._kv_adapter.sync_to_kv(
                self.set_client_config_async(client_id, current_config),
                f"client_config:{client_id}"
            )
        except Exception as e:
            logger.warning(f"Failed to update client config for {client_id}: {e}")
            # Fallback: just write the updates
            self._kv_adapter.sync_to_kv(
                self.set_client_config_async(client_id, updates),
                f"client_config:{client_id}"
            )
    
    def remove_client_config(self, client_id: str):
        """从 pyvk 移除 Client 配置"""
        # Delete directly from pyvk
        self._kv_adapter.sync_to_kv(
            self.delete_client_config_async(client_id),
            f"client_config:{client_id}"
        )
    
    def get_client_config_from_cache(self, client_id: str) -> Optional[Dict[str, Any]]:
        """从 pyvk 获取 Client 配置"""
        # Read directly from pyvk (single source of truth)
        helper = self._kv_adapter._ensure_sync_helper()
        try:
            result = helper.run_async(
                self._state_backend.get_client_config(client_id),
                timeout=5.0
            )
            return result
        except Exception as e:
            logger.warning(f"Failed to get client config for {client_id}: {e}")
            return None

    def has_client_config(self, client_id: str) -> bool:
        """检查指定的 Client 配置是否存在"""
        config = self.get_client_config_from_cache(client_id)
        return config is not None

    def get_all_client_configs(self) -> Dict[str, Dict[str, Any]]:
        """获取所有 Client 配置"""
        # Note: This method is not efficient as it requires listing all keys
        # Consider deprecating or implementing a key listing mechanism in state_backend
        logger.warning("get_all_client_configs() is not efficient with pyvk backend")
        return {}
    
    # === Async Methods for KV Storage ===
    
    @map_kv_exception
    async def set_client_config_async(self, client_id: str, config: Dict[str, Any]) -> None:
        """异步设置 Client 配置到 KV 存储"""
        await self._state_backend.set_client_config(client_id, config)
    
    @map_kv_exception
    async def get_client_config_async(self, client_id: str) -> Optional[Dict[str, Any]]:
        """异步从 KV 存储获取 Client 配置"""
        return await self._state_backend.get_client_config(client_id)
    
    @map_kv_exception
    async def delete_client_config_async(self, client_id: str) -> None:
        """异步从 KV 存储删除 Client 配置"""
        await self._state_backend.delete_client_config(client_id)
