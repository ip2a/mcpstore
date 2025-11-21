"""
Redis health checking and connection validation.

This module provides utilities for validating Redis connections and
implementing health checks with automatic reconnection.

Validates:
    - Requirements 18.3: Fault handling configuration
    - Requirements 18.4: Fail-Fast error handling
"""

from __future__ import annotations

import asyncio
import logging
import time
from typing import TYPE_CHECKING, Optional

if TYPE_CHECKING:
    from key_value.aio.stores.redis import RedisStore

logger = logging.getLogger(__name__)


class RedisConnectionError(Exception):
    """
    Exception raised when Redis connection fails.
    
    This exception is raised during fail-fast validation to provide
    clear error messages about connection failures.
    
    Validates:
        - Requirements 18.4: Fail-Fast error handling
    """
    pass


async def validate_redis_connection(
    store: 'RedisStore',
    timeout: float = 5.0
) -> None:
    """
    Validate Redis connection with fail-fast behavior.
    
    This function attempts to ping the Redis server to ensure the connection
    is working. If the connection fails, it raises RedisConnectionError with
    a clear error message.
    
    Args:
        store: RedisStore instance to validate
        timeout: Timeout in seconds for the ping operation
    
    Raises:
        RedisConnectionError: If connection validation fails
        asyncio.TimeoutError: If ping operation times out
    
    Examples:
        >>> store = RedisStore(url="redis://localhost:6379/0")
        >>> await validate_redis_connection(store)
        # Raises RedisConnectionError if connection fails
    
    Validates:
        - Requirements 18.4: Throw exception immediately when Redis connection fails
    """
    try:
        # Attempt to ping Redis with timeout
        await asyncio.wait_for(
            _ping_redis(store),
            timeout=timeout
        )
        logger.info("Redis connection validated successfully")
        
    except asyncio.TimeoutError as e:
        error_msg = (
            f"Redis connection validation timed out after {timeout}s. "
            f"Redis server may be unresponsive or network is slow."
        )
        logger.error(error_msg)
        raise RedisConnectionError(error_msg) from e
        
    except Exception as e:
        error_msg = (
            f"Redis connection validation failed: {e}. "
            f"Please verify Redis server is running and accessible."
        )
        logger.error(error_msg)
        raise RedisConnectionError(error_msg) from e


async def _ping_redis(store: 'RedisStore') -> None:
    """
    Internal function to ping Redis.
    
    This attempts a simple operation to verify the connection works.
    
    Args:
        store: RedisStore instance
    
    Raises:
        Exception: If ping fails
    """
    # Try a simple operation to validate connection
    # We'll try to get a non-existent key, which should return None
    test_key = f"__mcpstore_health_check_{time.time()}"
    try:
        await store.get(test_key)
        logger.debug("Redis ping successful")
    except Exception as e:
        logger.error(f"Redis ping failed: {e}")
        raise


