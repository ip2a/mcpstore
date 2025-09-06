"""
共享 Client ID 服务状态同步管理器

处理共享同一 client_id 的服务之间的状态同步，确保 Agent 服务和 Store 中对应的
带后缀服务状态保持一致。

设计原则:
1. 对生命周期管理器零侵入
2. 自动透明同步
3. 防止递归同步
4. 详细的同步日志
"""

import asyncio
import logging
from typing import List, Tuple, Set, Optional, Dict
from mcpstore.core.models.service import ServiceConnectionState

logger = logging.getLogger(__name__)

class SharedClientStateSyncManager:
    """共享 Client ID 的服务状态同步管理器"""
    
    def __init__(self, registry):
        """
        初始化状态同步管理器
        
        Args:
            registry: ServiceRegistry 实例
        """
        self.registry = registry
        self._syncing: Set[Tuple[str, str]] = set()  # 防止递归同步的标记
        self._sync_lock = asyncio.Lock()  # 🆕 原子同步锁
        self._batch_sync_queue: Dict[str, List[Tuple[str, str, ServiceConnectionState]]] = {}  # 🆕 批量同步队列
        
    def sync_state_for_shared_client(self, agent_id: str, service_name: str, new_state: ServiceConnectionState):
        """
        为共享 Client ID 的服务同步状态
        
        Args:
            agent_id: 触发状态变更的服务所属 Agent ID
            service_name: 触发状态变更的服务名
            new_state: 新的服务状态
        """
        # 防止递归同步
        sync_key = (agent_id, service_name)
        if sync_key in self._syncing:
            logger.debug(f" [STATE_SYNC] Skipping recursive sync for {agent_id}:{service_name}")
            return
        
        try:
            self._syncing.add(sync_key)
            
            # 获取服务的 client_id
            client_id = self.registry.get_service_client_id(agent_id, service_name)
            if not client_id:
                logger.debug(f" [STATE_SYNC] No client_id found for {agent_id}:{service_name}")
                return
            
            # 查找所有使用相同 client_id 的服务
            shared_services = self._find_all_services_with_client_id(client_id)
            
            if len(shared_services) <= 1:
                logger.debug(f" [STATE_SYNC] No shared services found for client_id {client_id}")
                return
            
            # 同步状态到所有共享服务（排除触发源）
            synced_count = 0
            for target_agent_id, target_service_name in shared_services:
                if (target_agent_id, target_service_name) != (agent_id, service_name):
                    # 获取目标服务的当前状态
                    current_state = self.registry.get_service_state(target_agent_id, target_service_name)
                    
                    if current_state != new_state:
                        # 直接设置状态，避免触发递归同步
                        self._set_state_directly(target_agent_id, target_service_name, new_state)
                        synced_count += 1
                        logger.debug(f" [STATE_SYNC] Synced {new_state.value}: {agent_id}:{service_name} → {target_agent_id}:{target_service_name}")
                    else:
                        logger.debug(f" [STATE_SYNC] State already synced for {target_agent_id}:{target_service_name}")
            
            if synced_count > 0:
                logger.info(f" [STATE_SYNC] Synced state {new_state.value} to {synced_count} shared services for client_id {client_id}")
            else:
                logger.debug(f" [STATE_SYNC] No sync needed for client_id {client_id}")
                
        except Exception as e:
            logger.error(f"❌ [STATE_SYNC] Failed to sync state for {agent_id}:{service_name}: {e}")
        finally:
            self._syncing.discard(sync_key)
    
    def _find_all_services_with_client_id(self, client_id: str) -> List[Tuple[str, str]]:
        """
        查找使用指定 client_id 的所有服务
        
        Args:
            client_id: 要查找的 Client ID
            
        Returns:
            List of (agent_id, service_name) tuples
        """
        services = []
        
        for agent_id, service_mappings in self.registry.service_to_client.items():
            for service_name, mapped_client_id in service_mappings.items():
                if mapped_client_id == client_id:
                    services.append((agent_id, service_name))
        
        logger.debug(f"🔍 [STATE_SYNC] Found {len(services)} services with client_id {client_id}: {services}")
        return services
    
    def _set_state_directly(self, agent_id: str, service_name: str, state: ServiceConnectionState):
        """
        直接设置状态，不触发同步（避免递归）
        
        Args:
            agent_id: Agent ID
            service_name: 服务名
            state: 新状态
        """
        if agent_id not in self.registry.service_states:
            self.registry.service_states[agent_id] = {}
        
        self.registry.service_states[agent_id][service_name] = state
        logger.debug(f" [STATE_SYNC] Direct state set: {agent_id}:{service_name} → {state.value}")
    
    def get_shared_services_info(self, agent_id: str, service_name: str) -> Optional[dict]:
        """
        获取共享服务信息（用于调试和监控）
        
        Args:
            agent_id: Agent ID
            service_name: 服务名
            
        Returns:
            共享服务信息字典，如果没有共享服务则返回 None
        """
        try:
            client_id = self.registry.get_service_client_id(agent_id, service_name)
            if not client_id:
                return None
            
            shared_services = self._find_all_services_with_client_id(client_id)
            if len(shared_services) <= 1:
                return None
            
            # 收集所有共享服务的状态信息
            services_info = []
            for svc_agent_id, svc_service_name in shared_services:
                state = self.registry.get_service_state(svc_agent_id, svc_service_name)
                services_info.append({
                    "agent_id": svc_agent_id,
                    "service_name": svc_service_name,
                    "state": state.value if state else "unknown"
                })
            
            return {
                "client_id": client_id,
                "shared_services_count": len(shared_services),
                "services": services_info
            }
            
        except Exception as e:
            logger.error(f"❌ [STATE_SYNC] Failed to get shared services info for {agent_id}:{service_name}: {e}")
            return None
    
    async def atomic_state_update(self, agent_id: str, service_name: str, new_state: ServiceConnectionState):
        """
        原子状态更新，确保所有共享服务同步更新
        
        Args:
            agent_id: 触发状态变更的服务所属 Agent ID
            service_name: 触发状态变更的服务名
            new_state: 新的服务状态
        """
        async with self._sync_lock:
            try:
                logger.debug(f" [ATOMIC_SYNC] Starting atomic state update: {agent_id}:{service_name} -> {new_state.value}")
                
                # 获取服务的 client_id
                client_id = self.registry.get_service_client_id(agent_id, service_name)
                if not client_id:
                    logger.debug(f" [ATOMIC_SYNC] No client_id found for {agent_id}:{service_name}")
                    return
                
                # 查找所有使用相同 client_id 的服务
                shared_services = self._find_all_services_with_client_id(client_id)
                
                if len(shared_services) <= 1:
                    logger.debug(f" [ATOMIC_SYNC] No shared services found for client_id {client_id}")
                    # 只有一个服务，直接更新
                    self._set_state_directly(agent_id, service_name, new_state)
                    return
                
                # 原子更新所有共享服务的状态
                updated_count = 0
                for target_agent_id, target_service_name in shared_services:
                    self._set_state_directly(target_agent_id, target_service_name, new_state)
                    updated_count += 1
                    logger.debug(f" [ATOMIC_SYNC] Updated {target_agent_id}:{target_service_name} -> {new_state.value}")
                
                logger.info(f" [ATOMIC_SYNC] Atomic update completed: {updated_count} services updated to {new_state.value} for client_id {client_id}")
                
            except Exception as e:
                logger.error(f"❌ [ATOMIC_SYNC] Failed atomic state update for {agent_id}:{service_name}: {e}")
                raise
    
    def validate_state_consistency(self, client_id: str) -> Dict[str, any]:
        """
        验证共享client_id的所有服务状态是否一致
        
        Args:
            client_id: 要验证的 Client ID
            
        Returns:
            Dict: 验证结果
            - consistent: bool 是否一致
            - services: List 所有服务状态
            - inconsistent_services: List 状态不一致的服务
        """
        try:
            logger.debug(f"🔍 [STATE_VALIDATION] Validating state consistency for client_id: {client_id}")
            
            # 查找所有使用该 client_id 的服务
            shared_services = self._find_all_services_with_client_id(client_id)
            
            if len(shared_services) <= 1:
                return {
                    "consistent": True,
                    "services": shared_services,
                    "inconsistent_services": [],
                    "message": f"Only {len(shared_services)} service(s) found, consistency check not applicable"
                }
            
            # 收集所有服务的状态
            service_states = []
            state_groups = {}
            
            for agent_id, service_name in shared_services:
                state = self.registry.get_service_state(agent_id, service_name)
                state_value = state.value if state else "unknown"
                
                service_states.append({
                    "agent_id": agent_id,
                    "service_name": service_name,
                    "state": state_value
                })
                
                # 按状态分组
                if state_value not in state_groups:
                    state_groups[state_value] = []
                state_groups[state_value].append((agent_id, service_name))
            
            # 检查一致性
            is_consistent = len(state_groups) == 1
            inconsistent_services = []
            
            if not is_consistent:
                # 找出不一致的服务（非主要状态的服务）
                main_state = max(state_groups.keys(), key=lambda k: len(state_groups[k]))
                for state_value, services in state_groups.items():
                    if state_value != main_state:
                        inconsistent_services.extend(services)
            
            result = {
                "consistent": is_consistent,
                "services": service_states,
                "inconsistent_services": inconsistent_services,
                "state_groups": state_groups,
                "message": f"Consistency check completed for {len(shared_services)} services"
            }
            
            if is_consistent:
                logger.info(f"✅ [STATE_VALIDATION] State consistency validated for client_id {client_id}: ALL CONSISTENT")
            else:
                logger.warning(f"⚠️ [STATE_VALIDATION] State inconsistency detected for client_id {client_id}: {len(inconsistent_services)} services inconsistent")
            
            return result
            
        except Exception as e:
            logger.error(f"❌ [STATE_VALIDATION] Failed to validate state consistency for client_id {client_id}: {e}")
            return {
                "consistent": False,
                "services": [],
                "inconsistent_services": [],
                "error": str(e),
                "message": "Validation failed due to error"
            }
    
    async def batch_sync_client_states(self, client_id: str, target_state: ServiceConnectionState):
        """
        批量同步指定client_id的所有服务到目标状态
        
        Args:
            client_id: Client ID
            target_state: 目标状态
        """
        async with self._sync_lock:
            try:
                logger.info(f" [BATCH_SYNC] Starting batch sync for client_id {client_id} to {target_state.value}")
                
                # 查找所有使用该 client_id 的服务
                shared_services = self._find_all_services_with_client_id(client_id)
                
                if not shared_services:
                    logger.warning(f"⚠️ [BATCH_SYNC] No services found for client_id {client_id}")
                    return
                
                # 批量更新所有服务状态
                updated_count = 0
                for agent_id, service_name in shared_services:
                    current_state = self.registry.get_service_state(agent_id, service_name)
                    if current_state != target_state:
                        self._set_state_directly(agent_id, service_name, target_state)
                        updated_count += 1
                        logger.debug(f" [BATCH_SYNC] Updated {agent_id}:{service_name}: {current_state} -> {target_state.value}")
                    else:
                        logger.debug(f" [BATCH_SYNC] Skipped {agent_id}:{service_name}: already {target_state.value}")
                
                logger.info(f"✅ [BATCH_SYNC] Batch sync completed: {updated_count}/{len(shared_services)} services updated for client_id {client_id}")
                
            except Exception as e:
                logger.error(f"❌ [BATCH_SYNC] Failed batch sync for client_id {client_id}: {e}")
                raise
    
    def _set_state_directly(self, agent_id: str, service_name: str, new_state: ServiceConnectionState):
        """
        直接设置服务状态，绕过同步机制（用于内部原子操作）
        
        Args:
            agent_id: Agent ID
            service_name: 服务名
            new_state: 新状态
        """
        try:
            # 直接更新registry中的状态，不触发同步
            if agent_id in self.registry.service_states:
                if service_name in self.registry.service_states[agent_id]:
                    old_state = self.registry.service_states[agent_id][service_name]
                    self.registry.service_states[agent_id][service_name] = new_state
                    logger.debug(f" [DIRECT_SET] {agent_id}:{service_name} state: {old_state} -> {new_state.value}")
                else:
                    logger.warning(f"⚠️ [DIRECT_SET] Service {service_name} not found in agent {agent_id}")
            else:
                logger.warning(f"⚠️ [DIRECT_SET] Agent {agent_id} not found in service_states")
                
        except Exception as e:
            logger.error(f"❌ [DIRECT_SET] Failed to set state directly for {agent_id}:{service_name}: {e}")
            raise
