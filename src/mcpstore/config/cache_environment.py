"""
Cache environment variables management.

This module handles reading sensitive cache configuration from environment variables,
ensuring sensitive data never gets stored in TOML files or KV storage.
"""

import logging

logger = logging.getLogger(__name__)


def get_sensitive_redis_config() -> dict:
    """
    Get all sensitive Redis configuration from environment variables.

    Returns:
        Dictionary containing sensitive configuration
    """
    config = {}
    return config


def get_cache_type_from_env() -> str:
    """
    Get cache type from environment variables.

    Returns:
        Cache type ('memory' or 'redis'), defaults to 'memory'
    """
    return "memory"
