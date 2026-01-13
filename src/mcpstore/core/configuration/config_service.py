"""
MCPStore Configuration Service

Provides runtime configuration management capabilities including:
- Dynamic configuration key definitions
- Configuration reading and validation
- Safe configuration updates with TOML persistence
- Configuration metadata and source tracking
"""

import asyncio
import logging
from dataclasses import dataclass, field

# Import TOML libraries
try:
    import tomli
    import tomli_w
except ImportError:
    tomli = None
    tomli_w = None

from datetime import datetime
from enum import Enum
from pathlib import Path
from typing import Any, Dict, List, Optional, Set, Union

from ...config.toml_config import get_config, get_user_config_path

from ...config.config_defaults import (
    HealthCheckConfigDefaults,
    ContentUpdateConfigDefaults,
    MonitoringConfigDefaults,
    CacheMemoryConfigDefaults,
    CacheRedisConfigDefaults,
    StandaloneConfigDefaults,
    ServerConfigDefaults,
)
from .config_snapshot import (
    ConfigSnapshot
)
from .config_snapshot_generator import ConfigSnapshotGenerator
from .config_export_service import ConfigExportService

logger = logging.getLogger(__name__)


class ConfigKeyType(Enum):
    """Configuration key type enumeration."""
    STRING = "string"
    INTEGER = "integer"
    FLOAT = "float"
    BOOLEAN = "boolean"
    LIST = "list"


class ConfigUpdateResult(Enum):
    """Configuration update result."""
    SUCCESS = "success"
    VALIDATION_ERROR = "validation_error"
    READONLY_KEY = "readonly_key"
    PERSISTENCE_ERROR = "persistence_error"
    INTERNAL_ERROR = "internal_error"


@dataclass
class ConfigKeyMetadata:
    """Metadata for a configuration key."""
    key: str
    key_type: ConfigKeyType
    description: str
    default_value: Any
    min_value: Optional[Union[int, float]] = None
    max_value: Optional[Union[int, float]] = None
    allowed_values: Optional[List[Any]] = None
    is_dynamic: bool = True  # False means requires restart
    is_sensitive: bool = False
    category: str = "general"
    requires_restart: bool = False


@dataclass
class ConfigUpdateResponse:
    """Response for configuration update operation."""
    result: ConfigUpdateResult
    key: str
    old_value: Optional[Any] = None
    new_value: Optional[Any] = None
    message: str = ""
    timestamp: datetime = field(default_factory=datetime.now)
    requires_restart: bool = False


@dataclass
class ConfigInfo:
    """Information about a configuration key."""
    key: str
    value: Any
    metadata: ConfigKeyMetadata
    source: str  # "toml", "kv", "default", "environment"
    last_modified: Optional[datetime] = None


