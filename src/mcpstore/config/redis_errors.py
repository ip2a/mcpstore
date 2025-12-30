"""
Redis connection error handling module.

This module provides comprehensive error handling for Redis connection failures,
including detailed error messages, password masking, and troubleshooting guidance.
"""

import logging
from typing import Dict, Any

from redis.exceptions import (
    ConnectionError as RedisConnectionError,
    AuthenticationError,
    TimeoutError as RedisTimeoutError,
    ResponseError,
    RedisError
)

logger = logging.getLogger(__name__)


class RedisConnectionFailure(Exception):
    """
    Custom exception for Redis connection failures with detailed context.
    
    This exception provides:
    - Masked connection details (passwords hidden)
    - Specific error categorization
    - Troubleshooting steps
    - Original exception for debugging
    """
    
    def __init__(
        self,
        message: str,
        connection_info: Dict[str, Any],
        original_error: Exception,
        troubleshooting_steps: list[str]
    ):
        self.message = message
        self.connection_info = connection_info
        self.original_error = original_error
        self.troubleshooting_steps = troubleshooting_steps
        
        # Build comprehensive error message
        full_message = self._build_error_message()
        super().__init__(full_message)
    
    def _build_error_message(self) -> str:
        """Build a comprehensive error message with all context."""
        lines = [
            "",
            "=" * 80,
            "Redis Connection Failure",
            "=" * 80,
            "",
            f"Error: {self.message}",
            "",
            "Connection Details:",
        ]
        
        # Add connection info (passwords already masked)
        for key, value in self.connection_info.items():
            lines.append(f"  {key}: {value}")
        
        lines.extend([
            "",
            "Original Error:",
            f"  {type(self.original_error).__name__}: {str(self.original_error)}",
            "",
            "Troubleshooting Steps:",
        ])
        
        # Add numbered troubleshooting steps
        for i, step in enumerate(self.troubleshooting_steps, 1):
            lines.append(f"  {i}. {step}")
        
        lines.extend([
            "",
            "=" * 80,
            ""
        ])
        
        return "\n".join(lines)


def mask_password_in_url(url: str) -> str:
    """
    Mask password in Redis URL for safe logging.
    
    Args:
        url: Redis URL (e.g., redis://user:password@host:port/db)
    
    Returns:
        URL with password masked (e.g., redis://user:***@host:port/db)
    
    Examples:
        >>> mask_password_in_url("redis://localhost:6379/0")
        'redis://localhost:6379/0'
        
        >>> mask_password_in_url("redis://:mypass@localhost:6379/0")
        'redis://:***@localhost:6379/0'
        
        >>> mask_password_in_url("redis://user:secret@localhost:6379/0")
        'redis://user:***@localhost:6379/0'
    """
    if not url:
        return url
    
    # Check if URL contains authentication
    if '@' not in url or '://' not in url:
        return url
    
    try:
        # Split by protocol
        parts = url.split('://', 1)
        if len(parts) != 2:
            return url
        
        protocol = parts[0]
        rest = parts[1]
        
        # Check if there's authentication
        if '@' not in rest:
            return url
        
        # Split authentication and host parts
        auth_and_host = rest.split('@', 1)
        if len(auth_and_host) != 2:
            return url
        
        auth_part = auth_and_host[0]
        host_part = auth_and_host[1]
        
        # Mask password in auth part
        if ':' in auth_part:
            # Format: user:password or :password
            auth_components = auth_part.split(':', 1)
            masked_auth = f"{auth_components[0]}:***"
        else:
            # No password, just username
            masked_auth = auth_part
        
        # Reconstruct URL
        return f"{protocol}://{masked_auth}@{host_part}"
    
    except Exception:
        # If anything goes wrong, return original URL
        # (better to show password than crash)
        return url


def get_connection_info(config: "RedisConfig") -> Dict[str, Any]:
    """
    Extract connection information from RedisConfig with password masking.
    
    Args:
        config: RedisConfig instance
    
    Returns:
        Dictionary with masked connection details
    """
    info = {}
    
    if config.url:
        info["url"] = mask_password_in_url(config.url)
    else:
        info["host"] = config.host or "localhost"
        info["port"] = config.port or 6379
        info["db"] = config.db or 0
        if config.password:
            info["password"] = "***"
    
    info["namespace"] = config.namespace or "mcpstore"
    info["max_connections"] = config.max_connections
    info["socket_timeout"] = f"{config.socket_timeout}s"
    info["socket_connect_timeout"] = f"{config.socket_connect_timeout}s"
    
    return info


