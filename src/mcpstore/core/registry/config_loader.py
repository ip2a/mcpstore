"""
配置加载模块

根据工作模式从不同的源加载配置：
- 本地模式: 从 JSON 文件加载
- 混合模式: 从 JSON 文件加载并同步到 Redis
- 共享模式: 从 Redis 加载

支持配置的导入和导出功能。
"""

import json
import logging
from pathlib import Path
from typing import Optional, Dict, Any, TYPE_CHECKING

from .work_mode import WorkMode, should_load_from_json, should_load_from_cache, should_sync_to_cache

if TYPE_CHECKING:
    from key_value.aio.protocols import AsyncKeyValue

logger = logging.getLogger(__name__)


async def load_configuration(
    mode: WorkMode,
    mcpjson_path: Optional[str],
    kv_store: 'AsyncKeyValue',
    namespace: Optional[str] = None
) -> Dict[str, Any]:
    """
    根据工作模式加载配置
    
    Args:
        mode: 工作模式 (local/hybrid/shared)
        mcpjson_path: JSON 配置文件路径
        kv_store: py-key-value 存储实例
        namespace: 命名空间前缀（用于 Redis 键隔离）
    
    Returns:
        加载的配置字典
    
    Raises:
        FileNotFoundError: 如果 JSON 文件不存在（本地/混合模式）
        RuntimeError: 如果从缓存加载失败（共享模式）
    
    工作流程:
        - 本地模式: 从 JSON 加载 → 返回配置
        - 混合模式: 从 JSON 加载 → 同步到 Redis → 返回配置
        - 共享模式: 从 Redis 加载 → 返回配置
    """
    if mode == "local":
        # 本地模式: 从 JSON 加载
        logger.info(f"Loading configuration from JSON (local mode): {mcpjson_path}")
        config = load_from_json(mcpjson_path)
        logger.debug(f"Loaded {len(config)} configuration items from JSON")
        return config
    
    elif mode == "hybrid":
        # 混合模式: 从 JSON 加载并同步到 Redis
        logger.info(f"Loading configuration from JSON (hybrid mode): {mcpjson_path}")
        config = load_from_json(mcpjson_path)
        
        logger.info("Syncing configuration to Redis cache")
        await sync_config_to_cache(kv_store, config, namespace)
        logger.debug(f"Synced {len(config)} configuration items to Redis")
        
        return config
    
    elif mode == "shared":
        # 共享模式: 从 Redis 加载
        logger.info("Loading configuration from Redis cache (shared mode)")
        config = await load_from_cache(kv_store, namespace)
        
        if not config:
            logger.warning("No configuration found in Redis cache")
            # 可选: 返回空配置或抛出异常
            # 这里返回空配置，允许后续动态添加服务
            config = {}
        
        logger.debug(f"Loaded {len(config)} configuration items from Redis")
        return config
    
    else:
        raise ValueError(f"Unknown work mode: {mode}")


def load_from_json(json_path: Optional[str]) -> Dict[str, Any]:
    """
    从 JSON 文件加载配置
    
    Args:
        json_path: JSON 文件路径，None 表示使用空配置
    
    Returns:
        配置字典
    
    Raises:
        FileNotFoundError: 如果文件不存在
        json.JSONDecodeError: 如果 JSON 格式无效
    """
    if json_path is None:
        logger.debug("No JSON path provided, returning empty configuration")
        return {}
    
    path = Path(json_path)
    if not path.exists():
        raise FileNotFoundError(f"Configuration file not found: {json_path}")
    
    try:
        with open(path, 'r', encoding='utf-8') as f:
            config = json.load(f)
        
        logger.debug(f"Successfully loaded configuration from {json_path}")
        return config
    
    except json.JSONDecodeError as e:
        logger.error(f"Invalid JSON format in {json_path}: {e}")
        raise
    
    except Exception as e:
        logger.error(f"Failed to load configuration from {json_path}: {e}")
        raise


