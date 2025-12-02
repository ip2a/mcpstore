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
        
        # Agent-Client mapping cache
        # Structure: {agent_id: [client_id1, client_id2, ...]}
        self.agent_clients: Dict[str, List[str]] = {}
        
        # Service to Client reverse mapping
        # Structure: {agent_id: {service_name: client_id}}
        self.service_to_client: Dict[str, Dict[str, str]] = {}
    
    # === Agent-Client Mapping Methods ===
    
    def add_agent_client_mapping(self, agent_id: str, client_id: str):
        """添加 Agent-Client 映射到缓存（委托后端）"""
        # Use in-memory cache for now (backward compatibility)
        if agent_id not in self.agent_clients:
            self.agent_clients[agent_id] = []
        if client_id not in self.agent_clients[agent_id]:
            self.agent_clients[agent_id].append(client_id)
        logger.debug(f"[REGISTRY] agent_client_mapped client_id={client_id} agent_id={agent_id}")
        logger.debug(f"[REGISTRY] agent_clients={dict(self.agent_clients)}")
    
    def remove_agent_client_mapping(self, agent_id: str, client_id: str):
        """从缓存移除 Agent-Client 映射（委托后端）"""
        # Use in-memory cache for now (backward compatibility)
        if agent_id in self.agent_clients and client_id in self.agent_clients[agent_id]:
            self.agent_clients[agent_id].remove(client_id)
    
    def get_agent_clients_from_cache(self, agent_id: str) -> List[str]:
        """从缓存获取 Agent 的所有 Client ID"""
        # Use in-memory cache for now (backward compatibility)
        return self.agent_clients.get(agent_id, [])
    
    def get_all_agent_ids(self) -> List[str]:
        """从缓存获取所有Agent ID列表"""
        agent_ids = list(self.agent_clients.keys())
        logger.debug(f"[REGISTRY] agent_ids={agent_ids}")
        logger.info(f"[REGISTRY] agent_clients_full={dict(self.agent_clients)}")
        return agent_ids
    
    def has_agent_client(self, agent_id: str, client_id: str) -> bool:
        """检查指定的 Agent-Client 映射是否存在"""
        return client_id in self.agent_clients.get(agent_id, [])
    
    def clear_agent_client_mappings(self, agent_id: str):
        """清除指定 agent 的所有 client 映射"""
        if agent_id in self.agent_clients:
            del self.agent_clients[agent_id]
        logger.debug(f"[REGISTRY] Cleared all client mappings for agent {agent_id}")
    
    # === Service-Client Mapping Methods ===
    
    def add_service_client_mapping(self, agent_id: str, service_name: str, client_id: str):
        """添加 Service-Client 映射到缓存"""
        # 1. 立即更新内存缓存（不依赖 KV 同步）
        if agent_id not in self.service_to_client:
            self.service_to_client[agent_id] = {}
        self.service_to_client[agent_id][service_name] = client_id
        logger.debug(f"Mapped service {service_name} to client {client_id} for agent {agent_id}")
        # 立即验证内存缓存更新
        logger.debug(f"Memory cache verification for {agent_id}: {self.service_to_client.get(agent_id, {})}")

        # 2. 异步同步到 KV（失败不影响内存缓存）
        try:
            self._kv_adapter.sync_to_kv(
                self.set_service_client_mapping_async(agent_id, service_name, client_id),
                f"service_client:{agent_id}:{service_name}"
            )
        except Exception as e:
            logger.warning(f"KV sync failed for service_client mapping {agent_id}:{service_name} -> {client_id}: {e}")
            # 内存缓存已经更新，不影响功能
    
    def remove_service_client_mapping(self, agent_id: str, service_name: str):
        """移除 Service-Client 映射"""
        # Use in-memory cache for now (backward compatibility)
        if agent_id in self.service_to_client and service_name in self.service_to_client[agent_id]:
            del self.service_to_client[agent_id][service_name]
        self._kv_adapter.sync_to_kv(
            self.delete_service_client_mapping_async(agent_id, service_name),
            f"service_client:{agent_id}:{service_name}"
        )
    
    def get_service_client_id(self, agent_id: str, service_name: str) -> Optional[str]:
        """获取服务对应的 Client ID"""
        # Use in-memory cache for now (backward compatibility)
        return self.service_to_client.get(agent_id, {}).get(service_name)
    
    def get_service_client_mapping(self, agent_id: str) -> Dict[str, str]:
        """获取指定 agent 的所有 service-client 映射"""
        return self.service_to_client.get(agent_id, {})
    
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
