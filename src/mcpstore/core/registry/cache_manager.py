import asyncio
import copy
import logging
import time
from datetime import datetime
from typing import Dict, Any, List, Optional, Tuple

from mcpstore.core.models.service import ServiceConnectionState

logger = logging.getLogger(__name__)


class ServiceCacheManager:
    """
    服务缓存管理器 - 提供高级缓存操作
    """
    
    def __init__(self, registry, lifecycle_manager):
        self.registry = registry
        self.lifecycle_manager = lifecycle_manager
    
    # === 🔧 智能缓存操作 ===
    
    async def smart_add_service(self, agent_id: str, service_name: str, service_config: Dict[str, Any]) -> Dict[str, Any]:
        """
        智能添加服务：自动处理连接、状态管理、缓存更新
        
        Returns:
            {
                "success": True,
                "state": "healthy",
                "tools_added": 5,
                "message": "Service added successfully"
            }
        """
        try:
            # 1. 初始化到生命周期管理器
            self.lifecycle_manager.initialize_service(agent_id, service_name, service_config)
            
            # 2. 立即添加到缓存（初始化状态）
            self.registry.add_service(
                agent_id=agent_id,
                name=service_name,
                session=None,
                tools=[],
                service_config=service_config,
                state=ServiceConnectionState.INITIALIZING
            )
            
            return {
                "success": True,
                "state": "initializing",
                "tools_added": 0,
                "message": "Service added to cache, connecting in background"
            }
                
        except Exception as e:
            # 5. 异常处理，记录错误状态
            self.registry.add_failed_service(agent_id, service_name, service_config, str(e))
            return {
                "success": False,
                "state": "disconnected",
                "tools_added": 0,
                "message": f"Service addition failed: {str(e)}"
            }
    
    def sync_with_lifecycle_manager(self, agent_id: str) -> Dict[str, Any]:
        """
        🔧 [REFACTOR] 与生命周期管理器同步缓存状态 - 现在Registry为唯一状态源

        Returns:
            {
                "synced_services": 0,  # 不再需要同步
                "updated_states": 0,
                "conflicts_resolved": 0
            }
        """
        # 🔧 [REFACTOR] 由于Registry现在是唯一状态源，不再需要同步
        # LifecycleManager直接操作Registry，状态始终一致

        try:
            # 🔧 [REFACTOR] Registry为唯一状态源，无需同步操作
            # 所有状态变更都直接在Registry中进行，保证一致性

            service_count = len(self.registry.get_all_service_names(agent_id))
            logger.debug(f"🔧 [SYNC] Registry contains {service_count} services for agent {agent_id}")

            return {
                "synced_services": 0,  # 不再需要同步
                "updated_states": 0,   # 状态始终一致
                "conflicts_resolved": 0,  # 无冲突
                "message": "Registry is single source of truth - no sync needed"
            }
            
        except Exception as e:
            logger.error(f"Failed to sync with lifecycle manager for agent {agent_id}: {e}")
            return {
                "synced_services": 0,
                "updated_states": 0,
                "conflicts_resolved": 0,
                "error": str(e)
            }
    
    def sync_from_client_manager(self, client_manager):
        """
        从 ClientManager 同步数据到缓存（初始化时覆盖策略）

        新逻辑：初始化时直接覆盖空缓存，默认缓存为空
        """
        try:
            # 检查缓存是否已初始化
            cache_initialized = getattr(self.registry, 'cache_initialized', False)

            if not cache_initialized:
                # 初始化时：直接覆盖空缓存
                logger.info("🔄 [CACHE_INIT] 初始化模式：文件数据覆盖空缓存")

                # 直接覆盖Agent-Client映射
                agent_clients_data = client_manager.load_all_agent_clients()
                logger.info(f"🔧 [CACHE_INIT] 从文件加载的agent_clients数据: {agent_clients_data}")
                self.registry.agent_clients = agent_clients_data.copy()
                logger.info(f"🔧 [CACHE_INIT] 覆盖后的agent_clients缓存: {dict(self.registry.agent_clients)}")

                # 直接覆盖Client配置
                client_services_data = client_manager.load_all_clients()
                logger.info(f"🔧 [CACHE_INIT] 从文件加载的client_configs数据: {len(client_services_data)} clients")
                self.registry.client_configs = client_services_data.copy()
                logger.info(f"🔧 [CACHE_INIT] 覆盖后的client_configs缓存: {len(self.registry.client_configs)} clients")

                # 标记缓存已初始化
                self.registry.cache_initialized = True

            else:
                # 运行时：合并策略（保留现有逻辑作为备用）
                logger.info("🔄 [CACHE_SYNC] 运行时模式：合并文件数据到缓存")

                agent_clients_data = client_manager.load_all_agent_clients()
                for agent_id, client_ids in agent_clients_data.items():
                    if agent_id not in self.registry.agent_clients:
                        self.registry.agent_clients[agent_id] = []
                    for client_id in client_ids:
                        if client_id not in self.registry.agent_clients[agent_id]:
                            self.registry.agent_clients[agent_id].append(client_id)

                client_services_data = client_manager.load_all_clients()
                for client_id, config in client_services_data.items():
                    if client_id not in self.registry.client_configs:
                        self.registry.client_configs[client_id] = config
            
            # 重建 Service-Client 映射
            self.registry.service_to_client = {}
            for agent_id, client_ids in self.registry.agent_clients.items():
                self.registry.service_to_client[agent_id] = {}
                for client_id in client_ids:
                    client_config = self.registry.client_configs.get(client_id, {})
                    for service_name in client_config.get("mcpServers", {}):
                        self.registry.service_to_client[agent_id][service_name] = client_id
            
            # 更新同步时间
            self.registry.cache_sync_status["client_manager"] = datetime.now()
            
            logger.info("Successfully synced cache from ClientManager")
            
        except Exception as e:
            logger.error(f"Failed to sync cache from ClientManager: {e}")
            raise
    
    def sync_to_client_manager(self, client_manager):
        """将缓存数据同步到 ClientManager"""
        try:
            # 同步 Agent-Client 映射
            client_manager.save_all_agent_clients(self.registry.agent_clients)
            
            # 同步 Client 配置
            client_manager.save_all_clients(self.registry.client_configs)
            
            # 更新同步时间
            self.registry.cache_sync_status["to_client_manager"] = datetime.now()
            
            logger.info("Successfully synced cache to ClientManager")
            
        except Exception as e:
            logger.error(f"Failed to sync cache to ClientManager: {e}")
            raise


