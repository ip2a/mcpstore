"""
Redis configuration parser and validator.

This module provides utilities for parsing and validating Redis connection
configuration from user-provided config dictionaries.

Validates:
    - Requirements 18.1: Basic connection configuration
    - Requirements 18.2: Connection pool configuration
"""

from __future__ import annotations

import logging
from typing import Any, Dict, Optional

from ...config.config_defaults import CacheRedisConfigDefaults

logger = logging.getLogger(__name__)

_redis_defaults = CacheRedisConfigDefaults()


class RedisConfig:
    """
    Parsed and validated Redis configuration.
    
    This class encapsulates all Redis connection-related configuration options,
    providing defaults and validation.
    
    Attributes:
        url: Redis connection URL (required)
        password: Redis password (optional)
        socket_timeout: Socket timeout in seconds
        socket_connect_timeout: Connection timeout in seconds
        max_connections: Maximum number of connections in pool
        healthcheck_interval: Health check interval in seconds
    
    Validates:
        - Requirements 18.1: Basic connection configuration
        - Requirements 18.2: Connection pool configuration
    """
    
    # Default values
    DEFAULT_SOCKET_TIMEOUT = _redis_defaults.socket_timeout  # seconds
    DEFAULT_SOCKET_CONNECT_TIMEOUT = _redis_defaults.socket_connect_timeout  # seconds
    DEFAULT_MAX_CONNECTIONS = _redis_defaults.max_connections
    DEFAULT_HEALTHCHECK_INTERVAL = _redis_defaults.health_check_interval  # seconds
    
    def __init__(
        self,
        url: str,
        password: Optional[str] = None,
        socket_timeout: float = DEFAULT_SOCKET_TIMEOUT,
        socket_connect_timeout: float = DEFAULT_SOCKET_CONNECT_TIMEOUT,
        max_connections: int = DEFAULT_MAX_CONNECTIONS,
        healthcheck_interval: int = DEFAULT_HEALTHCHECK_INTERVAL
    ):
        """
        Initialize Redis configuration.
        
        Args:
            url: Redis connection URL (e.g., "redis://localhost:6379/0")
            password: Redis password (optional)
            socket_timeout: Socket timeout in seconds
            socket_connect_timeout: Connection timeout in seconds
            max_connections: Maximum number of connections in pool
            healthcheck_interval: Health check interval in seconds
        
        Raises:
            ValueError: If configuration is invalid
        """
        self.url = url
        self.password = password
        self.socket_timeout = socket_timeout
        self.socket_connect_timeout = socket_connect_timeout
        self.max_connections = max_connections
        self.healthcheck_interval = healthcheck_interval
        
        # Validate configuration
        self._validate()
    
    def _validate(self) -> None:
        """
        Validate configuration values.
        
        Raises:
            ValueError: If configuration is invalid
        """
        # Validate URL
        if not self.url:
            raise ValueError("Redis URL is required")
        
        if not isinstance(self.url, str):
            raise ValueError(f"Redis URL must be a string, got: {type(self.url)}")
        
        # Basic URL format validation
        if not (self.url.startswith("redis://") or self.url.startswith("rediss://")):
            raise ValueError(
                f"Redis URL must start with 'redis://' or 'rediss://', got: {self.url}"
            )
        
        # Validate socket_timeout
        if not isinstance(self.socket_timeout, (int, float)) or self.socket_timeout <= 0:
            raise ValueError(
                f"socket_timeout must be a positive number, got: {self.socket_timeout}"
            )
        
        # Validate socket_connect_timeout
        if not isinstance(self.socket_connect_timeout, (int, float)) or self.socket_connect_timeout <= 0:
            raise ValueError(
                f"socket_connect_timeout must be a positive number, got: {self.socket_connect_timeout}"
            )
        
        # Validate max_connections
        if not isinstance(self.max_connections, int) or self.max_connections <= 0:
            raise ValueError(
                f"max_connections must be a positive integer, got: {self.max_connections}"
            )
        
        # Warn if max_connections is too small
        if self.max_connections < 5:
            logger.warning(
                f"max_connections is very small ({self.max_connections}). "
                f"This may cause connection pool exhaustion under load."
            )
        
        # Validate healthcheck_interval
        if not isinstance(self.healthcheck_interval, (int, float)) or self.healthcheck_interval <= 0:
            raise ValueError(
                f"healthcheck_interval must be a positive number, got: {self.healthcheck_interval}"
            )
    
    @classmethod
    def from_dict(cls, config: Dict[str, Any]) -> 'RedisConfig':
        """
        Parse Redis configuration from a dictionary.
        
        Args:
            config: Configuration dictionary with keys:
                - url: str (required) - Redis connection URL
                - password: str (optional) - Redis password
                - socket_timeout: float (optional, default: 2.0) - Socket timeout
                - socket_connect_timeout: float (optional, default: 2.0) - Connect timeout
                - max_connections: int (optional, default: 50) - Max connections
                - healthcheck_interval: int (optional, default: 30) - Health check interval
        
        Returns:
            RedisConfig instance with parsed values
        
        Raises:
            ValueError: If required fields are missing or invalid
        
        Examples:
            >>> # Minimal configuration
            >>> config = RedisConfig.from_dict({
            ...     "url": "redis://localhost:6379/0"
            ... })
            
            >>> # Full configuration
            >>> config = RedisConfig.from_dict({
            ...     "url": "redis://prod-redis:6379/0",
            ...     "password": "secret",
            ...     "socket_timeout": 5.0,
            ...     "socket_connect_timeout": 3.0,
            ...     "max_connections": 100,
            ...     "healthcheck_interval": 60
            ... })
        
        Validates:
            - Requirements 18.1: Parse basic configuration such as URL, password, timeout
            - Requirements 18.2: Parse connection pool configuration
        """
        # Validate required fields
        url = config.get("url")
        if not url:
            raise ValueError(
                "Redis configuration requires 'url' field. "
                "Example: {'url': 'redis://localhost:6379/0'}"
            )
        
        # Parse optional fields with defaults
        # Use 'or' to handle both missing keys and explicit None values
        password = config.get("password")
        socket_timeout = config.get("socket_timeout") or cls.DEFAULT_SOCKET_TIMEOUT
        socket_connect_timeout = config.get("socket_connect_timeout") or cls.DEFAULT_SOCKET_CONNECT_TIMEOUT
        max_connections = config.get("max_connections") or cls.DEFAULT_MAX_CONNECTIONS
        healthcheck_interval = config.get("healthcheck_interval") or cls.DEFAULT_HEALTHCHECK_INTERVAL
        
        # Type coercion for robustness
        try:
            socket_timeout = float(socket_timeout)
            socket_connect_timeout = float(socket_connect_timeout)
            max_connections = int(max_connections)
            healthcheck_interval = float(healthcheck_interval)
        except (TypeError, ValueError) as e:
            raise ValueError(
                f"Invalid Redis configuration: {e}. "
                f"Example: {{'type': 'redis', 'url': 'redis://localhost:6379/0'}}"
            ) from e
        
        logger.debug(
            f"Parsed Redis config: url={url}, "
            f"socket_timeout={socket_timeout}s, "
            f"socket_connect_timeout={socket_connect_timeout}s, "
            f"max_connections={max_connections}, "
            f"healthcheck_interval={healthcheck_interval}s"
        )
        
        return cls(
            url=url,
            password=password,
            socket_timeout=socket_timeout,
            socket_connect_timeout=socket_connect_timeout,
            max_connections=max_connections,
            healthcheck_interval=healthcheck_interval
        )
    
    def to_dict(self) -> Dict[str, Any]:
        """
        Convert configuration to dictionary.
        
        Returns:
            Dictionary representation of configuration
        """
        result = {
            "url": self.url,
            "socket_timeout": self.socket_timeout,
            "socket_connect_timeout": self.socket_connect_timeout,
            "max_connections": self.max_connections,
            "healthcheck_interval": self.healthcheck_interval
        }
        
        # Only include password if set
        if self.password:
            result["password"] = self.password
        
        return result
    
    def to_redis_kwargs(self) -> Dict[str, Any]:
        """
        Convert configuration to kwargs for py-key-value RedisStore constructor.
        
        Note:
            py-key-value RedisStore only accepts: url, host, port, db, password, client, default_collection
            Connection pool and timeout settings are not directly supported by py-key-value RedisStore.
            These settings are stored in this config for potential future use or custom client creation.
        
        Returns:
            Dictionary of kwargs suitable for RedisStore(**kwargs)
        """
        kwargs = {
            "url": self.url,
        }
        
        # Only include password if set
        if self.password:
            kwargs["password"] = self.password
        
        return kwargs
    
    def __repr__(self) -> str:
        """String representation of configuration."""
        # Mask password for security
        password_display = "***" if self.password else "None"
        return (
            f"RedisConfig("
            f"url={self.url}, "
            f"password={password_display}, "
            f"socket_timeout={self.socket_timeout}s, "
            f"socket_connect_timeout={self.socket_connect_timeout}s, "
            f"max_connections={self.max_connections}, "
            f"healthcheck_interval={self.healthcheck_interval}s)"
        )


def parse_redis_config(config: Dict[str, Any]) -> RedisConfig:
    """
    Parse Redis configuration from a dictionary.
    
    This is a convenience function that delegates to RedisConfig.from_dict().
    
    Args:
        config: Configuration dictionary
    
    Returns:
        RedisConfig instance
    
    Raises:
        ValueError: If configuration is invalid
    
    Examples:
        >>> config = parse_redis_config({
        ...     "url": "redis://localhost:6379/0",
        ...     "password": "secret"
        ... })
        >>> print(config.url)
        redis://localhost:6379/0
    
    Validates:
        - Requirements 18.1: Parse basic configuration such as URL, password, timeout
        - Requirements 18.2: Parse connection pool configuration
    """
    return RedisConfig.from_dict(config)
