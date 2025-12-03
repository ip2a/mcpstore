"""
KV Storage Adapter for ServiceRegistry

This module provides a clean abstraction layer for py-key-value storage operations,
handling synchronization, collection naming, and value wrapping/unwrapping.

Extracted from core_registry.py to reduce God Object complexity.
"""

import logging
from typing import Any, Dict, Optional, TYPE_CHECKING

if TYPE_CHECKING:
    from key_value.aio.protocols import AsyncKeyValue

logger = logging.getLogger(__name__)


class KVStorageAdapter:
    """
    Adapter for py-key-value storage operations.
    
    Provides:
    - Sync-to-async conversion for KV operations
    - Collection name generation strategy
    - Value wrapping/unwrapping for scalar values
    - Error handling and logging
    """
    
    def __init__(self, kv_store: 'AsyncKeyValue'):
        """
        Initialize KV storage adapter.
        
        Args:
            kv_store: AsyncKeyValue instance for data storage
        """
        self._kv_store = kv_store
        self._sync_helper: Optional[Any] = None  # Lazy initialization
    
    def _ensure_sync_helper(self):
        """Ensure AsyncSyncHelper is initialized (lazy initialization)."""
        if self._sync_helper is None:
            from mcpstore.core.utils.async_sync_helper import AsyncSyncHelper
            self._sync_helper = AsyncSyncHelper()
            logger.debug("AsyncSyncHelper initialized for KVStorageAdapter")
        return self._sync_helper
    
    def sync_to_kv(self, coro, operation_name: str = "KV operation"):
        """
        Synchronously execute an async KV store operation.
        
        This is a bridge method for synchronous code that must perform KV
        operations. It uses AsyncSyncHelper to run async operations in a
        synchronous context.
        
        Args:
            coro: Coroutine to execute
            operation_name: Description of the operation for logging
        
        """
        try:
            logger.debug(f"[KV_SYNC] Starting sync: {operation_name}")
            helper = self._ensure_sync_helper()
            helper.run_async(coro, timeout=5.0)
            logger.debug(f"[KV_SYNC] Successfully synced: {operation_name}")
        except Exception as e:
            # Treat KV sync failures as hard errors so they are not hidden
            logger.error(
                f"[KV_SYNC] Failed to sync to KV store: {operation_name}. Error: {e}",
                exc_info=True,
            )
            raise
    
    def get_collection(self, agent_id: str, data_type: str) -> str:
        """
        Generate Collection name for py-key-value storage.
        
        This method implements the Collection mapping strategy for organizing data
        in the KV store with agent_id isolation.
        
        Args:
            agent_id: Agent identifier (e.g., "global_agent_store", "agent_123")
            data_type: Type of data being stored (e.g., "tools", "states", "metadata")
        
        Returns:
            Collection name in format "agent:{agent_id}:{data_type}"
        
        Examples:
            >>> adapter.get_collection("global_agent_store", "tools")
            "agent:global_agent_store:tools"
            
            >>> adapter.get_collection("agent_123", "states")
            "agent:agent_123:states"
        
        Note:
            This naming strategy ensures:
            - Clear agent_id isolation
            - Logical grouping by data type
            - Easy querying and debugging
            - Consistent with py-key-value Collection semantics
        """
        return f"agent:{agent_id}:{data_type}"
    
    def wrap_scalar_value(self, value: Any) -> Dict[str, Any]:
        """
        Wrap a scalar value in a dictionary for py-key-value storage.
        
        py-key-value expects dictionary values for storage. This method wraps
        scalar values (strings, numbers, booleans, None) in a standard format.
        
        Args:
            value: Value to wrap (can be scalar or already a dict)
        
        Returns:
            Dictionary with "value" key containing the original value
        
        Examples:
            >>> adapter.wrap_scalar_value("healthy")
            {"value": "healthy"}
            
            >>> adapter.wrap_scalar_value(42)
            {"value": 42}
            
            >>> adapter.wrap_scalar_value({"already": "dict"})
            {"already": "dict"}  # Already a dict, returned as-is
        
        Note:
            - If value is already a dict, returns it unchanged
            - This allows mixed storage of scalar and complex values
            - Unwrap with unwrap_scalar_value() when reading
        """
        # If already a dict, return as-is (assume it's properly formatted)
        if isinstance(value, dict):
            return value
        
        return {"value": value}
    
    def unwrap_scalar_value(self, wrapped: Any) -> Any:
        """
        Unwrap a scalar value from dictionary storage format.
        
        Reverses the wrapping done by wrap_scalar_value(). Handles both
        wrapped scalar values and complex dictionary values.
        
        Args:
            wrapped: Value from KV storage (may be wrapped or complex dict)
        
        Returns:
            Original unwrapped value
        
        Examples:
            >>> adapter.unwrap_scalar_value({"value": "healthy"})
            "healthy"
            
            >>> adapter.unwrap_scalar_value({"value": 42})
            42
            
            >>> adapter.unwrap_scalar_value({"complex": "dict", "with": "data"})
            {"complex": "dict", "with": "data"}  # Not wrapped, returned as-is
        
        Note:
            - If wrapped format detected (dict with single "value" key), unwraps it
            - Otherwise returns the value unchanged
            - Safe to call on any value from KV storage
        """
        # If it's a wrapped scalar (dict with single "value" key), unwrap it
        if self.is_wrapped_value(wrapped):
            return wrapped["value"]
        
        # Otherwise return as-is (complex dict or other type)
        return wrapped
    
    def is_wrapped_value(self, value: Any) -> bool:
        """
        Check if a value is in wrapped format.
        
        Determines if a value was wrapped by wrap_scalar_value() and needs
        to be unwrapped when reading from storage.
        
        Args:
            value: Value to check
        
        Returns:
            True if value is a wrapped scalar, False otherwise
        
        Examples:
            >>> adapter.is_wrapped_value({"value": "healthy"})
            True
            
            >>> adapter.is_wrapped_value({"value": 42})
            True
            
            >>> adapter.is_wrapped_value({"complex": "dict"})
            False
            
            >>> adapter.is_wrapped_value("not a dict")
            False
        
        Note:
            A value is considered wrapped if it's a dict with exactly one key "value"
        """
        return isinstance(value, dict) and "value" in value
    
    @property
    def kv_store(self) -> 'AsyncKeyValue':
        """Get the underlying KV store instance."""
        return self._kv_store