class RedisHealthChecker:
    """
    Health checker for Redis connections with automatic reconnection.
    
    This class provides periodic health checks and automatic reconnection
    with exponential backoff when Redis connection fails.
    
    Attributes:
        store: RedisStore instance to monitor
        check_interval: Interval between health checks in seconds
        max_retries: Maximum number of reconnection attempts
        backoff_factor: Exponential backoff factor for retries
    
    Validates:
        - Requirements 18.3: Periodic health checks
        - Requirements 18.3: Automatic reconnection (with backoff strategy)
    """
    
    def __init__(
        self,
        store: 'RedisStore',
        check_interval: float = 30.0,
        max_retries: int = 5,
        backoff_factor: float = 2.0
    ):
        """
        Initialize health checker.
        
        Args:
            store: RedisStore instance to monitor
            check_interval: Interval between health checks in seconds
            max_retries: Maximum number of reconnection attempts
            backoff_factor: Exponential backoff factor for retries
        """
        self.store = store
        self.check_interval = check_interval
        self.max_retries = max_retries
        self.backoff_factor = backoff_factor
        
        self._is_healthy = True
        self._consecutive_failures = 0
        self._last_check_time = 0.0
        self._health_check_task: Optional[asyncio.Task] = None
    
    async def start(self) -> None:
        """
        Start periodic health checks.
        
        This starts a background task that periodically checks Redis health
        and attempts reconnection if needed.
        
        Validates:
            - Requirements 18.3: Periodic health checks
        """
        if self._health_check_task is not None:
            logger.warning("Health checker already started")
            return
        
        self._health_check_task = asyncio.create_task(self._health_check_loop())
        logger.info(f"Started Redis health checker: interval={self.check_interval}s")
    
    async def stop(self) -> None:
        """
        Stop periodic health checks.
        """
        if self._health_check_task is None:
            return
        
        self._health_check_task.cancel()
        try:
            await self._health_check_task
        except asyncio.CancelledError:
            pass
        
        self._health_check_task = None
        logger.info("Stopped Redis health checker")
    
    async def _health_check_loop(self) -> None:
        """
        Main health check loop.
        
        This runs continuously, checking Redis health at regular intervals.
        """
        while True:
            try:
                await asyncio.sleep(self.check_interval)
                await self._perform_health_check()
            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"Health check loop error: {e}")
    
    async def _perform_health_check(self) -> None:
        """
        Perform a single health check.
        
        This pings Redis and updates health status. If the check fails,
        it attempts reconnection with exponential backoff.
        
        Validates:
            - Requirements 18.3: Automatic reconnection (with backoff strategy)
        """
        self._last_check_time = time.time()
        
        try:
            # Attempt to ping Redis
            await _ping_redis(self.store)
            
            # Success: reset failure counter
            if not self._is_healthy:
                logger.info("Redis connection recovered")
            
            self._is_healthy = True
            self._consecutive_failures = 0
            
        except Exception as e:
            # Failure: increment counter and attempt reconnection
            self._consecutive_failures += 1
            self._is_healthy = False
            
            logger.warning(
                f"Redis health check failed (attempt {self._consecutive_failures}/{self.max_retries}): {e}"
            )
            
            # Attempt reconnection with exponential backoff
            if self._consecutive_failures <= self.max_retries:
                await self._attempt_reconnection()
            else:
                logger.error(
                    f"Redis connection failed after {self.max_retries} attempts. "
                    f"Giving up on automatic reconnection."
                )
    
    async def _attempt_reconnection(self) -> None:
        """
        Attempt to reconnect to Redis with exponential backoff.
        
        Validates:
            - Requirements 18.3: Automatic reconnection (with backoff strategy)
        """
        # Calculate backoff delay
        delay = min(
            self.check_interval * (self.backoff_factor ** (self._consecutive_failures - 1)),
            300.0  # Max 5 minutes
        )
        
        logger.info(f"Attempting Redis reconnection in {delay:.1f}s...")
        await asyncio.sleep(delay)
        
        try:
            # Try to ping Redis
            await _ping_redis(self.store)
            logger.info("Redis reconnection successful")
            self._is_healthy = True
            self._consecutive_failures = 0
            
        except Exception as e:
            logger.warning(f"Redis reconnection failed: {e}")
    
    @property
    def is_healthy(self) -> bool:
        """
        Check if Redis connection is currently healthy.
        
        Returns:
            True if healthy, False otherwise
        """
        return self._is_healthy
    
    @property
    def consecutive_failures(self) -> int:
        """
        Get the number of consecutive health check failures.
        
        Returns:
            Number of consecutive failures
        """
        return self._consecutive_failures


