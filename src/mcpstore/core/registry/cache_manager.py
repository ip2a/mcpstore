import copy
import logging
from datetime import datetime
from typing import Dict, Any

from mcpstore.core.models.service import ServiceConnectionState

logger = logging.getLogger(__name__)


class ServiceCacheManager:
    """
    Service cache manager - provides advanced cache operations
    """
    
    def __init__(self, registry, lifecycle_manager):
        self.registry = registry
        self.lifecycle_manager = lifecycle_manager
    
    # === Intelligent cache operations ===
    
    async def smart_add_service(self, agent_id: str, service_name: str, service_config: Dict[str, Any]) -> Dict[str, Any]:
        """
        Smart add service: automatically handles connection, state management, cache updates
        
        Returns:
            {
                "success": True,
                "state": "healthy",
                "tools_added": 5,
                "message": "Service added successfully"
            }
        """
        try:
            # 1. Initialize to lifecycle manager
            await self.lifecycle_manager.initialize_service(agent_id, service_name, service_config)
            
            # 2. Immediately add to cache (initializing state)
            self.registry.add_service(
                agent_id=agent_id,
                name=service_name,
                session=None,
                tools=[],
                service_config=service_config,
                state=ServiceConnectionState.STARTUP
            )
            
            return {
                "success": True,
                "state": "startup",
                "tools_added": 0,
                "message": "Service added to cache, connecting in background"
            }
                
        except Exception as e:
            # 5. Exception handling, record error status
            self.registry.add_failed_service(agent_id, service_name, service_config, str(e))
            return {
                "success": False,
                "state": "disconnected",
                "tools_added": 0,
                "message": f"Service addition failed: {str(e)}"
            }

    def sync_from_client_manager(self, client_manager):
        """
        Single data source architecture: ClientManager no longer manages shard files

        Under the new architecture, cache is not synchronized from ClientManager,
        but from mcp.json through UnifiedMCPSyncManager
        """
        try:
            # Check if cache has been initialized
            cache_initialized = getattr(self.registry, 'cache_initialized', False)

            if not cache_initialized:
                # Single data source mode: initialize empty cache, wait for synchronization from mcp.json
                logger.info(" [CACHE_INIT] Single data source mode: initializing empty cache, waiting for synchronization from mcp.json")

                # Initialize empty cache
                # agent_clients removed - now derived from service_client mappings in pyvk
                # client_configs removed - now in pyvk only
                logger.info(" [CACHE_INIT] Empty cache initialization completed")

                # Mark cache as initialized
                self.registry.cache_initialized = True

            else:
                # Runtime: single data source mode does not need ClientManager synchronization
                logger.info(" [CACHE_SYNC] Single data source mode: skipping ClientManager synchronization at runtime")
                logger.info(" [CACHE_SYNC] Cache data is synchronized from mcp.json by UnifiedMCPSyncManager")

            # Update synchronization time (record operation)
            from datetime import datetime
            self.registry.cache_sync_status["client_manager"] = datetime.now()
            self.registry.cache_sync_status["sync_mode"] = "single_source_mode"

            logger.info(" [CACHE_INIT] ClientManager synchronization completed (single data source mode)")
            
        except Exception as e:
            logger.error(f"Failed to sync cache from ClientManager: {e}")
            raise
    
    def sync_to_client_manager(self, client_manager):
        """
        Single data source architecture: no longer synchronize to ClientManager

        Under the new architecture, cache data is only synchronized to mcp.json,
        shard files are no longer maintained
        """
        try:
            # Single data source mode: skip ClientManager synchronization
            logger.info(" [CACHE_SYNC] Single data source mode: skipping ClientManager synchronization, only maintaining mcp.json")

            # Update synchronization time (record skipped operation)
            from datetime import datetime
            self.registry.cache_sync_status["to_client_manager"] = datetime.now()
            self.registry.cache_sync_status["sync_skipped"] = "single_source_mode"

        except Exception as e:
            logger.error(f"Failed to update sync status: {e}")
            raise


class CacheTransactionManager:
    """Cache transaction manager - supports rollback"""
    
    def __init__(self, registry):
        self.registry = registry
        self.transaction_stack = []
        self.max_transactions = 10  # Maximum number of transactions
        self.transaction_timeout = 3600  # Transaction timeout time (seconds)
    
    async def begin_transaction(self, transaction_id: str):
        """Begin cache transaction

        Note: tool_cache, service_to_client, client_configs removed - now stored in pyvk only.
        Transaction snapshots only cover in-memory runtime data.
        """
        # Create current state snapshot (only in-memory runtime data)
        snapshot = {
            "transaction_id": transaction_id,
            "timestamp": datetime.now(),
            # agent_clients removed - now derived from service_client mappings in pyvk
            # client_configs removed - now in pyvk only
            # service_to_client removed - now in pyvk only
            "service_states": copy.deepcopy(self.registry.service_states),
            "service_metadata": copy.deepcopy(self.registry.service_metadata),
            "sessions": copy.deepcopy(self.registry.sessions)
        }

        self.transaction_stack.append(snapshot)

        # Clean up expired and excessive transactions
        self._cleanup_transactions()

        logger.debug(f"Started cache transaction: {transaction_id}")
    
    async def commit_transaction(self, transaction_id: str):
        """Commit cache transaction"""
        # Remove corresponding snapshot
        self.transaction_stack = [
            snap for snap in self.transaction_stack 
            if snap["transaction_id"] != transaction_id
        ]
        logger.debug(f"Committed cache transaction: {transaction_id}")
    
    async def rollback_transaction(self, transaction_id: str):
        """Rollback cache transaction"""
        # Find corresponding snapshot
        snapshot = None
        for snap in self.transaction_stack:
            if snap["transaction_id"] == transaction_id:
                snapshot = snap
                break
        
        if not snapshot:
            logger.error(f"Transaction snapshot not found: {transaction_id}")
            return False
        
        try:
            # Restore cache state (only in-memory runtime data)
            # agent_clients removed - now derived from service_client mappings in pyvk
            # client_configs removed - now in pyvk only
            # service_to_client removed - now in pyvk only
            self.registry.service_states = snapshot["service_states"]
            self.registry.service_metadata = snapshot["service_metadata"]
            self.registry.sessions = snapshot["sessions"]

            # Remove snapshot
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
        """Clean up expired and excessive transactions"""
        current_time = datetime.now()

        # Clean up expired transactions
        self.transaction_stack = [
            snap for snap in self.transaction_stack
            if (current_time - snap["timestamp"]).total_seconds() < self.transaction_timeout
        ]

        # Limit transaction count (keep latest)
        if len(self.transaction_stack) > self.max_transactions:
            self.transaction_stack = self.transaction_stack[-self.max_transactions:]
            logger.warning(f"Transaction stack exceeded limit, kept latest {self.max_transactions} transactions")

    def get_transaction_count(self) -> int:
        """Get current transaction count"""
        return len(self.transaction_stack)

    def clear_all_transactions(self):
        """Clear all transactions (use with caution)"""
        count = len(self.transaction_stack)
        self.transaction_stack.clear()
        logger.warning(f"Cleared all {count} transactions from stack")
