import copy
import logging
from datetime import datetime
from typing import Dict, Any

from mcpstore.core.models.service import ServiceConnectionState

logger = logging.getLogger(__name__)


class ServiceCacheManager:
    """
    服务缓存管理器 - 提供高级缓存操作
    """
    
    def __init__(self, registry, lifecycle_manager):
        self.registry = registry
        self.lifecycle_manager = lifecycle_manager
    
    # ===  智能缓存操作 ===
    
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
    

    def sync_from_client_manager(self, client_manager):
        """
         单一数据源架构：ClientManager不再管理分片文件
        
        新架构下，缓存不从ClientManager同步，而是从mcp.json通过UnifiedMCPSyncManager同步
        """
        try:
            # 检查缓存是否已初始化
            cache_initialized = getattr(self.registry, 'cache_initialized', False)

            if not cache_initialized:
                # 单一数据源模式：缓存初始化为空，等待从mcp.json同步
                logger.info(" [CACHE_INIT] 单一数据源模式：初始化空缓存，等待从mcp.json同步")

                # 初始化为空缓存
                self.registry.agent_clients = {}
                self.registry.client_configs = {}
                logger.info(" [CACHE_INIT] 空缓存初始化完成")

                # 标记缓存已初始化
                self.registry.cache_initialized = True

            else:
                # 运行时：单一数据源模式下无需从ClientManager同步
                logger.info(" [CACHE_SYNC] 单一数据源模式：运行时跳过ClientManager同步")
                logger.info("ℹ️ [CACHE_SYNC] 缓存数据由UnifiedMCPSyncManager从mcp.json同步")
            
            # 更新同步时间（记录操作）
            from datetime import datetime
            self.registry.cache_sync_status["client_manager"] = datetime.now()
            self.registry.cache_sync_status["sync_mode"] = "single_source_mode"
            
            logger.info(" [CACHE_INIT] ClientManager同步完成（单一数据源模式）")
            
        except Exception as e:
            logger.error(f"Failed to sync cache from ClientManager: {e}")
            raise
    
    def sync_to_client_manager(self, client_manager):
        """
         单一数据源架构：不再同步到ClientManager
        
        新架构下，缓存数据只同步到mcp.json，不再维护分片文件
        """
        try:
            # 单一数据源模式：跳过ClientManager同步
            logger.info(" [CACHE_SYNC] 单一数据源模式：跳过ClientManager同步，仅维护mcp.json")
            
            # 更新同步时间（记录跳过的操作）
            from datetime import datetime
            self.registry.cache_sync_status["to_client_manager"] = datetime.now()
            self.registry.cache_sync_status["sync_skipped"] = "single_source_mode"
            
        except Exception as e:
            logger.error(f"Failed to update sync status: {e}")
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