class ConfigService:
    """
    Configuration service for runtime configuration management.

    Provides safe configuration updates with validation and persistence.
    """

    def __init__(self):
        """Initialize the configuration service."""
        self._dynamic_keys: Set[str] = set()
        self._key_metadata: Dict[str, ConfigKeyMetadata] = {}
        self._update_lock = asyncio.Lock()
        self._snapshot_generator: Optional[ConfigSnapshotGenerator] = None
        self._export_service: Optional[ConfigExportService] = None
        self._initialize_metadata()

    def _initialize_metadata(self):
        """Initialize configuration key metadata."""
        hc = HealthCheckConfigDefaults()
        # 健康检查/探针
        self._register_key_metadata("health_check.enabled", ConfigKeyType.BOOLEAN, "Enable health checks", hc.enabled, category="health_check")
        self._register_key_metadata("health_check.startup_interval", ConfigKeyType.FLOAT, "Startup probe interval (s)", hc.startup_interval, min_value=0.1, max_value=60.0, category="health_check")
        self._register_key_metadata("health_check.startup_timeout", ConfigKeyType.FLOAT, "Startup timeout (s)", hc.startup_timeout, min_value=1.0, max_value=1800.0, category="health_check")
        self._register_key_metadata("health_check.startup_hard_timeout", ConfigKeyType.FLOAT, "Startup hard timeout (s)", hc.startup_hard_timeout, min_value=1.0, max_value=7200.0, category="health_check")
        self._register_key_metadata("health_check.readiness_interval", ConfigKeyType.FLOAT, "Readiness probe interval (s)", hc.readiness_interval, min_value=1.0, max_value=300.0, category="health_check")
        self._register_key_metadata("health_check.readiness_success_threshold", ConfigKeyType.INTEGER, "Readiness success threshold", hc.readiness_success_threshold, min_value=1, max_value=10, category="health_check")
        self._register_key_metadata("health_check.readiness_failure_threshold", ConfigKeyType.INTEGER, "Readiness failure threshold", hc.readiness_failure_threshold, min_value=1, max_value=10, category="health_check")
        self._register_key_metadata("health_check.liveness_interval", ConfigKeyType.FLOAT, "Liveness probe interval (s)", hc.liveness_interval, min_value=1.0, max_value=300.0, category="health_check")
        self._register_key_metadata("health_check.liveness_failure_threshold", ConfigKeyType.INTEGER, "Liveness failure threshold", hc.liveness_failure_threshold, min_value=1, max_value=10, category="health_check")
        self._register_key_metadata("health_check.ping_timeout_http", ConfigKeyType.FLOAT, "HTTP ping timeout (s)", hc.ping_timeout_http, min_value=0.1, max_value=600.0, category="health_check")
        self._register_key_metadata("health_check.ping_timeout_sse", ConfigKeyType.FLOAT, "SSE ping timeout (s)", hc.ping_timeout_sse, min_value=0.1, max_value=600.0, category="health_check")
        self._register_key_metadata("health_check.ping_timeout_stdio", ConfigKeyType.FLOAT, "STDIO ping timeout (s)", hc.ping_timeout_stdio, min_value=0.1, max_value=1200.0, category="health_check")
        self._register_key_metadata("health_check.warning_ping_timeout", ConfigKeyType.FLOAT, "Relaxed ping timeout for degraded/circuit/half-open (s)", hc.warning_ping_timeout, min_value=0.1, max_value=1200.0, category="health_check")
        # 窗口判定
        self._register_key_metadata("health_check.window_size", ConfigKeyType.INTEGER, "Sliding window size", hc.window_size, min_value=1, max_value=1000, category="health_check")
        self._register_key_metadata("health_check.window_min_calls", ConfigKeyType.INTEGER, "Sliding window min calls", hc.window_min_calls, min_value=1, max_value=1000, category="health_check")
        self._register_key_metadata("health_check.error_rate_threshold", ConfigKeyType.FLOAT, "Error rate threshold", hc.error_rate_threshold, min_value=0.0, max_value=1.0, category="health_check")
        self._register_key_metadata("health_check.latency_p95_warn", ConfigKeyType.FLOAT, "P95 warn threshold (s)", hc.latency_p95_warn, min_value=0.01, max_value=30.0, category="health_check")
        self._register_key_metadata("health_check.latency_p99_critical", ConfigKeyType.FLOAT, "P99 critical threshold (s)", hc.latency_p99_critical, min_value=0.01, max_value=60.0, category="health_check")
        # 退避/熔断/半开
        self._register_key_metadata("health_check.max_reconnect_attempts", ConfigKeyType.INTEGER, "Max reconnect attempts", hc.max_reconnect_attempts, min_value=1, max_value=100, category="health_check")
        self._register_key_metadata("health_check.backoff_base", ConfigKeyType.FLOAT, "Backoff base (s)", hc.backoff_base, min_value=0.1, max_value=300.0, category="health_check")
        self._register_key_metadata("health_check.backoff_max", ConfigKeyType.FLOAT, "Backoff max (s)", hc.backoff_max, min_value=1.0, max_value=3600.0, category="health_check")
        self._register_key_metadata("health_check.backoff_jitter", ConfigKeyType.FLOAT, "Backoff jitter", hc.backoff_jitter, min_value=0.0, max_value=1.0, category="health_check")
        self._register_key_metadata("health_check.backoff_max_duration", ConfigKeyType.FLOAT, "Backoff max duration (s)", hc.backoff_max_duration, min_value=1.0, max_value=7200.0, category="health_check")
        self._register_key_metadata("health_check.half_open_max_calls", ConfigKeyType.INTEGER, "Half-open max probe calls", hc.half_open_max_calls, min_value=1, max_value=100, category="health_check")
        self._register_key_metadata("health_check.half_open_success_rate_threshold", ConfigKeyType.FLOAT, "Half-open success rate threshold", hc.half_open_success_rate_threshold, min_value=0.0, max_value=1.0, category="health_check")
        self._register_key_metadata("health_check.reconnect_hard_timeout", ConfigKeyType.FLOAT, "Reconnect hard timeout (s)", hc.reconnect_hard_timeout, min_value=1.0, max_value=7200.0, category="health_check")
        # 租约
        self._register_key_metadata("health_check.lease_ttl", ConfigKeyType.FLOAT, "Lease TTL (s)", hc.lease_ttl, min_value=1.0, max_value=3600.0, category="health_check")
        self._register_key_metadata("health_check.lease_renew_interval", ConfigKeyType.FLOAT, "Lease renew interval (s)", hc.lease_renew_interval, min_value=0.5, max_value=3600.0, category="health_check")

        # Content update configuration
        self._register_key_metadata(
            "content_update.tools_update_interval",
            ConfigKeyType.FLOAT,
            "Tools update interval in seconds",
            ContentUpdateConfigDefaults().tools_update_interval,
            min_value=60.0, max_value=3600.0,
            category="content_update"
        )
        self._register_key_metadata(
            "content_update.max_concurrent_updates",
            ConfigKeyType.INTEGER,
            "Maximum concurrent updates",
            ContentUpdateConfigDefaults().max_concurrent_updates,
            min_value=1, max_value=10,
            category="content_update"
        )

        # Monitoring configuration
        self._register_key_metadata(
            "monitoring.tools_update_hours",
            ConfigKeyType.FLOAT,
            "Tools update interval in hours",
            MonitoringConfigDefaults().tools_update_hours,
            min_value=0.1, max_value=24.0,
            category="monitoring"
        )
        self._register_key_metadata(
            "monitoring.enable_tools_update",
            ConfigKeyType.BOOLEAN,
            "Enable automatic tools update",
            MonitoringConfigDefaults().enable_tools_update,
            category="monitoring"
        )
        self._register_key_metadata(
            "monitoring.enable_reconnection",
            ConfigKeyType.BOOLEAN,
            "Enable reconnection scheduler",
            MonitoringConfigDefaults().enable_reconnection,
            category="monitoring"
        )
        self._register_key_metadata(
            "monitoring.update_tools_on_reconnection",
            ConfigKeyType.BOOLEAN,
            "Update tools when reconnecting",
            MonitoringConfigDefaults().update_tools_on_reconnection,
            category="monitoring"
        )
        self._register_key_metadata(
            "monitoring.detect_tools_changes",
            ConfigKeyType.BOOLEAN,
            "Detect tools changes",
            MonitoringConfigDefaults().detect_tools_changes,
            category="monitoring"
        )
        self._register_key_metadata(
            "monitoring.reconnection_seconds",
            ConfigKeyType.INTEGER,
            "Reconnection scan interval in seconds",
            MonitoringConfigDefaults().reconnection_seconds,
            min_value=5, max_value=1800,
            category="monitoring"
        )
        self._register_key_metadata(
            "monitoring.cleanup_hours",
            ConfigKeyType.FLOAT,
            "Cleanup interval in hours",
            MonitoringConfigDefaults().cleanup_hours,
            min_value=0.1, max_value=168.0,
            category="monitoring"
        )
        self._register_key_metadata(
            "monitoring.local_service_ping_timeout",
            ConfigKeyType.INTEGER,
            "Local service ping timeout (s)",
            MonitoringConfigDefaults().local_service_ping_timeout,
            min_value=1, max_value=60,
            category="monitoring"
        )
        self._register_key_metadata(
            "monitoring.remote_service_ping_timeout",
            ConfigKeyType.INTEGER,
            "Remote service ping timeout (s)",
            MonitoringConfigDefaults().remote_service_ping_timeout,
            min_value=1, max_value=120,
            category="monitoring"
        )
        self._register_key_metadata(
            "monitoring.enable_adaptive_timeout",
            ConfigKeyType.BOOLEAN,
            "Enable adaptive timeout",
            MonitoringConfigDefaults().enable_adaptive_timeout,
            category="monitoring"
        )
        self._register_key_metadata(
            "monitoring.adaptive_timeout_multiplier",
            ConfigKeyType.FLOAT,
            "Adaptive timeout multiplier",
            MonitoringConfigDefaults().adaptive_timeout_multiplier,
            min_value=1.0, max_value=5.0,
            category="monitoring"
        )
        self._register_key_metadata(
            "monitoring.response_time_history_size",
            ConfigKeyType.INTEGER,
            "Response time history size",
            MonitoringConfigDefaults().response_time_history_size,
            min_value=5, max_value=100,
            category="monitoring"
        )

        # Cache memory configuration
        self._register_key_metadata(
            "cache.memory.timeout",
            ConfigKeyType.FLOAT,
            "Memory cache timeout in seconds",
            CacheMemoryConfigDefaults().timeout,
            min_value=0.1, max_value=60.0,
            category="cache"
        )
        self._register_key_metadata(
            "cache.memory.retry_attempts",
            ConfigKeyType.INTEGER,
            "Memory cache retry attempts",
            CacheMemoryConfigDefaults().retry_attempts,
            min_value=1, max_value=10,
            category="cache"
        )
        self._register_key_metadata(
            "cache.memory.max_size",
            ConfigKeyType.INTEGER,
            "Memory cache maximum size (None for unlimited)",
            CacheMemoryConfigDefaults().max_size,
            min_value=1, max_value=10000,
            category="cache"
        )

        # Cache Redis configuration (non-sensitive only)
        self._register_key_metadata(
            "cache.redis.timeout",
            ConfigKeyType.FLOAT,
            "Redis cache timeout in seconds",
            CacheRedisConfigDefaults().timeout,
            min_value=0.1, max_value=60.0,
            category="cache"
        )
        self._register_key_metadata(
            "cache.redis.retry_attempts",
            ConfigKeyType.INTEGER,
            "Redis cache retry attempts",
            CacheRedisConfigDefaults().retry_attempts,
            min_value=1, max_value=10,
            category="cache"
        )
        self._register_key_metadata(
            "cache.redis.max_connections",
            ConfigKeyType.INTEGER,
            "Redis cache maximum connections",
            CacheRedisConfigDefaults().max_connections,
            min_value=1, max_value=1000,
            category="cache"
        )

        # Standalone configuration
        self._register_key_metadata(
            "standalone.heartbeat_interval_seconds",
            ConfigKeyType.FLOAT,
            "Heartbeat interval in seconds",
            StandaloneConfigDefaults().heartbeat_interval_seconds,
            min_value=1.0, max_value=300.0,
            category="standalone"
        )
        self._register_key_metadata(
            "standalone.http_timeout_seconds",
            ConfigKeyType.FLOAT,
            "HTTP timeout in seconds",
            StandaloneConfigDefaults().http_timeout_seconds,
            min_value=1.0, max_value=300.0,
            category="standalone"
        )
        self._register_key_metadata(
            "standalone.reconnection_interval_seconds",
            ConfigKeyType.FLOAT,
            "Reconnection interval in seconds",
            StandaloneConfigDefaults().reconnection_interval_seconds,
            min_value=1.0, max_value=1800.0,
            category="standalone"
        )
        self._register_key_metadata(
            "standalone.log_level",
            ConfigKeyType.STRING,
            "Log level",
            StandaloneConfigDefaults().log_level,
            allowed_values=["DEBUG", "INFO", "DEGRADED", "ERROR"],
            category="standalone"
        )
        self._register_key_metadata(
            "standalone.enable_debug",
            ConfigKeyType.BOOLEAN,
            "Enable debug mode",
            StandaloneConfigDefaults().enable_debug,
            category="standalone"
        )

        # Server configuration (only dynamic ones)
        self._register_key_metadata(
            "server.reload",
            ConfigKeyType.BOOLEAN,
            "Enable server auto-reload",
            ServerConfigDefaults().reload,
            category="server"
        )
        self._register_key_metadata(
            "server.auto_open_browser",
            ConfigKeyType.BOOLEAN,
            "Auto-open browser on server start",
            ServerConfigDefaults().auto_open_browser,
            category="server"
        )
        self._register_key_metadata(
            "server.show_startup_info",
            ConfigKeyType.BOOLEAN,
            "Show startup information",
            ServerConfigDefaults().show_startup_info,
            category="server"
        )
        # Note: ServerConfigDefaults doesn't have log_level field, so we use a default value
        self._register_key_metadata(
            "server.log_level",
            ConfigKeyType.STRING,
            "Server log level",
            "info",
            allowed_values=["debug", "info", "degraded", "error", "critical"],
            category="server"
        )

        logger.info(f"Initialized {len(self._key_metadata)} configuration keys with {len(self._dynamic_keys)} dynamic keys")

    def _register_key_metadata(
        self,
        key: str,
        key_type: ConfigKeyType,
        description: str,
        default_value: Any,
        min_value: Optional[Union[int, float]] = None,
        max_value: Optional[Union[int, float]] = None,
        allowed_values: Optional[List[Any]] = None,
        is_sensitive: bool = False,
        category: str = "general",
        requires_restart: bool = False
    ):
        """Register metadata for a configuration key."""
        metadata = ConfigKeyMetadata(
            key=key,
            key_type=key_type,
            description=description,
            default_value=default_value,
            min_value=min_value,
            max_value=max_value,
            allowed_values=allowed_values,
            is_dynamic=not requires_restart,  # Dynamic if not requiring restart
            is_sensitive=is_sensitive,
            category=category,
            requires_restart=requires_restart
        )
        self._key_metadata[key] = metadata
        if metadata.is_dynamic:
            self._dynamic_keys.add(key)

    async def get_config_info(self, key: str) -> Optional[ConfigInfo]:
        """
        Get information about a specific configuration key.

        Args:
            key: Configuration key

        Returns:
            ConfigInfo if key exists, None otherwise
        """
        if key not in self._key_metadata:
            return None

        metadata = self._key_metadata[key]

        try:
            config = get_config()
            if config is None:
                # Fallback to default value
                return ConfigInfo(
                    key=key,
                    value=metadata.default_value,
                    metadata=metadata,
                    source="default"
                )

            # Get current value from MCPStoreConfig helper so that
            # wrapped values stored as {"value": actual} in the
            # config KV store are transparently unwrapped.
            value = await config._get_config_value(key, metadata.default_value)

            # Determine source
            source = "default"
            if value != metadata.default_value:
                # Simplified source tracking: anything different from the
                # default is considered coming from KV/TOML.
                source = "kv"  # Could be refined to distinguish TOML vs KV

            return ConfigInfo(
                key=key,
                value=value,
                metadata=metadata,
                source=source
            )
        except Exception as e:
            logger.error(f"Error getting config info for {key}: {e}")
            return ConfigInfo(
                key=key,
                value=metadata.default_value,
                metadata=metadata,
                source="default"
            )

    async def list_configs(self, category: Optional[str] = None) -> List[ConfigInfo]:
        """
        List all configuration keys, optionally filtered by category.

        Args:
            category: Optional category filter

        Returns:
            List of ConfigInfo objects
        """
        configs = []

        for key in self._key_metadata:
            if category and self._key_metadata[key].category != category:
                continue

            config_info = await self.get_config_info(key)
            if config_info:
                configs.append(config_info)

        return configs

    async def update_config(
        self,
        key: str,
        value: Any,
        persist_to_toml: bool = True,
        update_reason: Optional[str] = None
    ) -> ConfigUpdateResponse:
        """
        Update a configuration key.

        Args:
            key: Configuration key to update
            value: New value
            persist_to_toml: Whether to persist the change to TOML file
            update_reason: Optional reason for the update

        Returns:
            ConfigUpdateResponse with operation result
        """
        async with self._update_lock:
            try:
                # Check if key exists
                if key not in self._key_metadata:
                    return ConfigUpdateResponse(
                        result=ConfigUpdateResult.VALIDATION_ERROR,
                        key=key,
                        message=f"Unknown configuration key: {key}"
                    )

                metadata = self._key_metadata[key]

                # Check if key is dynamic
                if not metadata.is_dynamic:
                    return ConfigUpdateResponse(
                        result=ConfigUpdateResult.READONLY_KEY,
                        key=key,
                        message=f"Configuration key '{key}' is not dynamic and requires restart",
                        requires_restart=True
                    )

                # Validate new value
                validation_result = self._validate_value(key, value, metadata)
                if not validation_result.is_valid:
                    return ConfigUpdateResponse(
                        result=ConfigUpdateResult.VALIDATION_ERROR,
                        key=key,
                        message=validation_result.error_message
                    )

                # Get current value
                config = get_config()
                if config is None:
                    return ConfigUpdateResponse(
                        result=ConfigUpdateResult.INTERNAL_ERROR,
                        key=key,
                        message="Configuration system not initialized"
                    )

                # Read current value via MCPStoreConfig helper to get the
                # unwrapped scalar value instead of the underlying
                # {"value": actual} dict stored in the KV backend.
                old_value = await config._get_config_value(key, metadata.default_value)

                # Convert value to appropriate type
                typed_value = self._convert_value_type(value, metadata.key_type)

                # Prepare value for storage: wrap non-dict values into
                # {"value": actual} to satisfy py-key-value wrappers that
                # expect dict[str, Any] as the stored value type.
                if isinstance(typed_value, dict):
                    store_value = typed_value
                else:
                    store_value = {"value": typed_value}

                full_key = f"{config._namespace}.{key}"
                await config._kv.put(full_key, store_value)
                logger.info(f"Updated config {key}: {old_value} -> {typed_value}")

                # Persist to TOML if requested
                if persist_to_toml:
                    persist_result = await self._persist_to_toml(key, typed_value)
                    if not persist_result:
                        # Rollback KV change if TOML persistence failed.
                        # old_value is the unwrapped scalar/object, so we
                        # need to wrap it again when writing back to KV.
                        if isinstance(old_value, dict):
                            rollback_store_value = old_value
                        else:
                            rollback_store_value = {"value": old_value}
                        await config._kv.put(full_key, rollback_store_value)
                        return ConfigUpdateResponse(
                            result=ConfigUpdateResult.PERSISTENCE_ERROR,
                            key=key,
                            old_value=old_value,
                            new_value=typed_value,
                            message="Failed to persist configuration to TOML file"
                        )

                return ConfigUpdateResponse(
                    result=ConfigUpdateResult.SUCCESS,
                    key=key,
                    old_value=old_value,
                    new_value=typed_value,
                    message=f"Successfully updated {key}",
                    requires_restart=metadata.requires_restart
                )

            except Exception as e:
                logger.error(f"Error updating config {key}: {e}")
                return ConfigUpdateResponse(
                    result=ConfigUpdateResult.INTERNAL_ERROR,
                    key=key,
                    message=f"Internal error: {str(e)}"
                )

    async def _persist_to_toml(self, key: str, value: Any) -> bool:
        """
        Persist configuration change to TOML file.

        Args:
            key: Configuration key
            value: New value

        Returns:
            True if successful, False otherwise
        """
        try:
            # Get TOML file path
            config_path = get_user_config_path()
            if not config_path.exists():
                logger.warning(f"Config file {config_path} does not exist, skipping TOML persistence")
                return True  # Not an error, just no file to update

            # Read current TOML content
            if tomli is None:
                logger.error("tomli library not available, cannot read TOML file")
                return False

            with open(config_path, 'rb') as f:
                toml_data = tomli.load(f)

            # Navigate to the appropriate section
            key_parts = key.split('.')
            current_section = toml_data

            # Navigate to parent section
            for part in key_parts[:-1]:
                if part not in current_section:
                    current_section[part] = {}
                current_section = current_section[part]

            # Update the value
            final_key = key_parts[-1]
            current_section[final_key] = value

            # Write back to TOML file with backup
            backup_path = config_path.with_suffix('.toml.backup')

            # Create backup
            if config_path.exists():
                import shutil
                shutil.copy2(config_path, backup_path)

            # Write updated TOML
            if tomli_w is None:
                logger.error("tomli_w library not available, cannot write TOML file")
                return False

            with open(config_path, 'wb') as f:
                tomli_w.dump(toml_data, f)

            logger.info(f"Persisted config {key} to {config_path}")
            return True

        except Exception as e:
            logger.error(f"Failed to persist config {key} to TOML: {e}")
            return False

    def _validate_value(self, key: str, value: Any, metadata: ConfigKeyMetadata) -> "ValidationResult":
        """Validate a configuration value."""
        try:
            # Type validation
            if metadata.key_type == ConfigKeyType.INTEGER:
                int_value = int(value)
                if metadata.min_value is not None and int_value < metadata.min_value:
                    return ValidationResult(False, f"Value {int_value} is below minimum {metadata.min_value}")
                if metadata.max_value is not None and int_value > metadata.max_value:
                    return ValidationResult(False, f"Value {int_value} is above maximum {metadata.max_value}")

            elif metadata.key_type == ConfigKeyType.FLOAT:
                float_value = float(value)
                if metadata.min_value is not None and float_value < metadata.min_value:
                    return ValidationResult(False, f"Value {float_value} is below minimum {metadata.min_value}")
                if metadata.max_value is not None and float_value > metadata.max_value:
                    return ValidationResult(False, f"Value {float_value} is above maximum {metadata.max_value}")

            elif metadata.key_type == ConfigKeyType.BOOLEAN:
                if isinstance(value, str):
                    if value.lower() not in ('true', 'false', '1', '0', 'yes', 'no'):
                        return ValidationResult(False, f"Invalid boolean value: {value}")
                elif not isinstance(value, bool):
                    return ValidationResult(False, f"Expected boolean, got {type(value)}")

            elif metadata.key_type == ConfigKeyType.STRING:
                if not isinstance(value, str):
                    return ValidationResult(False, f"Expected string, got {type(value)}")

                if metadata.allowed_values and value not in metadata.allowed_values:
                    return ValidationResult(False, f"Value '{value}' not in allowed values: {metadata.allowed_values}")

            return ValidationResult(True)
        except Exception as e:
            return ValidationResult(False, f"Validation error: {str(e)}")

    def _convert_value_type(self, value: Any, target_type: ConfigKeyType) -> Any:
        """Convert value to the target type."""
        if target_type == ConfigKeyType.INTEGER:
            return int(value)
        elif target_type == ConfigKeyType.FLOAT:
            return float(value)
        elif target_type == ConfigKeyType.BOOLEAN:
            if isinstance(value, str):
                return value.lower() in ('true', '1', 'yes')
            return bool(value)
        elif target_type == ConfigKeyType.STRING:
            return str(value)
        else:
            return value

    def get_dynamic_keys(self) -> Set[str]:
        """Get all dynamic configuration keys."""
        return self._dynamic_keys.copy()

    def get_categories(self) -> Set[str]:
        """Get all configuration categories."""
        return {metadata.category for metadata in self._key_metadata.values()}

    def _get_snapshot_generator(self) -> ConfigSnapshotGenerator:
        """Get or create snapshot generator."""
        if self._snapshot_generator is None:
            try:
                config = get_config()
                self._snapshot_generator = ConfigSnapshotGenerator(config)
            except Exception as e:
                logger.error(f"Failed to initialize snapshot generator: {e}")
                raise RuntimeError(f"Failed to initialize configuration snapshot generator: {e}")
        return self._snapshot_generator

    def _get_export_service(self) -> ConfigExportService:
        """Get or create export service."""
        if self._export_service is None:
            self._export_service = ConfigExportService()
        return self._export_service

    async def generate_config_snapshot(self,
                                     categories: Optional[List[str]] = None,
                                     key_pattern: Optional[str] = None,
                                     include_sensitive: bool = True) -> ConfigSnapshot:
        """
        生成配置快照

        Args:
            categories: 要包含的配置分类列表
            key_pattern: 键名过滤模式（正则表达式）
            include_sensitive: 是否包含敏感配置

        Returns:
            ConfigSnapshot: 配置快照对象
        """
        generator = self._get_snapshot_generator()
        return await generator.generate_snapshot(
            categories=categories,
            key_pattern=key_pattern,
            include_sensitive=include_sensitive
        )

    async def export_config_snapshot(self,
                                   format: str = "table",
                                   categories: Optional[List[str]] = None,
                                   key_pattern: Optional[str] = None,
                                   include_sensitive: bool = False,
                                   output_file: Optional[Union[str, Path]] = None,
                                   mask_sensitive: bool = True) -> str:
        """
        导出配置快照

        Args:
            format: 输出格式 ("json", "yaml", "table")
            categories: 要包含的配置分类列表
            key_pattern: 键名过滤模式（正则表达式）
            include_sensitive: 是否包含敏感配置
            output_file: 输出文件路径，None 表示返回字符串
            mask_sensitive: 是否屏蔽敏感配置值

        Returns:
            str: 配置快照内容或文件路径
        """
        export_service = self._get_export_service()
        return await export_service.export_config(
            format=format,
            categories=categories,
            key_pattern=key_pattern,
            include_sensitive=include_sensitive,
            output_file=output_file,
            mask_sensitive=mask_sensitive
        )

    async def get_config_summary(self) -> Dict[str, Any]:
        """
        获取配置摘要信息

        Returns:
            Dict[str, Any]: 配置摘要
        """
        export_service = self._get_export_service()
        return await export_service.get_config_summary()

    async def search_config(self,
                           query: str,
                           include_sensitive: bool = False) -> Dict[str, Any]:
        """
        搜索配置项

        Args:
            query: 搜索查询（键名或描述）
            include_sensitive: 是否包含敏感配置

        Returns:
            Dict[str, Any]: 搜索结果
        """
        export_service = self._get_export_service()
        return await export_service.search_config(query, include_sensitive)

    async def validate_config(self) -> Dict[str, Any]:
        """
        验证配置的完整性和一致性

        Returns:
            Dict[str, Any]: 验证结果
        """
        export_service = self._get_export_service()
        return await export_service.validate_config()


@dataclass
class ValidationResult:
    """Result of value validation."""
    is_valid: bool
    error_message: str = ""


# Global configuration service instance
_config_service: Optional[ConfigService] = None


def get_config_service() -> ConfigService:
    """Get the global configuration service instance."""
    global _config_service
    if _config_service is None:
        _config_service = ConfigService()
    return _config_service