class RedisCircuitBreaker:
    """
    Circuit breaker for Redis operations.
    
    This implements a circuit breaker pattern to prevent cascading failures
    when Redis is unavailable. The circuit breaker has three states:
    - CLOSED: Normal operation, requests pass through
    - OPEN: Redis is failing, requests are rejected immediately
    - HALF_OPEN: Testing if Redis has recovered
    
    Validates:
        - Requirements 18.3: Circuit breaker mechanism
    """
    
    # Circuit breaker states
    STATE_CLOSED = "CLOSED"
    STATE_OPEN = "OPEN"
    STATE_HALF_OPEN = "HALF_OPEN"
    
    def __init__(
        self,
        failure_threshold: int = 5,
        recovery_timeout: float = 60.0,
        half_open_max_calls: int = 3
    ):
        """
        Initialize circuit breaker.
        
        Args:
            failure_threshold: Number of failures before opening circuit
            recovery_timeout: Time to wait before attempting recovery (seconds)
            half_open_max_calls: Max calls to allow in half-open state
        """
        self.failure_threshold = failure_threshold
        self.recovery_timeout = recovery_timeout
        self.half_open_max_calls = half_open_max_calls
        
        self._state = self.STATE_CLOSED
        self._failure_count = 0
        self._last_failure_time = 0.0
        self._half_open_calls = 0
    
    async def call(self, func, *args, **kwargs):
        """
        Execute a function through the circuit breaker.
        
        Args:
            func: Async function to execute
            *args: Positional arguments for func
            **kwargs: Keyword arguments for func
        
        Returns:
            Result of func
        
        Raises:
            RuntimeError: If circuit is open
            Exception: Any exception raised by func
        
        Validates:
            - Requirements 18.3: Circuit breaker mechanism
        """
        # Check if circuit should transition to half-open
        if self._state == self.STATE_OPEN:
            if time.time() - self._last_failure_time >= self.recovery_timeout:
                logger.info("Circuit breaker transitioning to HALF_OPEN")
                self._state = self.STATE_HALF_OPEN
                self._half_open_calls = 0
            else:
                raise RuntimeError(
                    f"Circuit breaker is OPEN. Redis operations are blocked. "
                    f"Will retry in {self.recovery_timeout - (time.time() - self._last_failure_time):.1f}s"
                )
        
        # Reject calls in half-open state if limit reached
        if self._state == self.STATE_HALF_OPEN:
            if self._half_open_calls >= self.half_open_max_calls:
                raise RuntimeError(
                    "Circuit breaker is HALF_OPEN and call limit reached. "
                    "Waiting for test calls to complete."
                )
            self._half_open_calls += 1
        
        # Execute the function
        try:
            result = await func(*args, **kwargs)
            
            # Success: reset or close circuit
            if self._state == self.STATE_HALF_OPEN:
                logger.info("Circuit breaker transitioning to CLOSED (recovery successful)")
                self._state = self.STATE_CLOSED
                self._failure_count = 0
                self._half_open_calls = 0
            elif self._state == self.STATE_CLOSED:
                self._failure_count = 0
            
            return result
            
        except Exception as e:
            # Failure: increment counter and potentially open circuit
            self._failure_count += 1
            self._last_failure_time = time.time()
            
            if self._state == self.STATE_HALF_OPEN:
                logger.warning("Circuit breaker transitioning to OPEN (recovery failed)")
                self._state = self.STATE_OPEN
                self._half_open_calls = 0
            elif self._failure_count >= self.failure_threshold:
                logger.error(
                    f"Circuit breaker transitioning to OPEN "
                    f"(failure threshold {self.failure_threshold} reached)"
                )
                self._state = self.STATE_OPEN
            
            raise
    
    @property
    def state(self) -> str:
        """
        Get current circuit breaker state.
        
        Returns:
            Current state (CLOSED, OPEN, or HALF_OPEN)
        """
        return self._state
    
    def reset(self) -> None:
        """
        Manually reset the circuit breaker to CLOSED state.
        """
        logger.info("Circuit breaker manually reset to CLOSED")
        self._state = self.STATE_CLOSED
        self._failure_count = 0
        self._half_open_calls = 0
