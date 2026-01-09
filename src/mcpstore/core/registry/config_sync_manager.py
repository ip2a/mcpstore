"""
配置同步管理器

负责在不同工作模式下管理配置的同步：
- JSON 到缓存的同步（本地模式、混合模式初始化时）
- 缓存到 JSON 的同步（共享模式导出时）
- 配置变更的增量同步

支持双向同步和冲突检测。
"""

import json
import logging
from datetime import datetime
from pathlib import Path
from typing import Optional, Dict, Any, TYPE_CHECKING

if TYPE_CHECKING:
    from key_value.aio.protocols import AsyncKeyValue

logger = logging.getLogger(__name__)


class ConfigSyncManager:
    """
    配置同步管理器
    
    负责在 JSON 文件和缓存之间同步配置数据。
    支持完整同步和增量同步。
    """
    
    def __init__(
        self,
        kv_store: 'AsyncKeyValue',
        namespace: Optional[str] = None
    ):
        """
        初始化配置同步管理器
        
        Args:
            kv_store: py-key-value 存储实例
            namespace: 命名空间前缀（用于多实例隔离）
        """
        self.kv_store = kv_store
        self.namespace = namespace
        self._last_sync_time: Optional[datetime] = None
    
    async def sync_json_to_cache(
        self,
        json_path: str,
        overwrite: bool = True
    ) -> Dict[str, Any]:
        """
        从 JSON 文件同步配置到缓存
        
        Args:
            json_path: JSON 配置文件路径
            overwrite: 是否覆盖缓存中的现有配置
        
        Returns:
            同步后的配置字典
        
        Raises:
            FileNotFoundError: 如果 JSON 文件不存在
            json.JSONDecodeError: 如果 JSON 格式无效
        
        工作流程:
            1. 从 JSON 文件加载配置
            2. 如果 overwrite=False，合并缓存中的现有配置
            3. 将配置写入缓存
            4. 更新同步时间戳
        """
        logger.info(f"Syncing configuration from JSON to cache: {json_path}")
        
        # 1. 加载 JSON 配置
        json_config = self._load_json_file(json_path)
        
        # 2. 如果不覆盖，合并现有配置
        if not overwrite:
            existing_config = await self._load_cache_config()
            if existing_config:
                logger.debug("Merging with existing cache configuration")
                json_config = self._merge_configs(existing_config, json_config)
        
        # 3. 写入缓存
        await self._save_cache_config(json_config)
        
        # 4. 更新同步时间
        self._last_sync_time = datetime.now()
        await self._save_sync_metadata({
            "last_sync_time": self._last_sync_time.isoformat(),
            "sync_direction": "json_to_cache",
            "source_file": json_path
        })
        
        logger.info(f"Successfully synced {len(json_config)} items from JSON to cache")
        return json_config
    
    async def sync_cache_to_json(
        self,
        output_path: str,
        overwrite: bool = True,
        pretty: bool = True
    ) -> Dict[str, Any]:
        """
        从缓存同步配置到 JSON 文件
        
        Args:
            output_path: 输出 JSON 文件路径
            overwrite: 是否覆盖现有 JSON 文件
            pretty: 是否格式化输出（缩进）
        
        Returns:
            同步后的配置字典
        
        Raises:
            FileExistsError: 如果文件已存在且 overwrite=False
            RuntimeError: 如果写入失败
        
        工作流程:
            1. 从缓存加载配置
            2. 如果 overwrite=False 且文件存在，合并现有 JSON
            3. 将配置写入 JSON 文件
            4. 更新同步时间戳
        """
        logger.info(f"Syncing configuration from cache to JSON: {output_path}")
        
        # 1. 加载缓存配置
        cache_config = await self._load_cache_config()
        
        if not cache_config:
            logger.warning("No configuration found in cache to sync")
            cache_config = {}
        
        # 2. 如果不覆盖且文件存在，合并现有 JSON
        output_file = Path(output_path)
        if not overwrite and output_file.exists():
            logger.debug("Merging with existing JSON file")
            existing_json = self._load_json_file(output_path)
            cache_config = self._merge_configs(existing_json, cache_config)
        
        # 3. 写入 JSON 文件
        self._save_json_file(output_path, cache_config, pretty)
        
        # 4. 更新同步时间
        self._last_sync_time = datetime.now()
        await self._save_sync_metadata({
            "last_sync_time": self._last_sync_time.isoformat(),
            "sync_direction": "cache_to_json",
            "output_file": output_path
        })
        
        logger.info(f"Successfully synced {len(cache_config)} items from cache to JSON")
        return cache_config
    
    async def get_sync_status(self) -> Dict[str, Any]:
        """
        获取同步状态信息
        
        Returns:
            同步状态字典，包含最后同步时间、方向等信息
        """
        metadata = await self._load_sync_metadata()
        
        return {
            "last_sync_time": metadata.get("last_sync_time"),
            "sync_direction": metadata.get("sync_direction"),
            "source_file": metadata.get("source_file"),
            "output_file": metadata.get("output_file"),
            "namespace": self.namespace
        }
    
    async def clear_sync_metadata(self) -> None:
        """
        清除同步元数据
        
        主要用于测试和重置场景
        """
        collection = self._get_metadata_collection()
        
        try:
            await self.kv_store.delete("sync_metadata", collection=collection)
            self._last_sync_time = None
            logger.debug("Sync metadata cleared")
        
        except Exception as e:
            logger.warning(f"Failed to clear sync metadata: {e}")
    
    # ========== 私有辅助方法 ==========
    
    def _load_json_file(self, json_path: str) -> Dict[str, Any]:
        """从 JSON 文件加载配置"""
        path = Path(json_path)
        
        if not path.exists():
            raise FileNotFoundError(f"Configuration file not found: {json_path}")
        
        try:
            with open(path, 'r', encoding='utf-8') as f:
                config = json.load(f)
            
            if not isinstance(config, dict):
                logger.warning(f"JSON file does not contain a dictionary: {json_path}")
                return {}
            
            return config
        
        except json.JSONDecodeError as e:
            logger.error(f"Invalid JSON format in {json_path}: {e}")
            raise
        
        except Exception as e:
            logger.error(f"Failed to load JSON file {json_path}: {e}")
            raise
    
    def _save_json_file(
        self,
        json_path: str,
        config: Dict[str, Any],
        pretty: bool = True
    ) -> None:
        """保存配置到 JSON 文件"""
        path = Path(json_path)
        
        try:
            # 确保目录存在
            path.parent.mkdir(parents=True, exist_ok=True)
            
            # 写入文件
            with open(path, 'w', encoding='utf-8') as f:
                if pretty:
                    json.dump(config, f, indent=2, ensure_ascii=False)
                else:
                    json.dump(config, f, ensure_ascii=False)
            
            logger.debug(f"Configuration saved to {json_path}")
        
        except Exception as e:
            logger.error(f"Failed to save JSON file {json_path}: {e}")
            raise RuntimeError(f"Failed to save configuration: {e}")
    
    async def _load_cache_config(self) -> Dict[str, Any]:
        """从缓存加载配置"""
        collection = self._get_config_collection()
        
        try:
            config = await self.kv_store.get("mcp_config", collection=collection)
            
            if config is None:
                return {}
            
            if not isinstance(config, dict):
                logger.warning(f"Invalid configuration type in cache: {type(config)}")
                return {}
            
            return config
        
        except Exception as e:
            logger.error(f"Failed to load configuration from cache: {e}")
            return {}
    
    async def _save_cache_config(self, config: Dict[str, Any]) -> None:
        """保存配置到缓存"""
        collection = self._get_config_collection()
        
        try:
            await self.kv_store.put("mcp_config", config, collection=collection)
            logger.debug(f"Configuration saved to cache (collection={collection})")
        
        except Exception as e:
            logger.error(f"Failed to save configuration to cache: {e}")
            raise
    
    async def _load_sync_metadata(self) -> Dict[str, Any]:
        """加载同步元数据"""
        collection = self._get_metadata_collection()
        
        try:
            metadata = await self.kv_store.get("sync_metadata", collection=collection)
            
            if metadata is None:
                return {}
            
            if not isinstance(metadata, dict):
                return {}
            
            return metadata
        
        except Exception as e:
            logger.debug(f"Failed to load sync metadata: {e}")
            return {}
    
    async def _save_sync_metadata(self, metadata: Dict[str, Any]) -> None:
        """保存同步元数据"""
        collection = self._get_metadata_collection()
        
        try:
            await self.kv_store.put("sync_metadata", metadata, collection=collection)
        
        except Exception as e:
            logger.warning(f"Failed to save sync metadata: {e}")
    
    def _merge_configs(
        self,
        base: Dict[str, Any],
        update: Dict[str, Any]
    ) -> Dict[str, Any]:
        """
        合并两个配置字典
        
        Args:
            base: 基础配置（优先级低）
            update: 更新配置（优先级高）
        
        Returns:
            合并后的配置
        
        Note:
            使用深度合并策略，update 中的值会覆盖 base 中的值
        """
        result = base.copy()
        
        for key, value in update.items():
            if key in result and isinstance(result[key], dict) and isinstance(value, dict):
                # 递归合并嵌套字典
                result[key] = self._merge_configs(result[key], value)
            else:
                # 直接覆盖
                result[key] = value
        
        return result
    
    def _get_config_collection(self) -> str:
        """获取配置存储的 Collection 名称"""
        if self.namespace:
            return f"{self.namespace}:config:global"
        return "config:global"
    
    def _get_metadata_collection(self) -> str:
        """获取元数据存储的 Collection 名称"""
        if self.namespace:
            return f"{self.namespace}:config:metadata"
        return "config:metadata"