class CacheTransactionManager:
    """缓存事务管理器 - 支持回滚"""
    
    def __init__(self, registry):
        self.registry = registry
        self.transaction_stack = []
        self.max_transactions = 10  # 最大事务数量
        self.transaction_timeout = 3600  # 事务超时时间（秒）
    
    async def begin_transaction(self, transaction_id: str):
        """开始缓存事务"""
        # 创建当前状态快照
        snapshot = {
            "transaction_id": transaction_id,
            "timestamp": datetime.now(),
            "agent_clients": copy.deepcopy(self.registry.agent_clients),
            "client_configs": copy.deepcopy(self.registry.client_configs),
            "service_to_client": copy.deepcopy(self.registry.service_to_client),
            "service_states": copy.deepcopy(self.registry.service_states),
            "service_metadata": copy.deepcopy(self.registry.service_metadata),
            "sessions": copy.deepcopy(self.registry.sessions),
            "tool_cache": copy.deepcopy(self.registry.tool_cache)
        }
        
        self.transaction_stack.append(snapshot)

        # 清理过期和过多的事务
        self._cleanup_transactions()

        logger.debug(f"Started cache transaction: {transaction_id}")
    
    async def commit_transaction(self, transaction_id: str):
        """提交缓存事务"""
        # 移除对应的快照
        self.transaction_stack = [
            snap for snap in self.transaction_stack 
            if snap["transaction_id"] != transaction_id
        ]
        logger.debug(f"Committed cache transaction: {transaction_id}")
    
    async def rollback_transaction(self, transaction_id: str):
        """回滚缓存事务"""
        # 找到对应的快照
        snapshot = None
        for snap in self.transaction_stack:
            if snap["transaction_id"] == transaction_id:
                snapshot = snap
                break
        
        if not snapshot:
            logger.error(f"Transaction snapshot not found: {transaction_id}")
            return False
        
        try:
            # 恢复缓存状态
            self.registry.agent_clients = snapshot["agent_clients"]
            self.registry.client_configs = snapshot["client_configs"]
            self.registry.service_to_client = snapshot["service_to_client"]
            self.registry.service_states = snapshot["service_states"]
            self.registry.service_metadata = snapshot["service_metadata"]
            self.registry.sessions = snapshot["sessions"]
            self.registry.tool_cache = snapshot["tool_cache"]
            
            # 移除快照
            self.transaction_stack = [
                snap for snap in self.transaction_stack 
                if snap["transaction_id"] != transaction_id
            ]
            
            logger.info(f"Rolled back cache transaction: {transaction_id}")
            return True
            
        except Exception as e:
            logger.error(f"Failed to rollback transaction {transaction_id}: {e}")
            return False

    def _cleanup_transactions(self):
        """清理过期和过多的事务"""
        current_time = datetime.now()

        # 清理过期事务
        self.transaction_stack = [
            snap for snap in self.transaction_stack
            if (current_time - snap["timestamp"]).total_seconds() < self.transaction_timeout
        ]

        # 限制事务数量（保留最新的）
        if len(self.transaction_stack) > self.max_transactions:
            self.transaction_stack = self.transaction_stack[-self.max_transactions:]
            logger.warning(f"Transaction stack exceeded limit, kept latest {self.max_transactions} transactions")

    def get_transaction_count(self) -> int:
        """获取当前事务数量"""
        return len(self.transaction_stack)

    def clear_all_transactions(self):
        """清理所有事务（慎用）"""
        count = len(self.transaction_stack)
        self.transaction_stack.clear()
        logger.warning(f"Cleared all {count} transactions from stack")
