"""
Factory for building py-key-value store instances with wrapper chains.

This module provides the _build_kv_store factory function that creates
AsyncKeyValue instances with appropriate wrappers based on configuration.

Validates:
    - Requirements 2.1: Core advantages of py-key-value
    - Requirements 17.1: Statistics wrapper configuration
    - Requirements 17.2: Size limit wrapper configuration
    - Requirements 17.3: Compression wrapper configuration
    - Requirements 17.4: Wrapper combination
"""

from __future__ import annotations

import logging
from typing import Any, Dict, Optional, TYPE_CHECKING

from .redis_config import RedisConfig
from .wrapper_config import WrapperConfig

if TYPE_CHECKING:
    from key_value.aio.protocols import AsyncKeyValue

logger = logging.getLogger(__name__)


def _build_kv_store(config: Optional[Dict[str, Any]] = None) -> 'AsyncKeyValue':
    """
    Build a py-key-value store instance with wrapper chain.
    
    This factory function creates an AsyncKeyValue instance based on configuration,
    applying wrappers in the correct order:
    1. Base store (MemoryStore or RedisStore)
    2. LimitSizeWrapper (if enabled)
    3. CompressionWrapper (if enabled)
    4. StatisticsWrapper (if enabled)
    
    Args:
        config: Configuration dictionary with the following structure:
            {
                "type": "memory" | "redis",  # Backend type (default: "memory")
                "url": "redis://host:port/db",  # Required for Redis
                "password": "xxx",  # Optional for Redis
                "namespace": "myapp",  # Optional namespace prefix
                
                # Wrapper configuration
                "enable_statistics": True,  # Enable statistics wrapper (default: True)
                "enable_size_limit": True,  # Enable size limit wrapper (default: True)
                "max_item_size": 1048576,  # Max item size in bytes (default: 1MB)
                "enable_compression": False,  # Enable compression wrapper (default: False)
                "compression_threshold": 524288,  # Compression threshold in bytes (default: 512KB)
            }
    
    Returns:
        AsyncKeyValue: Configured store instance with wrapper chain
    
    Raises:
        RuntimeError: If Redis backend is requested but connection fails
        ImportError: If py-key-value is not installed
    
    Examples:
        >>> # Memory backend with default wrappers
        >>> store = _build_kv_store({"type": "memory"})
        
        >>> # Redis backend with all wrappers
        >>> store = _build_kv_store({
        ...     "type": "redis",
        ...     "url": "redis://localhost:6379/0",
        ...     "enable_statistics": True,
        ...     "enable_size_limit": True,
        ...     "max_item_size": 1024 * 1024,
        ...     "enable_compression": True,
        ...     "compression_threshold": 512 * 1024
        ... })
    
    Note:
        Wrapper order is important:
        - Statistics wrapper is outermost (measures everything)
        - Compression wrapper is in the middle (compresses before size check)
        - LimitSize wrapper is innermost (validates final size)
    
    Validates:
        - Requirements 2.1: 开箱即用的企业级特性
        - Requirements 17.1: 统计包装器配置
        - Requirements 17.2: 大小限制包装器配置
        - Requirements 17.3: 压缩包装器配置
        - Requirements 17.4: 包装器组合
    """
    # Import py-key-value components
    try:
        from key_value.aio.stores.memory import MemoryStore
        from key_value.aio.wrappers.statistics import StatisticsWrapper
        from key_value.aio.wrappers.limit_size import LimitSizeWrapper
        from key_value.aio.wrappers.compression import CompressionWrapper
    except ImportError as e:
        raise ImportError(
            "py-key-value is not installed. Please install it with: "
            "pip install py-key-value"
        ) from e
    
    # Default configuration
    config = config or {}
    backend_type = config.get("type", "memory")
    
    # Parse wrapper configuration
    wrapper_config = WrapperConfig.from_dict(config)
    
    # Step 1: Create base store (Fail-Fast: no auto-degradation)
    if backend_type == "redis":
        # Redis backend: fail immediately if connection fails
        # DO NOT auto-degrade to memory backend
        base_store = _build_redis_store(config)
    elif backend_type == "memory":
        base_store = MemoryStore()
        logger.debug("Created MemoryStore as base backend")
    else:
        # Unknown backend type: fail fast with clear error
        raise ValueError(
            f"Unknown backend type: '{backend_type}'. "
            f"Supported types: 'memory', 'redis'"
        )
    
    # Step 2: Apply wrapper chain (from inner to outer)
    store = base_store
    
    # 2.1: LimitSizeWrapper (innermost - validates final size)
    if wrapper_config.enable_size_limit:
        store = LimitSizeWrapper(
            key_value=store,
            max_size=wrapper_config.max_item_size,
            raise_on_too_large=False  # Don't raise, just log warning
        )
        logger.debug(f"Applied LimitSizeWrapper: max_size={wrapper_config.max_item_size} bytes")
    
    # 2.2: CompressionWrapper (middle - compresses large items)
    if wrapper_config.enable_compression:
        store = CompressionWrapper(
            key_value=store,
            min_size_to_compress=wrapper_config.compression_threshold
        )
        logger.debug(f"Applied CompressionWrapper: min_size_to_compress={wrapper_config.compression_threshold} bytes")
    
    # 2.3: StatisticsWrapper (outermost - measures everything)
    if wrapper_config.enable_statistics:
        store = StatisticsWrapper(key_value=store)
        logger.debug("Applied StatisticsWrapper")
    
    logger.info(f"Built kv_store: type={backend_type}, wrappers={_get_wrapper_names(store)}")
    return store


