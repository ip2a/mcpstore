"""
Configuration parser for py-key-value wrapper chain.

This module provides utilities for parsing and validating wrapper configuration
from user-provided config dictionaries.

Validates:
    - Requirements 17.1: 统计包装器配置
    - Requirements 17.2: 大小限制包装器配置
    - Requirements 17.3: 压缩包装器配置
"""

from __future__ import annotations

import logging
from typing import Any, Dict, Optional

from ...config.config_defaults import WrapperConfigDefaults

logger = logging.getLogger(__name__)

_wrapper_defaults = WrapperConfigDefaults()


class WrapperConfig:
    """
    Parsed and validated wrapper configuration.
    
    This class encapsulates all wrapper-related configuration options,
    providing defaults and validation.
    
    Attributes:
        enable_statistics: Whether to enable StatisticsWrapper
        enable_size_limit: Whether to enable LimitSizeWrapper
        max_item_size: Maximum item size in bytes (for LimitSizeWrapper)
        enable_compression: Whether to enable CompressionWrapper
        compression_threshold: Compression threshold in bytes
    
    Validates:
        - Requirements 17.1: 统计包装器配置
        - Requirements 17.2: 大小限制包装器配置
        - Requirements 17.3: 压缩包装器配置
    """
    
    # Default values
    DEFAULT_ENABLE_STATISTICS = True
    DEFAULT_ENABLE_SIZE_LIMIT = True
    DEFAULT_MAX_ITEM_SIZE = _wrapper_defaults.DEFAULT_MAX_ITEM_SIZE  # 1MB
    DEFAULT_ENABLE_COMPRESSION = False
    DEFAULT_COMPRESSION_THRESHOLD = _wrapper_defaults.DEFAULT_COMPRESSION_THRESHOLD  # 由 WrapperConfigDefaults 统一管理
    
    def __init__(
        self,
        enable_statistics: bool = DEFAULT_ENABLE_STATISTICS,
        enable_size_limit: bool = DEFAULT_ENABLE_SIZE_LIMIT,
        max_item_size: int = DEFAULT_MAX_ITEM_SIZE,
        enable_compression: bool = DEFAULT_ENABLE_COMPRESSION,
        compression_threshold: int = DEFAULT_COMPRESSION_THRESHOLD
    ):
        """
        Initialize wrapper configuration.
        
        Args:
            enable_statistics: Enable statistics wrapper
            enable_size_limit: Enable size limit wrapper
            max_item_size: Maximum item size in bytes
            enable_compression: Enable compression wrapper
            compression_threshold: Compression threshold in bytes
        """
        self.enable_statistics = enable_statistics
        self.enable_size_limit = enable_size_limit
        self.max_item_size = max_item_size
        self.enable_compression = enable_compression
        self.compression_threshold = compression_threshold
        
        # Validate configuration
        self._validate()
    
    def _validate(self) -> None:
        """
        Validate configuration values.
        
        Raises:
            ValueError: If configuration is invalid
        """
        # Validate max_item_size
        if self.enable_size_limit:
            if not isinstance(self.max_item_size, int) or self.max_item_size <= 0:
                raise ValueError(
                    f"max_item_size must be a positive integer, got: {self.max_item_size}"
                )
            
            # Warn if size is too small
            if self.max_item_size < 1024:  # Less than 1KB
                logger.warning(
                    f"max_item_size is very small ({self.max_item_size} bytes). "
                    f"This may cause issues with normal data."
                )
        
        # Validate compression_threshold
        if self.enable_compression:
            if not isinstance(self.compression_threshold, int) or self.compression_threshold <= 0:
                raise ValueError(
                    f"compression_threshold must be a positive integer, got: {self.compression_threshold}"
                )
            
            # Warn if threshold is larger than max size
            if self.enable_size_limit and self.compression_threshold > self.max_item_size:
                logger.warning(
                    f"compression_threshold ({self.compression_threshold}) is larger than "
                    f"max_item_size ({self.max_item_size}). Compression may never trigger."
                )
    
    @classmethod
    def from_dict(cls, config: Optional[Dict[str, Any]] = None) -> 'WrapperConfig':
        """
        Parse wrapper configuration from a dictionary.
        
        Args:
            config: Configuration dictionary with optional keys:
                - enable_statistics: bool (default: True)
                - enable_size_limit: bool (default: True)
                - max_item_size: int (default: 1MB)
                - enable_compression: bool (default: False)
                - compression_threshold: int (default: 512KB)
        
        Returns:
            WrapperConfig instance with parsed values
        
        Examples:
            >>> # Use defaults
            >>> config = WrapperConfig.from_dict()
            
            >>> # Custom configuration
            >>> config = WrapperConfig.from_dict({
            ...     "enable_statistics": True,
            ...     "enable_size_limit": True,
            ...     "max_item_size": 2 * 1024 * 1024,  # 2MB
            ...     "enable_compression": True,
            ...     "compression_threshold": 1024 * 1024  # 1MB
            ... })
        
        Validates:
            - Requirements 17.1: 解析 enable_statistics
            - Requirements 17.2: 解析 enable_size_limit 和 max_item_size
            - Requirements 17.3: 解析 enable_compression 和 compression_threshold
        """
        config = config or {}
        
        # Parse each configuration option with defaults
        enable_statistics = config.get("enable_statistics", cls.DEFAULT_ENABLE_STATISTICS)
        enable_size_limit = config.get("enable_size_limit", cls.DEFAULT_ENABLE_SIZE_LIMIT)
        max_item_size = config.get("max_item_size", cls.DEFAULT_MAX_ITEM_SIZE)
        enable_compression = config.get("enable_compression", cls.DEFAULT_ENABLE_COMPRESSION)
        compression_threshold = config.get("compression_threshold", cls.DEFAULT_COMPRESSION_THRESHOLD)
        
        # Type coercion for robustness
        try:
            enable_statistics = bool(enable_statistics)
            enable_size_limit = bool(enable_size_limit)
            max_item_size = int(max_item_size)
            enable_compression = bool(enable_compression)
            compression_threshold = int(compression_threshold)
        except (TypeError, ValueError) as e:
            raise ValueError(f"Invalid wrapper configuration: {e}") from e
        
        logger.debug(
            f"Parsed wrapper config: statistics={enable_statistics}, "
            f"size_limit={enable_size_limit} (max={max_item_size}), "
            f"compression={enable_compression} (threshold={compression_threshold})"
        )
        
        return cls(
            enable_statistics=enable_statistics,
            enable_size_limit=enable_size_limit,
            max_item_size=max_item_size,
            enable_compression=enable_compression,
            compression_threshold=compression_threshold
        )
    
    def to_dict(self) -> Dict[str, Any]:
        """
        Convert configuration to dictionary.
        
        Returns:
            Dictionary representation of configuration
        """
        return {
            "enable_statistics": self.enable_statistics,
            "enable_size_limit": self.enable_size_limit,
            "max_item_size": self.max_item_size,
            "enable_compression": self.enable_compression,
            "compression_threshold": self.compression_threshold
        }
    
    def __repr__(self) -> str:
        """String representation of configuration."""
        return (
            f"WrapperConfig("
            f"statistics={self.enable_statistics}, "
            f"size_limit={self.enable_size_limit}, "
            f"max_size={self.max_item_size}, "
            f"compression={self.enable_compression}, "
            f"threshold={self.compression_threshold})"
        )


def parse_wrapper_config(config: Optional[Dict[str, Any]] = None) -> WrapperConfig:
    """
    Parse wrapper configuration from a dictionary.
    
    This is a convenience function that delegates to WrapperConfig.from_dict().
    
    Args:
        config: Configuration dictionary
    
    Returns:
        WrapperConfig instance
    
    Examples:
        >>> config = parse_wrapper_config({"enable_statistics": True})
        >>> print(config.enable_statistics)
        True
    
    Validates:
        - Requirements 17.1: 解析 enable_statistics
        - Requirements 17.2: 解析 enable_size_limit 和 max_item_size
        - Requirements 17.3: 解析 enable_compression 和 compression_threshold
    """
    return WrapperConfig.from_dict(config)
