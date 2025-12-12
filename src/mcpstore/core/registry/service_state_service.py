"""
Service State Service for ServiceRegistry

Manages service lifecycle states and metadata.
Extracted from core_registry.py to reduce God Object complexity.
"""

import logging
from typing import Dict, Any, Optional, List, TYPE_CHECKING

from ..models.service import ServiceConnectionState, ServiceStateMetadata
from .exception_mapper import map_kv_exception

if TYPE_CHECKING:
    from key_value.aio.protocols import AsyncKeyValue
    from .state_backend import RegistryStateBackend

logger = logging.getLogger(__name__)


class ServiceStateService:
    """
    Manages service lifecycle states and metadata.
    
    Responsibilities:
    - Service connection state management
    - Service metadata storage and retrieval
    - State synchronization to KV storage
    """
    
    def __init__(self, kv_store: 'AsyncKeyValue', state_backend: 'RegistryStateBackend', kv_adapter, sync_helper):
        """
        Initialize Service State service.

        Args:
            kv_store: AsyncKeyValue instance for data storage
            state_backend: Registry state backend for KV operations
            kv_adapter: KV storage adapter for sync operations
            sync_helper: AsyncSyncHelper for sync-to-async conversion (or lambda that returns it)
        """
        self._kv_store = kv_store
        self._state_backend = state_backend
        self._kv_adapter = kv_adapter
        # Handle both direct object and lambda for backward compatibility
        self._sync_helper_provider = sync_helper
        if callable(sync_helper) and not hasattr(sync_helper, 'run_async'):
            # It's a lambda, not an AsyncSyncHelper object
            self._sync_helper = None
        else:
            # It's already an AsyncSyncHelper object
            self._sync_helper = sync_helper
        
        # 注意：这些内存缓存字段已废弃，应使用新的 StateManager
        # 保留这些字段仅用于过渡期间的兼容性
        # TODO: 完全迁移到 StateManager 后删除这些字段
        # agent_id -> {service_name: ServiceConnectionState}
        self.service_states: Dict[str, Dict[str, ServiceConnectionState]] = {}
        
        # agent_id -> {service_name: ServiceStateMetadata}
        self.service_metadata: Dict[str, Dict[str, ServiceStateMetadata]] = {}

    def _get_sync_helper(self):
        """Get the sync_helper, handling lambda provider"""
        if self._sync_helper is None:
            if callable(self._sync_helper_provider):
                self._sync_helper = self._sync_helper_provider()
            else:
                raise RuntimeError("sync_helper provider is not callable")
        return self._sync_helper

    # === Service State Management Methods ===
    
    def set_service_state(self, agent_id: str, service_name: str, state: Optional[ServiceConnectionState]):
        """设置服务生命周期状态并同步到 KV 存储。

        ServiceStateService 只负责本地缓存和 KV，同步共享 Client ID
        的跨服务状态由 ServiceRegistry 统一协调。
        """
        # 记录旧状态（仅用于日志）
        old_state = self.service_states.get(agent_id, {}).get(service_name)

        # 设置新状态（现有逻辑）
        if agent_id not in self.service_states:
            self.service_states[agent_id] = {}
        
        if state is None:
            # 删除状态
            if service_name in self.service_states[agent_id]:
                del self.service_states[agent_id][service_name]
                logger.debug(f"Service {service_name} (agent {agent_id}) state removed")
        else:
            # 设置状态
            self.service_states[agent_id][service_name] = state
            logger.debug(f"Service {service_name} (agent {agent_id}) state {getattr(old_state,'value',old_state)} -> {getattr(state,'value',state)}")
            # INFO级别记录状态变化以辅助诊断
            logger.info(f"[REGISTRY_STATE] {agent_id}:{service_name} {getattr(old_state,'value',old_state)} -> {getattr(state,'value',state)}")
            
            # Sync state to KV store
            self._kv_adapter.sync_to_kv(
                self.set_service_state_async(agent_id, service_name, state),
                f"service_state:{agent_id}:{service_name}"
            )
    
    def get_service_state(self, agent_id: str, service_name: str) -> ServiceConnectionState:
        """获取服务生命周期状态"""
        sync_helper = self._get_sync_helper()
        try:
            state = sync_helper.run_async(
                self.get_service_state_async(agent_id, service_name),
                timeout=5.0
            )
        except Exception as e:
            logger.error(f"[REGISTRY] get_service_state async error for {agent_id}/{service_name}: {e}")
            raise
        if state is not None:
            return state
        return self.service_states.get(agent_id, {}).get(service_name, ServiceConnectionState.DISCONNECTED)
    
    def set_service_metadata(self, agent_id: str, service_name: str, metadata: Optional[ServiceStateMetadata]):
        """[REFACTOR] 设置服务状态元数据，支持删除操作"""
        if agent_id not in self.service_metadata:
            self.service_metadata[agent_id] = {}
        
        if metadata is None:
            # 删除元数据
            if service_name in self.service_metadata[agent_id]:
                del self.service_metadata[agent_id][service_name]
                logger.debug(f"Service {service_name} (agent {agent_id}) metadata removed")
        else:
            # 设置元数据
            self.service_metadata[agent_id][service_name] = metadata
            logger.debug(f"Service {service_name} (agent {agent_id}) metadata updated")
            
            # Sync metadata to KV store
            self._kv_adapter.sync_to_kv(
                self.set_service_metadata_async(agent_id, service_name, metadata),
                f"service_metadata:{agent_id}:{service_name}"
            )
    
    def get_service_metadata(self, agent_id: str, service_name: str) -> Optional[ServiceStateMetadata]:
        """获取服务状态元数据"""
        sync_helper = self._get_sync_helper()
        try:
            metadata = sync_helper.run_async(
                self.get_service_metadata_async(agent_id, service_name),
                timeout=5.0
            )
        except Exception as e:
            logger.error(f"[REGISTRY] get_service_metadata async error for {agent_id}/{service_name}: {e}")
            raise
        if metadata is not None:
            return metadata
        return self.service_metadata.get(agent_id, {}).get(service_name)
    
    def get_all_service_names(self, agent_id: str) -> List[str]:
        """
        获取指定 agent_id 下所有已注册服务名。
        修复：从service_states获取服务列表，而不是sessions（sessions可能为空）
        """
        return list(self.service_states.get(agent_id, {}).keys())
    
    def has_service(self, agent_id: str, name: str) -> bool:
        """
        判断指定 agent_id 下是否存在某服务。
        修复：从service_states判断服务是否存在，而不是sessions（sessions可能为空）
        """
        return name in self.service_states.get(agent_id, {})
    
    # === Async Methods for KV Storage ===
    
    @map_kv_exception
    async def set_service_state_async(self, agent_id: str, service_name: str, state: ServiceConnectionState) -> None:
        """
        在 py-key-value 存储中设置服务状态。
        
        Args:
            agent_id: Agent ID
            service_name: 服务名称
            state: 要设置的 ServiceConnectionState
        
        Note:
            此方法同时更新内存缓存（过渡期间保留）。
        
        Raises:
            CacheOperationError: 如果缓存操作失败
            CacheConnectionError: 如果缓存连接失败
            CacheValidationError: 如果数据验证失败
        """
        # 委托给 KV 支持的状态后端
        await self._state_backend.set_service_state(agent_id, service_name, state)
        
        # 同时更新内存缓存（过渡期间保留）
        if agent_id not in self.service_states:
            self.service_states[agent_id] = {}
        self.service_states[agent_id][service_name] = state
        
        logger.debug(f"Set service state: agent={agent_id}, service={service_name}, state={state}")
    
    @map_kv_exception
    async def get_service_state_async(self, agent_id: str, service_name: str) -> Optional[ServiceConnectionState]:
        """
        从 py-key-value 存储中获取服务状态。
        
        Args:
            agent_id: Agent ID
            service_name: 服务名称
        
        Returns:
            ServiceConnectionState 或 None（如果未找到）
        
        Note:
            这是从 py-key-value 读取的异步版本。
            同步版本仍使用内存缓存（过渡期间保留）。
        
        Raises:
            CacheOperationError: 如果缓存操作失败
            CacheConnectionError: 如果缓存连接失败
            CacheValidationError: 如果数据验证失败
        """
        # 委托给 KV 支持的状态后端
        return await self._state_backend.get_service_state(agent_id, service_name)
    
    @map_kv_exception
    async def delete_service_state_async(self, agent_id: str, service_name: str) -> None:
        """删除服务状态从 KV 存储"""
        # Delegate to KV-backed state backend
        await self._state_backend.delete_service_state(agent_id, service_name)
    
    @map_kv_exception
    async def set_service_metadata_async(self, agent_id: str, service_name: str, metadata: ServiceStateMetadata) -> None:
        """
        在 py-key-value 存储中设置服务元数据。
        
        Args:
            agent_id: Agent ID
            service_name: 服务名称
            metadata: 要设置的 ServiceStateMetadata
        
        Note:
            此方法同时更新内存缓存（过渡期间保留）。
        
        Raises:
            CacheOperationError: 如果缓存操作失败
            CacheConnectionError: 如果缓存连接失败
            CacheValidationError: 如果数据验证失败
        """
        # 委托给 KV 支持的状态后端
        await self._state_backend.set_service_metadata(agent_id, service_name, metadata)
        
        # 同时更新内存缓存（过渡期间保留）
        if agent_id not in self.service_metadata:
            self.service_metadata[agent_id] = {}
        self.service_metadata[agent_id][service_name] = metadata
        
        logger.debug(f"Set service metadata: agent={agent_id}, service={service_name}")
    
    @map_kv_exception
    async def get_service_metadata_async(self, agent_id: str, service_name: str) -> Optional[ServiceStateMetadata]:
        """
        从 py-key-value 存储中获取服务元数据。
        
        Args:
            agent_id: Agent ID
            service_name: 服务名称
        
        Returns:
            ServiceStateMetadata 或 None（如果未找到）
        
        Note:
            这是从 py-key-value 读取的异步版本。
            同步版本仍使用内存缓存（过渡期间保留）。
        
        Raises:
            CacheOperationError: 如果缓存操作失败
            CacheConnectionError: 如果缓存连接失败
            CacheValidationError: 如果数据验证失败
        """
        # 委托给 KV 支持的状态后端
        return await self._state_backend.get_service_metadata(agent_id, service_name)
    
    @map_kv_exception
    async def delete_service_metadata_async(self, agent_id: str, service_name: str) -> None:
        """删除服务元数据从 KV 存储"""
        # Delegate to KV-backed state backend
        await self._state_backend.delete_service_metadata(agent_id, service_name)
