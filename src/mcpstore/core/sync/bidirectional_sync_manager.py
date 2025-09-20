"""
双向同步管理器

处理 Store ↔ Agent 之间的配置同步，确保：
1. Agent 添加/修改/删除服务时，自动同步到 Store
2. Store 修改 Agent 服务时，自动同步到对应的 Agent
3. 保持 mcp.json 和两个 JSON 文件的一致性

设计原则:
1. 自动透明同步
2. 原子性操作
3. 错误容错机制
4. 详细的同步日志
"""

import logging
from typing import Dict, Any, Optional, List, Tuple
from mcpstore.core.context.agent_service_mapper import AgentServiceMapper

logger = logging.getLogger(__name__)

class BidirectionalSyncManager:
    """Store ↔ Agent 双向配置同步管理器"""
    
    def __init__(self, store):
        """
        初始化双向同步管理器
        
        Args:
            store: MCPStore 实例
        """
        self.store = store
        self._syncing_services: set = set()  # 防止递归同步的标记
        
    async def sync_agent_to_store(self, agent_id: str, local_name: str, new_config: Dict[str, Any], operation: str = "update"):
        """
        Agent 配置变更同步到 Store
        
        Args:
            agent_id: Agent ID
            local_name: Agent 中的本地服务名
            new_config: 新的服务配置
            operation: 操作类型 ("add", "update", "delete")
        """
        sync_key = f"{agent_id}:{local_name}:{operation}"
        if sync_key in self._syncing_services:
            logger.debug(f" [BIDIRECTIONAL_SYNC] Skipping recursive sync: {sync_key}")
            return
        
        try:
            self._syncing_services.add(sync_key)
            
            global_name = self.store.registry.get_global_name_from_agent_service(agent_id, local_name)
            if not global_name:
                logger.warning(f" [BIDIRECTIONAL_SYNC] No global mapping found for {agent_id}:{local_name}")
                return
            
            logger.info(f" [BIDIRECTIONAL_SYNC] Agent → Store: {agent_id}:{local_name} → {global_name} ({operation})")
            
            if operation == "add" or operation == "update":
                # 更新 Store 中的服务配置
                await self._update_store_service_config(global_name, new_config)
                
            elif operation == "delete":
                # 从 Store 中删除服务
                await self._delete_store_service(global_name)
            
            logger.info(f"✅ [BIDIRECTIONAL_SYNC] Agent → Store 同步完成: {sync_key}")
            
        except Exception as e:
            logger.error(f"❌ [BIDIRECTIONAL_SYNC] Agent → Store 同步失败 {sync_key}: {e}")
        finally:
            self._syncing_services.discard(sync_key)
    
    async def sync_store_to_agent(self, global_name: str, new_config: Dict[str, Any], operation: str = "update"):
        """
        Store 配置变更同步到对应的 Agent
        
        Args:
            global_name: Store 中的全局服务名
            new_config: 新的服务配置
            operation: 操作类型 ("add", "update", "delete")
        """
        sync_key = f"store:{global_name}:{operation}"
        if sync_key in self._syncing_services:
            logger.debug(f" [BIDIRECTIONAL_SYNC] Skipping recursive sync: {sync_key}")
            return
        
        try:
            self._syncing_services.add(sync_key)
            
            # 检查是否为 Agent 服务
            if not AgentServiceMapper.is_any_agent_service(global_name):
                logger.debug(f" [BIDIRECTIONAL_SYNC] Not an Agent service: {global_name}")
                return
            
            # 解析 Agent 信息
            agent_id, local_name = AgentServiceMapper.parse_agent_service_name(global_name)
            
            logger.info(f" [BIDIRECTIONAL_SYNC] Store → Agent: {global_name} → {agent_id}:{local_name} ({operation})")
            
            if operation == "add" or operation == "update":
                # 更新 Agent 中的服务配置
                await self._update_agent_service_config(agent_id, local_name, new_config)
                
            elif operation == "delete":
                # 从 Agent 中删除服务
                await self._delete_agent_service(agent_id, local_name)
            
            logger.info(f"✅ [BIDIRECTIONAL_SYNC] Store → Agent 同步完成: {sync_key}")
            
        except Exception as e:
            logger.error(f"❌ [BIDIRECTIONAL_SYNC] Store → Agent 同步失败 {sync_key}: {e}")
        finally:
            self._syncing_services.discard(sync_key)
    
    async def handle_service_update_with_sync(self, agent_id: str, service_name: str, new_config: Dict[str, Any]):
        """
        带同步的服务更新（统一入口）
        
        Args:
            agent_id: Agent ID（如果是 global_agent_store 则为 Store 操作）
            service_name: 服务名
            new_config: 新配置
        """
        try:
            if agent_id == self.store.client_manager.global_agent_store_id:
                # Store 操作：检查是否需要同步到 Agent
                if AgentServiceMapper.is_any_agent_service(service_name):
                    await self.sync_store_to_agent(service_name, new_config, "update")
            else:
                # Agent 操作：同步到 Store
                await self.sync_agent_to_store(agent_id, service_name, new_config, "update")
            
        except Exception as e:
            logger.error(f"❌ [BIDIRECTIONAL_SYNC] 服务更新同步失败 {agent_id}:{service_name}: {e}")
    
    async def handle_service_deletion_with_sync(self, agent_id: str, service_name: str):
        """
        带同步的服务删除（统一入口）
        
        Args:
            agent_id: Agent ID（如果是 global_agent_store 则为 Store 操作）
            service_name: 服务名
        """
        try:
            if agent_id == self.store.client_manager.global_agent_store_id:
                # Store 操作：检查是否需要同步到 Agent
                if AgentServiceMapper.is_any_agent_service(service_name):
                    await self.sync_store_to_agent(service_name, {}, "delete")
            else:
                # Agent 操作：同步到 Store
                await self.sync_agent_to_store(agent_id, service_name, {}, "delete")
            
        except Exception as e:
            logger.error(f"❌ [BIDIRECTIONAL_SYNC] 服务删除同步失败 {agent_id}:{service_name}: {e}")
    
    # === 内部同步实现方法 ===
    
    async def _update_store_service_config(self, global_name: str, new_config: Dict[str, Any]):
        """更新 Store 中的服务配置"""
        try:
            # 1. 更新 Registry 中的配置
            if hasattr(self.store.registry, 'update_service_config'):
                self.store.registry.update_service_config(
                    self.store.client_manager.global_agent_store_id, 
                    global_name, 
                    new_config
                )
            
            # 2. 更新 mcp.json
            current_mcp_config = self.store.config.load_config()
            if "mcpServers" not in current_mcp_config:
                current_mcp_config["mcpServers"] = {}
            
            current_mcp_config["mcpServers"][global_name] = new_config
            success = self.store.config.save_config(current_mcp_config)
            
            if success:
                logger.debug(f"✅ [BIDIRECTIONAL_SYNC] Store 配置更新成功: {global_name}")
            else:
                logger.error(f"❌ [BIDIRECTIONAL_SYNC] Store 配置更新失败: {global_name}")
            
        except Exception as e:
            logger.error(f"❌ [BIDIRECTIONAL_SYNC] 更新 Store 服务配置失败 {global_name}: {e}")
            raise
    
    async def _update_agent_service_config(self, agent_id: str, local_name: str, new_config: Dict[str, Any]):
        """更新 Agent 中的服务配置"""
        try:
            # 更新 Registry 中的配置
            if hasattr(self.store.registry, 'update_service_config'):
                self.store.registry.update_service_config(agent_id, local_name, new_config)
            
            logger.debug(f"✅ [BIDIRECTIONAL_SYNC] Agent 配置更新成功: {agent_id}:{local_name}")
            
        except Exception as e:
            logger.error(f"❌ [BIDIRECTIONAL_SYNC] 更新 Agent 服务配置失败 {agent_id}:{local_name}: {e}")
            raise
    
    async def _delete_store_service(self, global_name: str):
        """从 Store 中删除服务"""
        try:
            # 1. 从 Registry 中删除
            self.store.registry.remove_service(
                self.store.client_manager.global_agent_store_id, 
                global_name
            )
            
            # 2. 从 mcp.json 中删除
            current_mcp_config = self.store.config.load_config()
            if "mcpServers" in current_mcp_config and global_name in current_mcp_config["mcpServers"]:
                del current_mcp_config["mcpServers"][global_name]
                success = self.store.config.save_config(current_mcp_config)
                
                if success:
                    logger.debug(f"✅ [BIDIRECTIONAL_SYNC] Store 服务删除成功: {global_name}")
                else:
                    logger.error(f"❌ [BIDIRECTIONAL_SYNC] Store 服务删除失败: {global_name}")
            
        except Exception as e:
            logger.error(f"❌ [BIDIRECTIONAL_SYNC] 删除 Store 服务失败 {global_name}: {e}")
            raise
    
    async def _delete_agent_service(self, agent_id: str, local_name: str):
        """从 Agent 中删除服务"""
        try:
            # 从 Registry 中删除
            self.store.registry.remove_service(agent_id, local_name)
            
            # 移除映射关系
            self.store.registry.remove_agent_service_mapping(agent_id, local_name)
            
            logger.debug(f"✅ [BIDIRECTIONAL_SYNC] Agent 服务删除成功: {agent_id}:{local_name}")
            
        except Exception as e:
            logger.error(f"❌ [BIDIRECTIONAL_SYNC] 删除 Agent 服务失败 {agent_id}:{local_name}: {e}")
            raise
    
    def get_sync_status(self) -> Dict[str, Any]:
        """
        获取同步状态信息（用于调试和监控）
        
        Returns:
            Dict: 同步状态信息
        """
        return {
            "currently_syncing": list(self._syncing_services),
            "sync_count": len(self._syncing_services),
            "store_id": self.store.client_manager.global_agent_store_id,
            "agent_mappings": dict(self.store.registry.agent_to_global_mappings)
        }
