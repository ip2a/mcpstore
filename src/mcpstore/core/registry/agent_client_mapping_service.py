"""
Agent-Client Mapping Service for ServiceRegistry

Manages the mapping relationships between agents, clients, and services.
Extracted from core_registry.py to reduce God Object complexity.
"""

import logging
from typing import Dict, Any, Optional, List, TYPE_CHECKING

if TYPE_CHECKING:
    from key_value.aio.protocols import AsyncKeyValue
    from .state_backend import RegistryStateBackend

logger = logging.getLogger(__name__)


class AgentClientMappingService:
    """
    Manages Agent-Client and Service-Client mapping relationships.
    
    Responsibilities:
    - Agent to Client ID mappings
    - Service to Client ID mappings
    - Reverse lookups and queries
    """
    
    def __init__(self, kv_store: 'AsyncKeyValue', state_backend: 'RegistryStateBackend', kv_adapter):
        """
        Initialize Agent-Client mapping service.

        Args:
            kv_store: AsyncKeyValue instance for data storage
            state_backend: Registry state backend for KV operations
            kv_adapter: KV storage adapter for sync operations
        """
        self._kv_store = kv_store
        self._state_backend = state_backend
        self._kv_adapter = kv_adapter

        # agent_clients removed - now reads directly from pyvk
        # service_to_client removed - now reads directly from pyvk
        # This eliminates memory/pyvk inconsistency issues
        # Access via: self._state_backend.list_agent_clients(agent_id)
    
    # === Agent-Client Mapping Methods ===

    def add_agent_client_mapping(self, agent_id: str, client_id: str):
        """添加 Agent-Client 映射 (no-op - derived from service_client mappings)"""
        # Agent-client mappings are now derived from service_client mappings
        # No need to store separately - list_agent_clients() extracts unique client_ids
        logger.debug(f"[MAPPING] add_agent_client_mapping is now a no-op (derived from service_client)")
        logger.debug(f"[MAPPING] agent_id={agent_id}, client_id={client_id}")
    
    def remove_agent_client_mapping(self, agent_id: str, client_id: str):
        """移除 Agent-Client 映射 (no-op - derived from service_client mappings)"""
        # Agent-client mappings are now derived from service_client mappings
        # No need to remove separately - list_agent_clients() will reflect changes
        logger.debug(f"[MAPPING] remove_agent_client_mapping is now a no-op (derived from service_client)")
        logger.debug(f"[MAPPING] agent_id={agent_id}, client_id={client_id}")
    
    async def get_agent_clients_async(self, agent_id: str) -> List[str]:
        """
        从 pykv 获取 Agent 的所有 Client ID
        
        [pykv 唯一真相源] 所有数据必须从 pykv 读取
        """
        result = await self._state_backend.list_agent_clients(agent_id)
        logger.debug(f"[MAPPING] Get agent_clients for {agent_id} -> {len(result)} clients (pykv)")
        return result
    
    def get_all_agent_ids(self) -> List[str]:
        """获取所有Agent ID列表 (从运行时数据推导)"""
        # Since agent_clients is removed, we need to derive agent_ids from other sources
        # This method is called by core_registry, so we need to access registry data
        # For now, return empty list - caller should use registry's in-memory structures
        logger.warning("[MAPPING] get_all_agent_ids() called on AgentClientMappingService - should use registry data")
        return []
    
    def has_agent_client(self, agent_id: str, client_id: str) -> bool:
        """检查指定的 Agent-Client 映射是否存在 (从 pyvk 读取)"""
        try:
            helper = self._kv_adapter._ensure_sync_helper()
            client_ids = helper.run_async(
                self._state_backend.list_agent_clients(agent_id),
                timeout=5.0
            )
            return client_id in client_ids
        except Exception as e:
            logger.warning(f"[MAPPING] Failed to check agent_client for {agent_id}/{client_id}: {e}")
            return False
    
    def clear_agent_client_mappings(self, agent_id: str):
        """清除指定 agent 的所有 client 映射 (no-op - derived from service_client)"""
        # Agent-client mappings are derived from service_client mappings
        # Clearing service_client mappings will automatically clear agent_clients
        logger.debug(f"[MAPPING] clear_agent_client_mappings is now a no-op (derived from service_client)")
        logger.debug(f"[MAPPING] agent_id={agent_id}")
    
    # === Service-Client Mapping Methods ===

    def add_service_client_mapping(self, agent_id: str, service_name: str, client_id: str):
        """添加 Service-Client 映射 (直接写入 pyvk)"""
        logger.debug(f"[MAPPING] Adding service-client mapping: {agent_id}:{service_name} -> {client_id}")
        # Single source of truth: write directly to pyvk only
        self._kv_adapter.sync_to_kv(
            self.set_service_client_mapping_async(agent_id, service_name, client_id),
            f"service_client:{agent_id}:{service_name}"
        )
        logger.debug(f"[MAPPING] Successfully mapped service {service_name} to client {client_id} for agent {agent_id} (pyvk)")

    def remove_service_client_mapping(self, agent_id: str, service_name: str):
        """移除 Service-Client 映射 (直接从 pyvk 删除)"""
        # DEBUG: Track who is removing mappings
        import traceback
        logger.debug(f"[MAPPING] Removing service-client mapping for {agent_id}:{service_name}")
        logger.debug(f"[MAPPING] Call stack:\n" + "\n".join(traceback.format_stack()[-3:]))

        # Single source of truth: delete directly from pyvk only
        self._kv_adapter.sync_to_kv(
            self.delete_service_client_mapping_async(agent_id, service_name),
            f"service_client:{agent_id}:{service_name}"
        )
        logger.debug(f"[MAPPING] Removed service-client mapping for {agent_id}:{service_name} (pyvk)")

    def get_service_client_id(self, agent_id: str, service_name: str) -> Optional[str]:
        """获取服务对应的 Client ID (直接从 pyvk 读取)"""
        # Single source of truth: read directly from pyvk
        try:
            helper = self._kv_adapter._ensure_sync_helper()
            result = helper.run_async(
                self.get_service_client_id_async(agent_id, service_name),
                timeout=5.0
            )
            logger.debug(f"[MAPPING] Get service_client_id for {agent_id}:{service_name} -> {result} (pyvk)")
            return result
        except Exception as e:
            logger.warning(f"[MAPPING] Failed to get service_client_id for {agent_id}:{service_name}: {e}")
            return None

    def get_service_client_mapping(self, agent_id: str) -> Dict[str, str]:
        """获取指定 agent 的所有 service-client 映射 (直接从 pyvk 读取)"""
        # Single source of truth: read directly from pyvk
        try:
            helper = self._kv_adapter._ensure_sync_helper()
            result = helper.run_async(
                self.get_service_client_mapping_async(agent_id),
                timeout=5.0
            )
            logger.debug(f"[MAPPING] Get service_client_mapping for {agent_id} -> {len(result)} mappings (pyvk)")
            return result
        except Exception as e:
            logger.warning(f"[MAPPING] Failed to get service_client_mapping for {agent_id}: {e}")
            return {}

    def get_client_by_service(self, agent_id: str, service_name: str) -> Optional[str]:
        """根据服务名获取对应的 Client ID（别名方法）"""
        return self.get_service_client_id(agent_id, service_name)
    
    # === Async Methods for KV Storage ===

    async def set_service_client_mapping_async(self, agent_id: str, service_name: str, client_id: str) -> None:
        """异步设置 Service-Client 映射到 KV 存储"""
        await self._state_backend.set_service_client(agent_id, service_name, client_id)

    async def delete_service_client_mapping_async(self, agent_id: str, service_name: str) -> None:
        """异步删除 Service-Client 映射从 KV 存储"""
        await self._state_backend.delete_service_client(agent_id, service_name)

    async def get_service_client_id_async(self, agent_id: str, service_name: str) -> Optional[str]:
        """异步获取 Service-Client 映射从 KV 存储"""
        return await self._state_backend.get_service_client(agent_id, service_name)

    async def get_service_client_mapping_async(self, agent_id: str) -> Dict[str, str]:
        """异步获取指定 agent 的所有 service-client 映射"""
        return await self._state_backend.get_all_service_clients(agent_id)
