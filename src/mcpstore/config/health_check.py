"""
Health check functionality for Redis connections.

This module provides background health check tasks for monitoring Redis
connection health without blocking main operations.
"""

import asyncio
import logging
from typing import Optional, Any

from redis.asyncio import Redis

from mcpstore.core.bridge import get_async_bridge
from .cache_config import RedisConfig

logger = logging.getLogger(__name__)


class RedisHealthCheck:
    """
    Background health check task for Redis connections.
    
    This class manages a background task that periodically pings Redis
    to verify connection health. It logs warnings on failure but does
    not block main operations.
    
    Attributes:
        config: Redis configuration object
        client: Redis client instance
        task: Background asyncio task (if running)
        _stop_event: Event to signal task shutdown
    """
    
    def __init__(self, config: RedisConfig, client: Redis):
        """
        Initialize health check.
        
        Args:
            config: Redis configuration with health_check_interval
            client: Redis client to monitor
        """
        self.config = config
        self.client = client
        self.task: Optional[Any] = None
        self._stop_event = asyncio.Event()
        self._bridge_handle = None
        self._bridge = get_async_bridge()
    
    async def _health_check_loop(self):
        """
        Background loop that periodically pings Redis.
        
        This method runs in a background task and executes PING commands
        at the configured interval. Failures are logged as warnings but
        do not raise exceptions or block operations.
        """
        interval = self.config.health_check_interval
        
        logger.debug(
            f"Starting Redis health check with interval: {interval}s"
        )
        
        while not self._stop_event.is_set():
            try:
                # Wait for the interval or until stop is signaled
                await asyncio.wait_for(
                    self._stop_event.wait(),
                    timeout=interval
                )
                # If we get here, stop was signaled
                break
            except asyncio.TimeoutError:
                # Timeout is expected - time to do health check
                pass
            
            # Perform health check
            try:
                await self.client.ping()
                logger.debug("Redis health check: OK")
            except Exception as e:
                # Log health check failure with context
                logger.warning(
                    f"Redis health check failed: {type(e).__name__}: {e}. "
                    f"Connection may be unstable. "
                    f"Namespace: {self.config.namespace or 'default'}, "
                    f"Interval: {self.config.health_check_interval}s"
                )
    
    def start(self):
        """
        Start the health check background task.
        
        This method starts the background task if:
        - health_check_interval > 0
        - Task is not already running
        
        The task runs independently and does not block the caller.
        
        Note: This method can be called from both sync and async contexts.
        It will automatically detect the context and use the appropriate
        event loop.
        """
        # Only start if interval is positive
        if self.config.health_check_interval <= 0:
            logger.debug("Health check disabled (interval <= 0)")
            return
        
        # Don't start if already running
        if self.task is not None and not self.task.done():
            logger.debug("Health check already running")
            return
        
        # Try to get the running event loop
        try:
            asyncio.get_running_loop()
            self.task = asyncio.create_task(self._health_check_loop())
            logger.info(
                f"Started Redis health check (interval: {self.config.health_check_interval}s)"
            )
        except RuntimeError:
            try:
                self._bridge_handle = self._bridge.create_background_task(
                    self._health_check_loop(),
                    op_name="redis.health_check"
                )
                self.task = self._bridge_handle
                logger.info(
                    f"Started Redis health check in background bridge "
                    f"(interval: {self.config.health_check_interval}s)"
                )
            except Exception as e:
                logger.warning(
                    f"Failed to start health check: {e}. "
                    f"Health monitoring will be disabled."
                )
    
    async def stop(self):
        """
        Stop the health check background task.
        
        This method signals the background task to stop and waits for it
        to complete gracefully.
        
        Note: This is an async method and should be called with await.
        For sync contexts, use stop_sync() instead.
        """
        if self.task is None:
            return

        # bridge handle case
        if self._bridge_handle is not None:
            self._stop_event.set()
            try:
                self._bridge_handle.cancel()
            finally:
                self._bridge_handle = None
                self.task = None
                self._stop_event = asyncio.Event()
            return

        # Check if task is done (works for both Task and Future)
        try:
            if self.task.done():
                return
        except AttributeError:
            # If done() doesn't exist, assume it's not done
            pass
        
        # Signal the task to stop
        self._stop_event.set()
        
        # Wait for the task to complete
        try:
            # Handle both asyncio.Task and concurrent.futures.Future
            if hasattr(self.task, '__await__'):
                # It's an asyncio Task
                await asyncio.wait_for(self.task, timeout=5.0)
            else:
                # It's a concurrent.futures.Future from run_coroutine_threadsafe
                # We can't await it directly, just wait for completion
                import concurrent.futures
                try:
                    self.task.result(timeout=5.0)
                except concurrent.futures.TimeoutError:
                    logger.warning("Health check task did not stop gracefully")
                    self.task.cancel()
                except Exception as e:
                    # Task may have raised an exception, that's ok during shutdown
                    logger.debug(f"Health check task ended with: {e}")
            
            logger.debug("Health check stopped")
        except asyncio.TimeoutError:
            logger.warning("Health check task did not stop gracefully")
            if hasattr(self.task, 'cancel'):
                self.task.cancel()
                try:
                    await self.task
                except asyncio.CancelledError:
                    pass
        finally:
            self.task = None
            self._bridge_handle = None
            self._stop_event = asyncio.Event()


def start_health_check(
    config: RedisConfig,
    client: Redis
) -> Optional[RedisHealthCheck]:
    """
    Start a health check background task for Redis connection.
    
    This is a convenience function that creates and starts a health check
    task based on the configuration. If health_check_interval is 0 or
    negative, no task is created.
    
    Args:
        config: Redis configuration object
        client: Redis client to monitor
    
    Returns:
        RedisHealthCheck instance if started, None if disabled
    
    Examples:
        >>> config = RedisConfig(
        ...     url="redis://localhost:6379/0",
        ...     health_check_interval=30
        ... )
        >>> client = Redis.from_url(config.url)
        >>> health_check = start_health_check(config, client)
        >>> # Later, when shutting down:
        >>> if health_check:
        ...     await health_check.stop()
    """
    # Check if health check is enabled
    if config.health_check_interval <= 0:
        logger.debug("Health check disabled in configuration")
        return None
    
    # Create and start health check
    health_check = RedisHealthCheck(config, client)
    health_check.start()
    
    return health_check
