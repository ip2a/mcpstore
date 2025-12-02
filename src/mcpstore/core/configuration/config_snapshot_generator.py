#!/usr/bin/env python3
"""
配置快照生成器

实现配置来源追踪逻辑，区分配置值的来源（默认/TOML/KV/环境变量）
"""

import asyncio
import logging
import os
import sys
import toml
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Any, Dict, List, Optional, Set, Tuple

# 添加项目路径
sys.path.insert(0, str(Path(__file__).parent.parent.parent / "src"))

from mcpstore.config.config_defaults import *
from mcpstore.config.toml_config import MCPStoreConfig, get_config
from mcpstore.config.cache_environment import get_sensitive_redis_config, get_cache_type_from_env
from mcpstore.core.configuration.config_snapshot import (
    ConfigSnapshot, ConfigGroupSnapshot, ConfigItemSnapshot, ConfigSource,
    ConfigSnapshotError
)
# 避免循环导入，使用延迟导入

logger = logging.getLogger(__name__)


@dataclass
class ConfigTraceResult:
    """配置追踪结果"""
    value: Any
    source: ConfigSource
    original_value: Any = None  # 原始值（用于类型转换前）


class ConfigSnapshotGenerator:
    """配置快照生成器"""

    def __init__(self, config: Optional[MCPStoreConfig] = None):
        """
        初始化配置快照生成器

        Args:
            config: MCPStoreConfig 实例，如果为 None 则使用全局配置
        """
        self.config = config or get_config()
        if not self.config:
            raise ConfigSnapshotError("MCPStoreConfig 未初始化，请先调用 init_config()")

        # 缓存默认值以避免重复计算
        self._default_values_cache: Optional[Dict[str, Any]] = None
        self._toml_values_cache: Optional[Dict[str, Any]] = None

        # 敏感配置键模式
        self._sensitive_patterns = {
            "password", "secret", "token", "key", "auth", "credential",
            "redis_url", "database_url", "connection_string"
        }

        # 配置分类定义
        self._category_mappings = {
            "health_check": {
                "prefix": "health_check.",
                "description": "健康检查配置"
            },
            "content_update": {
                "prefix": "content_update.",
                "description": "内容更新配置"
            },
            "monitoring": {
                "prefix": "monitoring.",
                "description": "监控配置"
            },
            "cache": {
                "prefix": "cache.",
                "description": "缓存配置"
            },
            "standalone": {
                "prefix": "standalone.",
                "description": "独立应用配置"
            },
            "server": {
                "prefix": "server.",
                "description": "API 服务器配置"
            }
        }

    def _is_sensitive_key(self, key: str) -> bool:
        """判断配置键是否为敏感配置"""
        key_lower = key.lower()
        return any(pattern in key_lower for pattern in self._sensitive_patterns)

    def _get_category_for_key(self, key: str) -> str:
        """根据键名确定配置分类"""
        for category, config in self._category_mappings.items():
            if key.startswith(config["prefix"]):
                return category
        return "other"

    def _get_default_values(self) -> Dict[str, Any]:
        """获取所有配置的默认值"""
        if self._default_values_cache is None:
            self._default_values_cache = self._compute_default_values()
        return self._default_values_cache

    def _compute_default_values(self) -> Dict[str, Any]:
        """计算所有配置的默认值"""
        defaults = {}

        # 生命周期与健康检查默认值
        lifecycle_defaults = HealthCheckConfigDefaults()
        defaults.update({
            "health_check.enabled": lifecycle_defaults.enabled,
            "health_check.check_interval_seconds": lifecycle_defaults.check_interval_seconds,
            "health_check.failure_threshold": lifecycle_defaults.failure_threshold,
            "health_check.warning_failure_threshold": lifecycle_defaults.warning_failure_threshold,
            "health_check.termination_failure_threshold": lifecycle_defaults.termination_failure_threshold,
            "health_check.initialization_timeout_seconds": lifecycle_defaults.initialization_timeout_seconds,
            "health_check.shutdown_timeout_seconds": lifecycle_defaults.shutdown_timeout_seconds,
            "health_check.restart_delay_seconds": lifecycle_defaults.restart_delay_seconds,
            "health_check.health_check_timeout_seconds": lifecycle_defaults.health_check_timeout_seconds,
            "health_check.enableDetailedLogging": lifecycle_defaults.enableDetailedLogging,
            "health_check.collectStartupMetrics": lifecycle_defaults.collectStartupMetrics,
            "health_check.collectRuntimeMetrics": lifecycle_defaults.collectRuntimeMetrics,
            "health_check.collectShutdownMetrics": lifecycle_defaults.collectShutdownMetrics,
        })

        # 内容更新默认值
        content_defaults = ContentUpdateConfigDefaults()
        defaults.update({
            "content_update.enabled": content_defaults.enabled,
            "content_update.tools_update_interval": content_defaults.tools_update_interval,
            "content_update.services_update_interval": content_defaults.services_update_interval,
            "content_update.failure_threshold": content_defaults.failure_threshold,
            "content_update.max_retry_attempts": content_defaults.max_retry_attempts,
            "content_update.retry_delay_seconds": content_defaults.retry_delay_seconds,
            "content_update.enable_detailed_logging": content_defaults.enable_detailed_logging,
        })

        # 监控配置默认值
        monitoring_defaults = MonitoringConfigDefaults()
        defaults.update({
            "monitoring.enabled": monitoring_defaults.enabled,
            "monitoring.health_check_seconds": monitoring_defaults.health_check_seconds,
            "monitoring.metrics_collection_seconds": monitoring_defaults.metrics_collection_seconds,
            "monitoring.statistics_retention_hours": monitoring_defaults.statistics_retention_hours,
            "monitoring.max_statistics_memory_mb": monitoring_defaults.max_statistics_memory_mb,
            "monitoring.enable_performance_monitoring": monitoring_defaults.enable_performance_monitoring,
            "monitoring.enable_tools_update": monitoring_defaults.enable_tools_update,
            "monitoring.slow_query_threshold_seconds": monitoring_defaults.slow_query_threshold_seconds,
            "monitoring.memory_usage_warning_threshold": monitoring_defaults.memory_usage_warning_threshold,
            "monitoring.cpu_usage_warning_threshold": monitoring_defaults.cpu_usage_warning_threshold,
            "monitoring.enable_detailed_logging": monitoring_defaults.enable_detailed_logging,
            "monitoring.log_slow_operations": monitoring_defaults.log_slow_operations,
            "monitoring.export_format": monitoring_defaults.export_format,
        })

        # 缓存配置默认值（非敏感部分）
        cache_defaults = CacheConfigDefaults()
        defaults.update({
            "cache.type": "memory",  # 默认内存缓存
            "cache.memory.max_size": cache_defaults.memory.max_size,
            "cache.memory.ttl_seconds": cache_defaults.memory.ttl_seconds,
            "cache.memory.cleanup_interval_seconds": cache_defaults.memory.cleanup_interval_seconds,
            "cache.redis.max_connections": cache_defaults.redis.max_connections,
            "cache.redis.socket_timeout_seconds": cache_defaults.redis.socket_timeout_seconds,
            "cache.redis.socket_connect_timeout_seconds": cache_defaults.redis.socket_connect_timeout_seconds,
            "cache.redis.health_check_interval_seconds": cache_defaults.redis.health_check_interval_seconds,
            "cache.redis.max_retries": cache_defaults.redis.max_retries,
            "cache.redis.retry_delay_seconds": cache_defaults.redis.retry_delay_seconds,
        })

        # 独立应用配置默认值
        standalone_defaults = StandaloneConfigDefaults()
        defaults.update({
            "standalone.heartbeat_interval_seconds": standalone_defaults.heartbeat_interval_seconds,
            "standalone.http_timeout_seconds": standalone_defaults.http_timeout_seconds,
            "standalone.reconnection_interval_seconds": standalone_defaults.reconnection_interval_seconds,
            "standalone.cleanup_interval_seconds": standalone_defaults.cleanup_interval_seconds,
            "standalone.streamable_http_endpoint": standalone_defaults.streamable_http_endpoint,
            "standalone.default_transport": standalone_defaults.default_transport,
            "standalone.log_level": standalone_defaults.log_level,
            "standalone.enable_debug": standalone_defaults.enable_debug,
        })

        # 服务器配置默认值
        defaults.update({
            "server.host": "0.0.0.0",
            "server.port": 18200,
            "server.reload": False,
            "server.auto_open_browser": False,
            "server.show_startup_info": True,
            "server.log_level": "info",
            "server.url_prefix": "",
        })

        return defaults

    async def _get_toml_values(self) -> Dict[str, Any]:
        """获取 TOML 文件中的配置值"""
        if self._toml_values_cache is None:
            self._toml_values_cache = await self._load_toml_values()
        return self._toml_values_cache

    async def _load_toml_values(self) -> Dict[str, Any]:
        """从 TOML 文件加载配置值"""
        toml_values = {}

        try:
            config_path = Path.home() / ".mcpstore" / "config.toml"
            if config_path.exists():
                with open(config_path, 'r', encoding='utf-8') as f:
                    toml_data = toml.load(f)

                # 扁平化 TOML 数据
                toml_values = self._flatten_dict(toml_data)
        except Exception as e:
            logger.warning(f"加载 TOML 配置文件失败: {e}")

        return toml_values

    def _flatten_dict(self, data: Dict[str, Any], prefix: str = "", separator: str = ".") -> Dict[str, Any]:
        """扁平化嵌套字典"""
        result = {}

        for key, value in data.items():
            full_key = f"{prefix}{separator}{key}" if prefix else key

            if isinstance(value, dict):
                result.update(self._flatten_dict(value, full_key, separator))
            else:
                result[full_key] = value

        return result

    async def _trace_config_value(self, key: str, default_value: Any) -> ConfigTraceResult:
        """
        追踪配置值的来源

        优先级：KV 存储 > TOML 文件 > 默认值
        """
        # 1. 检查 KV 存储
        kv_key = f"config.{key}"
        try:
            kv_value = await self.config._kv.get(kv_key)
            if kv_value is not None:
                return ConfigTraceResult(
                    value=kv_value,
                    source=ConfigSource.KV,
                    original_value=kv_value
                )
        except Exception as e:
            logger.warning(f"读取 KV 配置失败 {kv_key}: {e}")

        # 2. 检查 TOML 文件
        toml_values = await self._get_toml_values()
        if key in toml_values:
            return ConfigTraceResult(
                value=toml_values[key],
                source=ConfigSource.TOML,
                original_value=toml_values[key]
            )

        # 3. 使用默认值
        return ConfigTraceResult(
            value=default_value,
            source=ConfigSource.DEFAULT,
            original_value=default_value
        )

    async def _get_dynamic_keys_metadata(self) -> Dict[str, Dict[str, Any]]:
        """获取动态配置键的元数据"""
        try:
            # 延迟导入避免循环依赖
            from mcpstore.core.configuration.config_service import get_config_service
            config_service = get_config_service()
            return config_service.get_all_metadata()
        except Exception as e:
            logger.warning(f"获取动态配置元数据失败: {e}")
            return {}

    async def generate_snapshot(self,
                              categories: Optional[List[str]] = None,
                              key_pattern: Optional[str] = None,
                              include_sensitive: bool = True) -> ConfigSnapshot:
        """
        生成配置快照

        Args:
            categories: 要包含的配置分类，None 表示包含所有
            key_pattern: 键名过滤模式（正则表达式）
            include_sensitive: 是否包含敏感配置

        Returns:
            ConfigSnapshot: 配置快照对象
        """
        import re

        start_time = datetime.now()
        logger.info(f"开始生成配置快照 (categories={categories}, pattern={key_pattern})")

        # 获取所有默认值
        default_values = self._get_default_values()
        dynamic_metadata = await self._get_dynamic_keys_metadata()

        # 收集配置项
        all_items = []

        for key, default_value in default_values.items():
            # 应用分类过滤
            category = self._get_category_for_key(key)
            if categories and category not in categories:
                continue

            # 应用键名模式过滤
            if key_pattern and not re.search(key_pattern, key, re.IGNORECASE):
                continue

            # 追踪配置值来源
            trace_result = await self._trace_config_value(key, default_value)

            # 获取元数据
            metadata = dynamic_metadata.get(key, {})
            is_dynamic = metadata.get("is_dynamic", False)
            description = metadata.get("description")
            validation_info = metadata.get("validation_info")

            # 检查是否为敏感配置
            is_sensitive = self._is_sensitive_key(key) or metadata.get("is_sensitive", False)

            # 如果不包含敏感配置且当前是敏感配置，则跳过
            if not include_sensitive and is_sensitive:
                continue

            # 创建配置项快照
            item = ConfigItemSnapshot(
                key=key,
                value=trace_result.value,
                source=trace_result.source,
                category=category,
                is_sensitive=is_sensitive,
                is_dynamic=is_dynamic,
                description=description,
                validation_info=validation_info
            )

            all_items.append(item)

        # 按分类分组
        groups_dict = {}
        for item in all_items:
            if item.category not in groups_dict:
                groups_dict[item.category] = []
            groups_dict[item.category].append(item)

        # 创建配置组快照
        groups = {}
        for category, items in groups_dict.items():
            groups[category] = ConfigGroupSnapshot(
                name=category,
                items=items
            )

        # 创建完整快照
        snapshot = ConfigSnapshot(
            timestamp=start_time,
            groups=groups
        )

        elapsed = (datetime.now() - start_time).total_seconds()
        logger.info(f"配置快照生成完成，耗时 {elapsed:.2f}s，包含 {snapshot.total_items} 项配置")

        return snapshot