def handle_redis_connection_error(
    error: Exception,
    config: "RedisConfig"
) -> RedisConnectionFailure:
    """
    Handle Redis connection errors and provide detailed troubleshooting.
    
    This function categorizes Redis errors and provides specific guidance:
    - Authentication errors: Check password and Redis AUTH configuration
    - Network errors: Check Redis server status and network connectivity
    - Timeout errors: Check network latency and timeout settings
    - Other errors: General troubleshooting steps
    
    Args:
        error: The original exception raised during connection
        config: RedisConfig instance with connection details
    
    Returns:
        RedisConnectionFailure with detailed context and troubleshooting
    
    Examples:
        >>> try:
        ...     # Connection attempt
        ...     pass
        ... except Exception as e:
        ...     raise handle_redis_connection_error(e, redis_config)
    """
    conn_info = get_connection_info(config)
    
    # Categorize error and provide specific guidance
    if isinstance(error, AuthenticationError):
        message = "Redis authentication failed"
        troubleshooting = [
            "Verify the Redis password is correct",
            "Check if Redis server requires authentication (requirepass in redis.conf)",
            "Ensure the password matches the Redis server configuration",
            "Try connecting with redis-cli to verify credentials: redis-cli -h <host> -p <port> -a <password>",
            "Check Redis ACL rules if using Redis 6+ ACL system"
        ]
    
    elif isinstance(error, (RedisConnectionError, OSError)):
        # Network connectivity issues
        message = "Cannot connect to Redis server"
        troubleshooting = [
            "Verify Redis server is running: redis-cli ping",
            "Check if Redis is listening on the specified host and port",
            "Verify network connectivity: ping <host>",
            "Check firewall rules allow connections to Redis port",
            "Ensure Redis bind address allows connections from your IP",
            "Check Redis logs for startup errors: tail -f /var/log/redis/redis-server.log"
        ]
    
    elif isinstance(error, RedisTimeoutError):
        message = "Redis connection timeout"
        troubleshooting = [
            f"Increase socket_connect_timeout (current: {config.socket_connect_timeout}s)",
            f"Increase socket_timeout (current: {config.socket_timeout}s)",
            "Check network latency to Redis server",
            "Verify Redis server is not overloaded: redis-cli --latency",
            "Check if Redis is performing slow operations: redis-cli slowlog get"
        ]
    
    elif isinstance(error, ResponseError):
        message = "Redis server returned an error response"
        troubleshooting = [
            "Check Redis server logs for detailed error information",
            "Verify Redis server version compatibility",
            "Check if Redis is in protected mode: CONFIG GET protected-mode",
            "Ensure Redis commands are not disabled in configuration"
        ]
    
    elif isinstance(error, RedisError):
        message = f"Redis error: {type(error).__name__}"
        troubleshooting = [
            "Check Redis server logs for detailed error information",
            "Verify Redis server is healthy: redis-cli ping",
            "Check Redis server memory usage: redis-cli info memory",
            "Review Redis configuration for any restrictions"
        ]
    
    else:
        # Generic error
        message = f"Unexpected error connecting to Redis: {type(error).__name__}"
        troubleshooting = [
            "Check Redis server status and logs",
            "Verify all connection parameters are correct",
            "Try connecting with redis-cli to isolate the issue",
            "Check system resources (memory, file descriptors)",
            "Review application logs for additional context"
        ]
    
    # Add common troubleshooting steps
    troubleshooting.extend([
        "",
        "Example working configuration:",
        "  RedisConfig(url='redis://localhost:6379/0')",
        "  RedisConfig(host='localhost', port=6379, db=0, password='your_password')"
    ])
    
    # Log the error with details
    logger.error(
        f"Redis connection failed: {message}",
        extra={
            "connection_info": conn_info,
            "error_type": type(error).__name__,
            "error_message": str(error)
        },
        exc_info=True
    )
    
    return RedisConnectionFailure(
        message=message,
        connection_info=conn_info,
        original_error=error,
        troubleshooting_steps=troubleshooting
    )


async def test_redis_connection(config: "RedisConfig") -> None:
    """
    Test Redis connection with fail-fast strategy.
    
    This function attempts to connect to Redis and execute a PING command
    to verify the connection is working. If the connection fails, it raises
    a detailed RedisConnectionFailure exception.
    
    Args:
        config: RedisConfig instance to test
    
    Raises:
        RedisConnectionFailure: If connection fails with detailed troubleshooting
    
    Examples:
        >>> config = RedisConfig(url="redis://localhost:6379/0")
        >>> await test_redis_connection(config)  # Raises if connection fails
    """
    from redis.asyncio import Redis
    
    client = None
    try:
        # Create a test client
        if config.client:
            # Use provided client
            client = config.client
            close_client = False
        elif config.url:
            # Create from URL
            client = Redis.from_url(
                config.url,
                socket_connect_timeout=config.socket_connect_timeout,
                socket_timeout=config.socket_timeout,
                max_connections=config.max_connections,
                socket_keepalive=config.socket_keepalive
            )
            close_client = True
        else:
            # Create from parameters
            client = Redis(
                host=config.host,
                port=config.port or 6379,
                db=config.db or 0,
                password=config.password,
                socket_connect_timeout=config.socket_connect_timeout,
                socket_timeout=config.socket_timeout,
                max_connections=config.max_connections,
                socket_keepalive=config.socket_keepalive
            )
            close_client = True
        
        # Test connection with PING
        logger.debug("Testing Redis connection with PING command...")
        response = await client.ping()
        
        if not response:
            raise RedisConnectionError("PING command failed")
        
        logger.debug("Redis connection test successful")
        
    except Exception as e:
        # Handle and re-raise with detailed context
        raise handle_redis_connection_error(e, config)
    
    finally:
        # Close test client if we created it
        if client and close_client:
            try:
                await client.aclose()
            except Exception:
                pass