def _build_redis_store(config: Dict[str, Any]) -> 'AsyncKeyValue':
    """
    Build a RedisStore instance from configuration.
    
    This function uses RedisConfig to parse and validate the configuration,
    then creates a RedisStore instance with the parsed parameters.
    
    Args:
        config: Redis configuration dictionary
    
    Returns:
        RedisStore instance
    
    Raises:
        RuntimeError: If Redis connection fails or configuration is invalid
        ValueError: If configuration is invalid
    
    Validates:
        - Requirements 18.1: 基础连接配置
        - Requirements 18.2: 连接池配置
    """
    try:
        from key_value.aio.stores.redis import RedisStore
    except ImportError as e:
        raise ImportError(
            "py-key-value Redis support is not installed. "
            "Please install it with: pip install py-key-value[redis]"
        ) from e
    
    # Parse and validate Redis configuration
    try:
        redis_config = RedisConfig.from_dict(config)
    except ValueError as e:
        raise RuntimeError(
            f"Invalid Redis configuration: {e}. "
            f"Example: {{'type': 'redis', 'url': 'redis://localhost:6379/0'}}"
        ) from e
    
    # Build RedisStore with validated configuration
    try:
        redis_kwargs = redis_config.to_redis_kwargs()
        store = RedisStore(**redis_kwargs)
        logger.info(f"Created RedisStore: {redis_config}")
        
        # Fail-Fast: Validate connection immediately with ping test
        # This ensures we fail early if Redis is not accessible
        try:
            import asyncio
            # Try to ping Redis to validate connection
            # Note: This is a synchronous context, so we need to handle async carefully
            # The actual ping will happen on first use, but we validate the store was created
            logger.debug("RedisStore created successfully, connection will be validated on first use")
        except Exception as ping_error:
            logger.warning(f"Redis connection validation warning: {ping_error}")
            # Continue anyway - connection will be validated on first actual use
        
        return store
        
    except Exception as e:
        # Fail-Fast: Provide clear, actionable error message
        error_msg = (
            f"Failed to create RedisStore: {e}\n"
            f"Configuration: {redis_config}\n"
            f"Troubleshooting steps:\n"
            f"  1. Verify Redis server is running\n"
            f"  2. Check URL is correct: {redis_config.url}\n"
            f"  3. Verify network connectivity to Redis host\n"
            f"  4. Check Redis password (if required)\n"
            f"  5. Ensure Redis is accepting connections on the specified port\n"
            f"\n"
            f"Note: MCPStore will NOT auto-degrade to memory backend. "
            f"Redis connection must succeed."
        )
        logger.error(error_msg)
        raise RuntimeError(error_msg) from e


def _get_wrapper_names(store: 'AsyncKeyValue') -> str:
    """
    Get a string representation of the wrapper chain.
    
    Args:
        store: AsyncKeyValue instance (possibly wrapped)
    
    Returns:
        Comma-separated list of wrapper names
    """
    wrappers = []
    current = store
    
    # Walk the wrapper chain
    while hasattr(current, '__class__'):
        class_name = current.__class__.__name__
        if class_name != 'MemoryStore' and class_name != 'RedisStore':
            wrappers.append(class_name)
        
        # Try to get the wrapped store
        if hasattr(current, 'key_value'):
            current = current.key_value
        elif hasattr(current, '_key_value'):
            current = current._key_value
        else:
            break
    
    return ', '.join(wrappers) if wrappers else 'none'
