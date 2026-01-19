#!/usr/bin/env python3
"""
配置快照生成器

实现配置来源追踪逻辑，区分配置值的来源（默认/TOML/KV/环境变量）
"""

import logging
import sys
from datetime import datetime
from pathlib import Path
from typing import List

import toml

# 添加项目路径
sys.path.insert(0, str(Path(__file__).parent.parent.parent / "src"))

from mcpstore.config.config_defaults import *
from mcpstore.config.toml_config import MCPStoreConfig, get_config
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
            raise ConfigSnapshotError("MCPStoreConfig is not initialized, please call init_config() first")

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
            "health_check.startup_interval": lifecycle_defaults.startup_interval,
            "health_check.startup_timeout": lifecycle_defaults.startup_timeout,
            "health_check.startup_hard_timeout": lifecycle_defaults.startup_hard_timeout,
            "health_check.readiness_interval": lifecycle_defaults.readiness_interval,
            "health_check.readiness_success_threshold": lifecycle_defaults.readiness_success_threshold,
            "health_check.readiness_failure_threshold": lifecycle_defaults.readiness_failure_threshold,
            "health_check.liveness_interval": lifecycle_defaults.liveness_interval,
            "health_check.liveness_failure_threshold": lifecycle_defaults.liveness_failure_threshold,
            "health_check.ping_timeout_http": lifecycle_defaults.ping_timeout_http,
            "health_check.ping_timeout_sse": lifecycle_defaults.ping_timeout_sse,
            "health_check.ping_timeout_stdio": lifecycle_defaults.ping_timeout_stdio,
            "health_check.warning_ping_timeout": lifecycle_defaults.warning_ping_timeout,
            "health_check.window_size": lifecycle_defaults.window_size,
            "health_check.window_min_calls": lifecycle_defaults.window_min_calls,
            "health_check.error_rate_threshold": lifecycle_defaults.error_rate_threshold,
            "health_check.latency_p95_warn": lifecycle_defaults.latency_p95_warn,
            "health_check.latency_p99_critical": lifecycle_defaults.latency_p99_critical,
            "health_check.max_reconnect_attempts": lifecycle_defaults.max_reconnect_attempts,
            "health_check.backoff_base": lifecycle_defaults.backoff_base,
            "health_check.backoff_max": lifecycle_defaults.backoff_max,
            "health_check.backoff_jitter": lifecycle_defaults.backoff_jitter,
            "health_check.backoff_max_duration": lifecycle_defaults.backoff_max_duration,
            "health_check.half_open_max_calls": lifecycle_defaults.half_open_max_calls,
            "health_check.half_open_success_rate_threshold": lifecycle_defaults.half_open_success_rate_threshold,
            "health_check.reconnect_hard_timeout": lifecycle_defaults.reconnect_hard_timeout,
            "health_check.lease_ttl": lifecycle_defaults.lease_ttl,
            "health_check.lease_renew_interval": lifecycle_defaults.lease_renew_interval,
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
            "monitoring.tools_update_hours": monitoring_defaults.tools_update_hours,
            "monitoring.reconnection_seconds": monitoring_defaults.reconnection_seconds,
            "monitoring.cleanup_hours": monitoring_defaults.cleanup_hours,
            "monitoring.enable_tools_update": monitoring_defaults.enable_tools_update,
            "monitoring.enable_reconnection": monitoring_defaults.enable_reconnection,
            "monitoring.update_tools_on_reconnection": monitoring_defaults.update_tools_on_reconnection,
            "monitoring.detect_tools_changes": monitoring_defaults.detect_tools_changes,
            "monitoring.local_service_ping_timeout": monitoring_defaults.local_service_ping_timeout,
            "monitoring.remote_service_ping_timeout": monitoring_defaults.remote_service_ping_timeout,
            "monitoring.enable_adaptive_timeout": monitoring_defaults.enable_adaptive_timeout,
            "monitoring.adaptive_timeout_multiplier": monitoring_defaults.adaptive_timeout_multiplier,
            "monitoring.response_time_history_size": monitoring_defaults.response_time_history_size,
        })

        # 缓存配置默认值（非敏感部分）
        cache_memory_defaults = CacheMemoryConfigDefaults()
        cache_redis_defaults = CacheRedisConfigDefaults()
        defaults.update({
            "cache.memory.timeout": cache_memory_defaults.timeout,
            "cache.memory.retry_attempts": cache_memory_defaults.retry_attempts,
            "cache.memory.health_check": cache_memory_defaults.health_check,
            "cache.memory.max_size": cache_memory_defaults.max_size,
            "cache.memory.cleanup_interval": cache_memory_defaults.cleanup_interval,
            "cache.redis.timeout": cache_redis_defaults.timeout,
            "cache.redis.retry_attempts": cache_redis_defaults.retry_attempts,
            "cache.redis.health_check": cache_redis_defaults.health_check,
            "cache.redis.max_connections": cache_redis_defaults.max_connections,
            "cache.redis.retry_on_timeout": cache_redis_defaults.retry_on_timeout,
            "cache.redis.socket_keepalive": cache_redis_defaults.socket_keepalive,
            "cache.redis.socket_connect_timeout": cache_redis_defaults.socket_connect_timeout,
            "cache.redis.socket_timeout": cache_redis_defaults.socket_timeout,
            "cache.redis.health_check_interval": cache_redis_defaults.health_check_interval,
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
            "standalone.log_format": standalone_defaults.log_format,
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
            logger.warning(f"[CONFIG_SNAPSHOT] [WARN] Failed to load TOML configuration file: {e}")

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
            logger.warning(f"[CONFIG_SNAPSHOT] [WARN] Failed to read KV configuration {kv_key}: {e}")

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
            logger.warning(f"[CONFIG_SNAPSHOT] [WARN] Failed to get dynamic configuration metadata: {e}")
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
        logger.info(f"[CONFIG_SNAPSHOT] [START] Starting to generate configuration snapshot (categories={categories}, pattern={key_pattern})")

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
        logger.info(f"[CONFIG_SNAPSHOT] [COMPLETE] Configuration snapshot generation completed, elapsed {elapsed:.2f}s, contains {snapshot.total_items} configuration items")

        return snapshot
