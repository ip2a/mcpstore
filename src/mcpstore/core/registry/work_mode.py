"""
工作模式检测和管理模块

支持三种工作模式：
1. 本地模式 (local): JSON + Memory
2. 混合模式 (hybrid): JSON + Redis
3. 共享模式 (shared): Redis Only

根据 mcpjson_path 和 external_db 参数自动检测工作模式。
"""

import logging
from typing import Optional, Dict, Any, Literal

logger = logging.getLogger(__name__)

# 工作模式类型定义
WorkMode = Literal["local", "hybrid", "shared"]


def detect_cache_mode(
    mcpjson_path: Optional[str],
    external_db: Optional[Dict[str, Any]]
) -> WorkMode:
    """
    自动检测缓存工作模式
    
    Args:
        mcpjson_path: mcp.json 配置文件路径，None 表示无配置文件
        external_db: 外部数据库配置字典，包含 cache 配置
    
    Returns:
        检测到的工作模式: "local" | "hybrid" | "shared"
    
    检测逻辑:
        - 无 JSON + Redis → 共享模式 (shared)
        - 有 JSON + Redis → 混合模式 (hybrid)
        - 有 JSON + 无 Redis → 本地模式 (local)
        - 默认 → 本地模式 (local)
    
    Examples:
        >>> # 场景 1: 共享模式
        >>> detect_cache_mode(None, {"cache": {"type": "redis", "url": "..."}})
        'shared'
        
        >>> # 场景 2: 混合模式
        >>> detect_cache_mode("./mcp.json", {"cache": {"type": "redis", "url": "..."}})
        'hybrid'
        
        >>> # 场景 3: 本地模式
        >>> detect_cache_mode("./mcp.json", None)
        'local'
        
        >>> # 场景 4: 默认本地模式
        >>> detect_cache_mode("./mcp.json", {"cache": {"type": "memory"}})
        'local'
    """
    # 检查是否配置了 Redis
    has_redis = False
    if external_db and isinstance(external_db, dict):
        cache_config = external_db.get("cache", {})
        if isinstance(cache_config, dict):
            has_redis = cache_config.get("type") == "redis"
    
    # 场景 1: 无 JSON + Redis → 共享模式
    if mcpjson_path is None and has_redis:
        logger.info("Detected cache mode: shared (no JSON, Redis backend)")
        return "shared"
    
    # 场景 2: 有 JSON + Redis → 混合模式
    if mcpjson_path and has_redis:
        logger.info("Detected cache mode: hybrid (JSON config, Redis backend)")
        return "hybrid"
    
    # 场景 3: 有 JSON + 无 Redis → 本地模式
    # 场景 4: 默认 → 本地模式
    logger.info("Detected cache mode: local (JSON config, Memory backend)")
    return "local"


def get_mode_description(mode: WorkMode) -> str:
    """
    获取工作模式的描述信息
    
    Args:
        mode: 工作模式
    
    Returns:
        模式描述字符串
    """
    descriptions = {
        "local": "本地模式 - 从 JSON 加载配置，数据存储在内存",
        "hybrid": "混合模式 - 从 JSON 加载配置，数据存储在 Redis",
        "shared": "共享模式 - 从 Redis 加载配置，支持多实例共享"
    }
    return descriptions.get(mode, "未知模式")


def validate_mode_config(
    mode: WorkMode,
    mcpjson_path: Optional[str],
    external_db: Optional[Dict[str, Any]]
) -> None:
    """
    验证工作模式配置的有效性
    
    Args:
        mode: 工作模式
        mcpjson_path: JSON 配置文件路径
        external_db: 外部数据库配置
    
    Raises:
        ValueError: 如果配置与模式不匹配
    
    Examples:
        >>> # 共享模式必须有 Redis
        >>> validate_mode_config("shared", None, None)
        Traceback (most recent call last):
        ...
        ValueError: Shared mode requires Redis configuration
        
        >>> # 本地模式必须有 JSON
        >>> validate_mode_config("local", None, None)
        Traceback (most recent call last):
        ...
        ValueError: Local mode requires mcp.json path
    """
    if mode == "shared":
        # 共享模式必须有 Redis
        if not external_db or not isinstance(external_db, dict):
            raise ValueError("Shared mode requires Redis configuration in external_db")
        
        cache_config = external_db.get("cache", {})
        if not isinstance(cache_config, dict) or cache_config.get("type") != "redis":
            raise ValueError("Shared mode requires Redis cache backend")
        
        if not cache_config.get("url"):
            raise ValueError("Shared mode requires Redis URL in cache configuration")
    
    elif mode == "local":
        # 本地模式必须有 JSON（或使用默认配置）
        # 注意：mcpjson_path 可以为 None，此时使用默认配置
        pass
    
    elif mode == "hybrid":
        # 混合模式必须同时有 JSON 和 Redis
        if not mcpjson_path:
            raise ValueError("Hybrid mode requires mcp.json path")
        
        if not external_db or not isinstance(external_db, dict):
            raise ValueError("Hybrid mode requires Redis configuration in external_db")
        
        cache_config = external_db.get("cache", {})
        if not isinstance(cache_config, dict) or cache_config.get("type") != "redis":
            raise ValueError("Hybrid mode requires Redis cache backend")
        
        if not cache_config.get("url"):
            raise ValueError("Hybrid mode requires Redis URL in cache configuration")
    
    else:
        raise ValueError(f"Unknown work mode: {mode}")


def should_load_from_json(mode: WorkMode) -> bool:
    """
    判断是否应该从 JSON 加载配置
    
    Args:
        mode: 工作模式
    
    Returns:
        True 如果应该从 JSON 加载，False 否则
    """
    return mode in ("local", "hybrid")


def should_load_from_cache(mode: WorkMode) -> bool:
    """
    判断是否应该从缓存加载配置
    
    Args:
        mode: 工作模式
    
    Returns:
        True 如果应该从缓存加载，False 否则
    """
    return mode == "shared"


def should_sync_to_cache(mode: WorkMode) -> bool:
    """
    判断是否应该同步配置到缓存
    
    Args:
        mode: 工作模式
    
    Returns:
        True 如果应该同步到缓存，False 否则
    """
    return mode in ("hybrid", "shared")