async def load_from_cache(
    kv_store: 'AsyncKeyValue',
    namespace: Optional[str] = None
) -> Dict[str, Any]:
    """
    从缓存加载配置
    
    Args:
        kv_store: py-key-value 存储实例
        namespace: 命名空间前缀
    
    Returns:
        配置字典
    
    Note:
        配置存储在 collection="config:global" 中
        键名: "mcp_config"
    """
    collection = _get_config_collection(namespace)
    
    try:
        config = await kv_store.get("mcp_config", collection=collection)
        
        if config is None:
            logger.debug("No configuration found in cache")
            return {}
        
        if not isinstance(config, dict):
            logger.warning(f"Invalid configuration type in cache: {type(config)}")
            return {}
        
        return config
    
    except Exception as e:
        logger.error(f"Failed to load configuration from cache: {e}")
        # 不抛出异常，返回空配置
        return {}


async def sync_config_to_cache(
    kv_store: 'AsyncKeyValue',
    config: Dict[str, Any],
    namespace: Optional[str] = None
) -> None:
    """
    同步配置到缓存
    
    Args:
        kv_store: py-key-value 存储实例
        config: 配置字典
        namespace: 命名空间前缀
    
    Note:
        配置存储在 collection="config:global" 中
        键名: "mcp_config"
    """
    collection = _get_config_collection(namespace)
    
    try:
        await kv_store.put("mcp_config", config, collection=collection)
        logger.debug(f"Configuration synced to cache (collection={collection})")
    
    except Exception as e:
        logger.error(f"Failed to sync configuration to cache: {e}")
        raise


async def export_config_to_json(
    kv_store: 'AsyncKeyValue',
    output_path: str,
    namespace: Optional[str] = None,
    pretty: bool = True
) -> None:
    """
    从缓存导出配置到 JSON 文件
    
    Args:
        kv_store: py-key-value 存储实例
        output_path: 输出 JSON 文件路径
        namespace: 命名空间前缀
        pretty: 是否格式化输出（缩进）
    
    Raises:
        RuntimeError: 如果导出失败
    """
    # 从缓存加载配置
    config = await load_from_cache(kv_store, namespace)
    
    if not config:
        logger.warning("No configuration to export")
        config = {}
    
    # 写入 JSON 文件
    try:
        path = Path(output_path)
        path.parent.mkdir(parents=True, exist_ok=True)
        
        with open(path, 'w', encoding='utf-8') as f:
            if pretty:
                json.dump(config, f, indent=2, ensure_ascii=False)
            else:
                json.dump(config, f, ensure_ascii=False)
        
        logger.info(f"Configuration exported to {output_path}")
    
    except Exception as e:
        logger.error(f"Failed to export configuration to {output_path}: {e}")
        raise RuntimeError(f"Configuration export failed: {e}")


def _get_config_collection(namespace: Optional[str] = None) -> str:
    """
    获取配置存储的 Collection 名称
    
    Args:
        namespace: 命名空间前缀
    
    Returns:
        Collection 名称
    
    Examples:
        >>> _get_config_collection(None)
        'config:global'
        
        >>> _get_config_collection("myapp")
        'myapp:config:global'
    """
    if namespace:
        return f"{namespace}:config:global"
    return "config:global"


async def clear_cache_config(
    kv_store: 'AsyncKeyValue',
    namespace: Optional[str] = None
) -> None:
    """
    清除缓存中的配置
    
    Args:
        kv_store: py-key-value 存储实例
        namespace: 命名空间前缀
    
    Note:
        主要用于测试和重置场景
    """
    collection = _get_config_collection(namespace)
    
    try:
        await kv_store.delete("mcp_config", collection=collection)
        logger.debug(f"Configuration cleared from cache (collection={collection})")
    
    except Exception as e:
        logger.warning(f"Failed to clear configuration from cache: {e}")
