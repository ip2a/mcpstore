"""
Unified lock abstractions for MCPStore
Provides consistent locking interface for both async and sync contexts
"""

import asyncio
import threading
import logging
from abc import ABC, abstractmethod
from typing import Dict, AsyncContextManager, ContextManager
from contextlib import asynccontextmanager, contextmanager

logger = logging.getLogger(__name__)


class LockProvider(ABC):
    """Abstract lock provider interface"""
    
    @abstractmethod
    async def acquire_async(self, key: str) -> AsyncContextManager:
        """Acquire lock asynchronously
        
        Args:
            key: Lock identifier (e.g., agent_id)
            
        Returns:
            Async context manager for the lock
        """
        pass
    
    @abstractmethod
    def acquire_sync(self, key: str) -> ContextManager:
        """Acquire lock synchronously
        
        Args:
            key: Lock identifier (e.g., agent_id)
            
        Returns:
            Context manager for the lock
        """
        pass


class AsyncLockProvider(LockProvider):
    """Async lock provider using asyncio.Lock
    
    Suitable for async contexts. Uses striped locking to reduce
    contention on the global lock used for lazy lock creation.
    """
    
    def __init__(self, num_stripes: int = 16):
        """Initialize async lock provider
        
        Args:
            num_stripes: Number of lock stripes for reducing contention
        """
        self._locks: Dict[str, asyncio.Lock] = {}
        self._num_stripes = num_stripes
        self._stripe_locks = [asyncio.Lock() for _ in range(num_stripes)]
        logger.debug(f"AsyncLockProvider initialized with {num_stripes} stripes")
    
    def _get_stripe_index(self, key: str) -> int:
        """Get stripe index for a key using hash-based distribution"""
        return hash(key) % self._num_stripes
    
    @asynccontextmanager
    async def acquire_async(self, key: str):
        """Acquire lock asynchronously with striped locking
        
        Uses double-checked locking pattern:
        1. Check if lock exists (no lock needed)
        2. If not, acquire stripe lock
        3. Check again and create if needed
        4. Acquire the actual lock
        """
        # Fast path: lock already exists
        if key in self._locks:
            async with self._locks[key]:
                yield
                return
        
        # Slow path: need to create lock
        stripe_idx = self._get_stripe_index(key)
        async with self._stripe_locks[stripe_idx]:
            # Double-check after acquiring stripe lock
            if key not in self._locks:
                self._locks[key] = asyncio.Lock()
                logger.debug(f"Created new async lock for key: {key}")
        
        # Now acquire the actual lock
        async with self._locks[key]:
            yield
    
    def acquire_sync(self, key: str) -> ContextManager:
        """Not supported in async provider
        
        Raises:
            NotImplementedError: Sync locks not supported
        """
        raise NotImplementedError(
            "Sync locks not supported in AsyncLockProvider. "
            "Use SyncLockProvider for synchronous contexts."
        )


class SyncLockProvider(LockProvider):
    """Sync lock provider using threading.Lock
    
    Suitable for sync contexts. Uses striped locking to reduce
    contention on the global lock used for lazy lock creation.
    """
    
    def __init__(self, num_stripes: int = 16):
        """Initialize sync lock provider
        
        Args:
            num_stripes: Number of lock stripes for reducing contention
        """
        self._locks: Dict[str, threading.Lock] = {}
        self._num_stripes = num_stripes
        self._stripe_locks = [threading.Lock() for _ in range(num_stripes)]
        logger.debug(f"SyncLockProvider initialized with {num_stripes} stripes")
    
    def _get_stripe_index(self, key: str) -> int:
        """Get stripe index for a key using hash-based distribution"""
        return hash(key) % self._num_stripes
    
    @contextmanager
    def acquire_sync(self, key: str):
        """Acquire lock synchronously with striped locking
        
        Uses double-checked locking pattern:
        1. Check if lock exists (no lock needed)
        2. If not, acquire stripe lock
        3. Check again and create if needed
        4. Acquire the actual lock
        """
        # Fast path: lock already exists
        if key in self._locks:
            with self._locks[key]:
                yield
                return
        
        # Slow path: need to create lock
        stripe_idx = self._get_stripe_index(key)
        with self._stripe_locks[stripe_idx]:
            # Double-check after acquiring stripe lock
            if key not in self._locks:
                self._locks[key] = threading.Lock()
                logger.debug(f"Created new sync lock for key: {key}")
        
        # Now acquire the actual lock
        with self._locks[key]:
            yield
    
    async def acquire_async(self, key: str) -> AsyncContextManager:
        """Not supported in sync provider
        
        Raises:
            NotImplementedError: Async locks not supported
        """
        raise NotImplementedError(
            "Async locks not supported in SyncLockProvider. "
            "Use AsyncLockProvider for asynchronous contexts."
        )


# Convenience function for creating the appropriate provider
def create_lock_provider(async_mode: bool = True, num_stripes: int = 16) -> LockProvider:
    """Create a lock provider based on context
    
    Args:
        async_mode: If True, create AsyncLockProvider; otherwise SyncLockProvider
        num_stripes: Number of lock stripes for reducing contention
        
    Returns:
        LockProvider instance
    """
    if async_mode:
        return AsyncLockProvider(num_stripes=num_stripes)
    else:
        return SyncLockProvider(num_stripes=num_stripes)

