"""
MCPStore TOML Configuration Management

This module provides unified configuration management for MCPStore using TOML files.
It handles initialization, loading, validation, and provides the MCPStoreConfig class.

Task T1: Configuration directory and default TOML file initialization
Task T2: TOML loading, default value merging, and validation pipeline
"""

import logging
from pathlib import Path
from typing import Optional, Dict, Any, Union, Tuple, List

import toml

from .config_defaults import (
    get_all_defaults,
    ServerConfigDefaults,
    HealthCheckConfigDefaults,
    ContentUpdateConfigDefaults,
    MonitoringConfigDefaults,
    CacheMemoryConfigDefaults,
    CacheRedisConfigDefaults,
    StandaloneConfigDefaults,
    LoggingConfigDefaults,
)
from .path_utils import get_user_data_dir, get_user_config_path

logger = logging.getLogger(__name__)

_server_defaults = ServerConfigDefaults()
_health_defaults = HealthCheckConfigDefaults()
_content_defaults = ContentUpdateConfigDefaults()
_monitoring_defaults = MonitoringConfigDefaults()
_cache_memory_defaults = CacheMemoryConfigDefaults()
_cache_redis_defaults = CacheRedisConfigDefaults()
_standalone_defaults = StandaloneConfigDefaults()
_logging_defaults = LoggingConfigDefaults()

# 尝试导入其他配置类，处理可能的导入失败
try:
    from .config_dataclasses import ServiceLifecycleConfig
except ImportError as e:
    print(f"Warning: ServiceLifecycleConfig not available: {e}")
    ServiceLifecycleConfig = None

try:
    from ..core.configuration.standalone_config import StandaloneConfig
except ImportError as e:
    print(f"Warning: StandaloneConfig not available: {e}")
    StandaloneConfig = None


def ensure_config_directory() -> Path:
    """
    Ensure the configuration directory exists.

    Returns:
        Path: The configuration directory path
    """
    config_dir = get_user_data_dir()
    config_dir.mkdir(parents=True, exist_ok=True)
    return config_dir


def get_default_config_template() -> str:
    """
    Get the default configuration template.

    Returns:
        str: Default TOML configuration template
    """
    defaults = get_all_defaults()

    def _bool(value: bool) -> str:
        return "true" if value else "false"

    def _list(values: List[Any]) -> str:
        parts = []
        for v in values:
            if isinstance(v, str):
                parts.append(f'"{v}"')
            else:
                parts.append(str(v))
        return "[" + ", ".join(parts) + "]"

    server = defaults["server"]
    health = defaults["health_check"]
    content = defaults["content_update"]
    monitoring = defaults["monitoring"]
    standalone = defaults["standalone"]
    # Note: Removed configurations that are not managed via TOML:
    # cache, wrapper, sync, transaction, api, tool_set, logging

    return f'''# =============================================================================
# MCPStore 统一配置文件
# 自动生成，用户可修改
# 描述：统一管理所有非敏感配置项，包含健康检查、监控、日志等配置
#
# 注意：缓存配置由代码参数控制，不在此文件中配置
# 使用示例：MCPStore.setup_store(cache=RedisConfig(url="redis://localhost:6379/0"))

[server]
# API服务器配置
host = "{server["host"]}"
port = {server["port"]}
reload = {_bool(server["reload"])}
auto_open_browser = {_bool(server["auto_open_browser"])}
show_startup_info = {_bool(server["show_startup_info"])}

[health_check]
# 健康检查与熔断配置（新模型，无兼容层）
enabled = {_bool(health["enabled"])}
startup_interval = {health["startup_interval"]}
startup_timeout = {health["startup_timeout"]}
startup_hard_timeout = {health["startup_hard_timeout"]}
readiness_interval = {health["readiness_interval"]}
readiness_success_threshold = {health["readiness_success_threshold"]}
readiness_failure_threshold = {health["readiness_failure_threshold"]}
liveness_interval = {health["liveness_interval"]}
liveness_failure_threshold = {health["liveness_failure_threshold"]}
ping_timeout_http = {health["ping_timeout_http"]}
ping_timeout_sse = {health["ping_timeout_sse"]}
ping_timeout_stdio = {health["ping_timeout_stdio"]}
warning_ping_timeout = {health["warning_ping_timeout"]}
window_size = {health["window_size"]}
window_min_calls = {health["window_min_calls"]}
error_rate_threshold = {health["error_rate_threshold"]}
latency_p95_warn = {health["latency_p95_warn"]}
latency_p99_critical = {health["latency_p99_critical"]}
max_reconnect_attempts = {health["max_reconnect_attempts"]}
backoff_base = {health["backoff_base"]}
backoff_max = {health["backoff_max"]}
backoff_jitter = {health["backoff_jitter"]}
backoff_max_duration = {health["backoff_max_duration"]}
half_open_max_calls = {health["half_open_max_calls"]}
half_open_success_rate_threshold = {health["half_open_success_rate_threshold"]}
reconnect_hard_timeout = {health["reconnect_hard_timeout"]}
lease_ttl = {health["lease_ttl"]}
lease_renew_interval = {health["lease_renew_interval"]}

[content_update]
# 内容更新配置
tools_update_interval = {content["tools_update_interval"]}
resources_update_interval = {content["resources_update_interval"]}
prompts_update_interval = {content["prompts_update_interval"]}
max_concurrent_updates = {content["max_concurrent_updates"]}
update_timeout = {content["update_timeout"]}
max_consecutive_failures = {content["max_consecutive_failures"]}
failure_backoff_multiplier = {content["failure_backoff_multiplier"]}

[monitoring]
# 监控系统配置
tools_update_hours = {monitoring["tools_update_hours"]}
reconnection_seconds = {monitoring["reconnection_seconds"]}
cleanup_hours = {monitoring["cleanup_hours"]}
enable_tools_update = {_bool(monitoring["enable_tools_update"])}
enable_reconnection = {_bool(monitoring["enable_reconnection"])}
update_tools_on_reconnection = {_bool(monitoring["update_tools_on_reconnection"])}
detect_tools_changes = {_bool(monitoring["detect_tools_changes"])}
local_service_ping_timeout = {monitoring["local_service_ping_timeout"]}
remote_service_ping_timeout = {monitoring["remote_service_ping_timeout"]}
enable_adaptive_timeout = {_bool(monitoring["enable_adaptive_timeout"])}
adaptive_timeout_multiplier = {monitoring["adaptive_timeout_multiplier"]}
response_time_history_size = {monitoring["response_time_history_size"]}

[standalone]
# 独立运行模式配置
heartbeat_interval_seconds = {standalone["heartbeat_interval_seconds"]}
http_timeout_seconds = {standalone["http_timeout_seconds"]}
reconnection_interval_seconds = {standalone["reconnection_interval_seconds"]}
cleanup_interval_seconds = {standalone["cleanup_interval_seconds"]}
# streamable_http_endpoint = null  # Not used in default config
default_transport = "{standalone["default_transport"]}"
log_level = "{standalone["log_level"]}"
log_format = "{standalone["log_format"]}"
enable_debug = {_bool(standalone["enable_debug"])}

# =============================================================================
# 以下配置项已移除（不通过TOML管理）：
# - [logging] : 由 setup_store(debug=...) 参数控制
# - [cache] / [cache.memory] / [cache.redis] : 由 setup_store(cache=...) 参数控制
# - [wrapper] : 使用代码中的 WrapperConfigDefaults
# - [sync] : 硬编码在 unified_sync_manager.py
# - [transaction] : 硬编码在 cache_manager.py
# - [api] : 未实际使用
# - [tool_set] : 未实际使用
# =============================================================================
'''


def create_default_config_if_not_exists() -> bool:
    """
    Create default config.toml file if it doesn't exist.

    Returns:
        bool: True if file was created or already exists, False if there was an error
    """
    try:
        # Ensure config directory exists
        config_dir = ensure_config_directory()
        config_path = get_user_config_path()

        if config_path.exists():
            logger.debug(f"Configuration file already exists: {config_path}")
            return True

        # Create default config file
        logger.info(f"Creating default configuration file: {config_path}")

        default_content = get_default_config_template()

        with open(config_path, 'w', encoding='utf-8') as f:
            f.write(default_content)

        logger.info(f"Default configuration file created successfully: {config_path}")
        return True

    except Exception as e:
        logger.error(f"Failed to create default configuration file: {e}")
        return False


def initialize_config_system() -> bool:
    """
    Initialize the configuration system by ensuring directories and files exist.

    This function should be called early in the application startup process.

    Returns:
        bool: True if initialization succeeded, False otherwise
    """
    try:
        logger.info("Initializing MCPStore configuration system...")

        # Ensure configuration directory exists
        config_dir = ensure_config_directory()
        logger.debug(f"Configuration directory ensured: {config_dir}")

        # Create default config.toml if it doesn't exist
        config_created = create_default_config_if_not_exists()
        if not config_created:
            logger.warning("Failed to create default configuration file, continuing...")

        logger.info("MCPStore configuration system initialized successfully")
        return True

    except Exception as e:
        logger.error(f"Failed to initialize configuration system: {e}")
        return False


class ConfigValidator:
    """Configuration validation and processing class for T2."""

    # Validation rules for configuration values
    # Note: Cache configuration removed - managed via setup_store(cache=...) parameter
    VALIDATION_RULES = {
        # Server configuration
        "server.port": {"min": 1000, "max": 65535, "type": int},
        "server.host": {"type": str, "allow_empty": False},
        "server.reload": {"type": bool},
        "server.auto_open_browser": {"type": bool},
        "server.show_startup_info": {"type": bool},

        # Health check configuration
        "health_check.enabled": {"type": bool},
        "health_check.startup_interval": {"min": 0.1, "max": 60.0, "type": float},
        "health_check.startup_timeout": {"min": 1.0, "max": 1800.0, "type": float},
        "health_check.startup_hard_timeout": {"min": 1.0, "max": 7200.0, "type": float},
        "health_check.readiness_interval": {"min": 1.0, "max": 300.0, "type": float},
        "health_check.readiness_success_threshold": {"min": 1, "max": 10, "type": int},
        "health_check.readiness_failure_threshold": {"min": 1, "max": 10, "type": int},
        "health_check.liveness_interval": {"min": 1.0, "max": 300.0, "type": float},
        "health_check.liveness_failure_threshold": {"min": 1, "max": 10, "type": int},
        "health_check.ping_timeout_http": {"min": 0.1, "max": 600.0, "type": float},
        "health_check.ping_timeout_sse": {"min": 0.1, "max": 600.0, "type": float},
        "health_check.ping_timeout_stdio": {"min": 0.1, "max": 1200.0, "type": float},
        "health_check.warning_ping_timeout": {"min": 0.1, "max": 1200.0, "type": float},
        "health_check.window_size": {"min": 1, "max": 1000, "type": int},
        "health_check.window_min_calls": {"min": 1, "max": 1000, "type": int},
        "health_check.error_rate_threshold": {"min": 0.0, "max": 1.0, "type": float},
        "health_check.latency_p95_warn": {"min": 0.01, "max": 30.0, "type": float},
        "health_check.latency_p99_critical": {"min": 0.01, "max": 60.0, "type": float},
        "health_check.max_reconnect_attempts": {"min": 1, "max": 100, "type": int},
        "health_check.backoff_base": {"min": 0.1, "max": 300.0, "type": float},
        "health_check.backoff_max": {"min": 1.0, "max": 3600.0, "type": float},
        "health_check.backoff_jitter": {"min": 0.0, "max": 1.0, "type": float},
        "health_check.backoff_max_duration": {"min": 1.0, "max": 7200.0, "type": float},
        "health_check.half_open_max_calls": {"min": 1, "max": 100, "type": int},
        "health_check.half_open_success_rate_threshold": {"min": 0.0, "max": 1.0, "type": float},
        "health_check.reconnect_hard_timeout": {"min": 1.0, "max": 7200.0, "type": float},
        "health_check.lease_ttl": {"min": 1.0, "max": 3600.0, "type": float},
        "health_check.lease_renew_interval": {"min": 0.5, "max": 3600.0, "type": float},

        # Content update configuration
        "content_update.tools_update_interval": {"min": 10.0, "max": 86400.0, "type": float},
        "content_update.resources_update_interval": {"min": 10.0, "max": 86400.0, "type": float},
        "content_update.prompts_update_interval": {"min": 10.0, "max": 86400.0, "type": float},
        "content_update.max_concurrent_updates": {"min": 1, "max": 20, "type": int},
        "content_update.update_timeout": {"min": 5.0, "max": 600.0, "type": float},
        "content_update.max_consecutive_failures": {"min": 1, "max": 10, "type": int},
        "content_update.failure_backoff_multiplier": {"min": 1.0, "max": 10.0, "type": float},

        # Monitoring configuration
        "monitoring.enable_tools_update": {"type": bool},
        "monitoring.enable_reconnection": {"type": bool},
        "monitoring.update_tools_on_reconnection": {"type": bool},
        "monitoring.detect_tools_changes": {"type": bool},
        "monitoring.enable_adaptive_timeout": {"type": bool},
        "monitoring.tools_update_hours": {"min": 0.1, "max": 168, "type": float},
        "monitoring.reconnection_seconds": {"min": 5, "max": 1800, "type": int},
        "monitoring.cleanup_hours": {"min": 0.1, "max": 168, "type": float},
        "monitoring.local_service_ping_timeout": {"min": 1, "max": 60, "type": int},
        "monitoring.remote_service_ping_timeout": {"min": 1, "max": 120, "type": int},
        "monitoring.adaptive_timeout_multiplier": {"min": 1.0, "max": 5.0, "type": float},
        "monitoring.response_time_history_size": {"min": 5, "max": 100, "type": int},

        # Standalone configuration
        "standalone.heartbeat_interval_seconds": {"min": 1.0, "max": 300.0, "type": float},
        "standalone.http_timeout_seconds": {"min": 1.0, "max": 300.0, "type": float},
        "standalone.reconnection_interval_seconds": {"min": 1.0, "max": 1800.0, "type": float},
        "standalone.cleanup_interval_seconds": {"min": 10.0, "max": 3600.0, "type": float},
        "standalone.default_transport": {"type": str, "allowed_values": ["stdio", "sse", "websocket"]},
        "standalone.enable_debug": {"type": bool},
        "standalone.log_level": {"type": str, "allowed_values": ["DEBUG", "INFO", "DEGRADED", "ERROR"]},
        "standalone.log_format": {"type": str, "allowed_values": ["json", "text"]},

        # Note: Following configurations removed (not managed via TOML):
        # - logging.* : Controlled by setup_store(debug=...) parameter
        # - api.* : Not actually used
        # - wrapper.* : Uses WrapperConfigDefaults in code
        # - sync.* : Hardcoded in unified_sync_manager.py
        # - transaction.* : Hardcoded in cache_manager.py
        # - tool_set.* : Not actually used
    }

    @classmethod
    def validate_config_key(cls, key: str, value: Any) -> Tuple[bool, Any, str]:
        """
        Validate a single configuration key-value pair.

        Args:
            key: Configuration key (e.g., "server.port")
            value: Value to validate

        Returns:
            Tuple of (is_valid, normalized_value, error_message)
        """
        if key not in cls.VALIDATION_RULES:
            # Unknown key, but allow it with a warning
            return True, value, f"Unknown configuration key: {key}"

        rules = cls.VALIDATION_RULES[key]

        # Type validation
        if "type" in rules:
            expected_type = rules["type"]
            try:
                if expected_type == bool and isinstance(value, str):
                    # Allow string representation of boolean
                    normalized_value = value.lower() in ("true", "1", "yes", "on")
                else:
                    normalized_value = expected_type(value)
            except (ValueError, TypeError):
                return False, value, f"Invalid type for {key}: expected {expected_type.__name__}, got {type(value).__name__}"
        else:
            normalized_value = value

        # Range validation
        if "min" in rules and normalized_value < rules["min"]:
            return False, value, f"Value too small for {key}: {normalized_value} < {rules['min']}"

        if "max" in rules and normalized_value > rules["max"]:
            return False, value, f"Value too large for {key}: {normalized_value} > {rules['max']}"

        # Allowed values validation
        if "allowed_values" in rules and normalized_value not in rules["allowed_values"]:
            return False, value, f"Invalid value for {key}: {normalized_value} not in {rules['allowed_values']}"

        # Empty string validation
        if not rules.get("allow_empty", True) and isinstance(normalized_value, str) and not normalized_value.strip():
            return False, value, f"Empty value not allowed for {key}"

        return True, normalized_value, ""

    @classmethod
    def validate_and_fix_config(cls, config: Dict[str, Any], defaults: Dict[str, Any]) -> Tuple[Dict[str, Any], int]:
        """
        Validate configuration and fix invalid values by using defaults.

        Args:
            config: User configuration loaded from TOML
            defaults: Default configuration values

        Returns:
            Tuple of (validated_config, warning_count)
        """
        validated_config = {}
        warning_count = 0

        def process_section(section_config: Dict[str, Any], section_defaults: Dict[str, Any], section_prefix: str = ""):
            nonlocal validated_config, warning_count

            for key, default_value in section_defaults.items():
                full_key = f"{section_prefix}.{key}" if section_prefix else key

                if section_prefix:
                    # Handle nested sections
                    if "." in key:
                        # This shouldn't happen with our structure, but just in case
                        continue
                else:
                    # Top-level section
                    if key in config and isinstance(config[key], dict):
                        # This is a section, process it separately
                        validated_config[key] = {}
                        process_section(config.get(key, {}), default_value, key)
                        continue

            # Process individual key-value pairs
            for key, user_value in section_config.items():
                full_key = f"{section_prefix}.{key}" if section_prefix else key

                if full_key in cls.VALIDATION_RULES:
                    is_valid, normalized_value, error_msg = cls.validate_config_key(full_key, user_value)
                    if is_valid:
                        # Use nested assignment logic
                        if section_prefix:
                            if section_prefix not in validated_config:
                                validated_config[section_prefix] = {}
                            validated_config[section_prefix][key] = normalized_value
                        else:
                            validated_config[key] = normalized_value
                    else:
                        # Use default value and log warning
                        default_value = defaults.get(section_prefix, {}).get(key, section_defaults.get(key))
                        if default_value is not None:
                            if section_prefix:
                                if section_prefix not in validated_config:
                                    validated_config[section_prefix] = {}
                                validated_config[section_prefix][key] = default_value
                            else:
                                validated_config[key] = default_value

                            logger.warning(f"Configuration validation failed for {full_key}: {error_msg}. Using default value: {default_value}")
                            warning_count += 1
                else:
                    # Unknown key, preserve as-is but log warning
                    if section_prefix:
                        if section_prefix not in validated_config:
                            validated_config[section_prefix] = {}
                        validated_config[section_prefix][key] = user_value
                    else:
                        validated_config[key] = user_value

                    logger.warning(f"Unknown configuration key: {full_key}")
                    warning_count += 1

        # Process all sections
        for section_name, section_config in config.items():
            if isinstance(section_config, dict):
                section_defaults = defaults.get(section_name, {})
                process_section(section_config, section_defaults, section_name)
            else:
                # Non-dict top-level value
                validated_config[section_name] = section_config

        # Add missing default values
        def add_missing_defaults(section_name: str, section_defaults: Dict[str, Any]):
            if section_name not in validated_config:
                validated_config[section_name] = {}

            for key, default_value in section_defaults.items():
                if key not in validated_config[section_name]:
                    validated_config[section_name][key] = default_value

        for section_name, section_defaults in defaults.items():
            if isinstance(section_defaults, dict):
                add_missing_defaults(section_name, section_defaults)

        return validated_config, warning_count


def load_toml_config(config_path: Optional[Path] = None) -> Tuple[Dict[str, Any], int]:
    """
    Load and validate TOML configuration with defaults merging.

    Args:
        config_path: Path to the configuration file (uses default if None)

    Returns:
        Tuple of (validated_config, warning_count)

    This function implements T2: TOML loading, default value merging, and validation pipeline.
    """
    if config_path is None:
        config_path = get_user_config_path()

    logger.info(f"Loading configuration from: {config_path}")

    # Get default configuration
    defaults = get_all_defaults()

    # Load user configuration if file exists
    user_config = {}
    if config_path.exists():
        try:
            with open(config_path, 'r', encoding='utf-8') as f:
                user_config = toml.load(f)
            logger.info(f"Successfully loaded TOML configuration from {config_path}")
        except toml.TomlDecodeError as e:
            logger.error(f"Failed to parse TOML configuration from {config_path}: {e}")
            logger.warning("Using default configuration due to TOML parsing error")
            user_config = {}
        except Exception as e:
            logger.error(f"Failed to read configuration file {config_path}: {e}")
            logger.warning("Using default configuration due to file read error")
            user_config = {}
    else:
        logger.warning(f"Configuration file {config_path} does not exist, using defaults")

    # Validate and merge configuration
    validator = ConfigValidator()
    validated_config, warning_count = validator.validate_and_fix_config(user_config, defaults)

    # Apply consistency checks
    validated_config = _apply_consistency_checks(validated_config)

    logger.info(f"Configuration loaded successfully with {warning_count} warnings")
    return validated_config, warning_count


def _apply_consistency_checks(config: Dict[str, Any]) -> Dict[str, Any]:
    """
    Apply logical consistency checks to the configuration.

    Args:
        config: Validated configuration

    Returns:
        Configuration with consistency fixes applied
    """
    # 新模型：不再调整旧的心跳/监控阈值，直接返回配置
    return config


class ConfigFlattener:
    """Configuration flattening class for T3."""

    @staticmethod
    def flatten_config(config: Dict[str, Any], prefix: str = "config") -> Dict[str, Any]:
        """
        Flatten nested configuration into key-value pairs.

        Args:
            config: Nested configuration dictionary
            prefix: Key prefix (default: "config")

        Returns:
            Flattened key-value pairs with format: "prefix.section.key"
        """
        flattened = {}

        def _flatten_recursive(obj: Any, current_path: list[str]) -> None:
            if isinstance(obj, dict):
                for key, value in obj.items():
                    new_path = current_path + [key]
                    _flatten_recursive(value, new_path)
            else:
                # Create the flattened key
                key_path = ".".join([prefix] + current_path)
                flattened[key_path] = obj

        _flatten_recursive(config, [])
        return flattened

    @staticmethod
    def create_config_kv_store(config: Optional[Dict[str, Any]] = None) -> Optional['AsyncKeyValue']:
        """
        Create a dedicated KV store for configuration using memory backend.

        Args:
            config: Configuration for the KV store (optional)

        Returns:
            AsyncKeyValue KV store instance or None if creation fails
        """
        try:
            # Import KV store factory
            from ..core.registry.kv_store_factory import _build_kv_store

            # Default configuration for config KV store
            kv_config = {
                "type": "memory",  # Always use memory backend for config
                "enable_statistics": False,  # No statistics needed for config
                "enable_size_limit": True,
                "max_item_size": 1024 * 1024,  # 1MB per config item
                "enable_compression": False,  # No compression for small config values
            }

            # Override with user-provided config if available
            if config:
                kv_config.update(config)

            logger.info(f"Creating configuration KV store with config: {kv_config}")
            return _build_kv_store(kv_config)

        except Exception as e:
            logger.error(f"Failed to create configuration KV store: {e}")
            return None

    @staticmethod
    async def write_config_to_kv(config: Dict[str, Any], kv_store: 'AsyncKeyValue',
                                 prefix: str = "config") -> bool:
        """
        Write configuration to KV store with flattened keys.

        Args:
            config: Configuration dictionary to write
            kv_store: KV store instance
            prefix: Key prefix (default: "config")

        Returns:
            True if successful, False otherwise
        """
        try:
            # Flatten configuration
            flattened = ConfigFlattener.flatten_config(config, prefix)

            logger.info(f"Writing {len(flattened)} configuration keys to KV store")

            # Write all keys to KV store
            for key, value in flattened.items():
                # Respect py-key-value's expectation that values are dicts
                # by wrapping non-dict values into {"value": actual}.
                # Dict values (if any) are passed through as-is.
                if isinstance(value, dict):
                    store_value = value
                else:
                    store_value = {"value": value}

                await kv_store.put(key, store_value)
                # logger.debug(f"Put config key: {key} = {store_value}")  # Commented out: avoid 80 duplicate logs

            logger.info(f"Successfully wrote {len(flattened)} configuration keys to KV store")
            return True

        except Exception as e:
            logger.error(f"Failed to write configuration to KV store: {e}")
            return False

    @staticmethod
    def write_config_to_kv_sync(config: Dict[str, Any], kv_store: 'AsyncKeyValue',
                                prefix: str = "config") -> bool:
        """
        Write configuration to KV store synchronously.

        Args:
            config: Configuration dictionary to write
            kv_store: KV store instance
            prefix: Key prefix (default: "config")

        Returns:
            True if successful, False otherwise
        """
        try:
            import asyncio

            # Check if we're already in an async context
            try:
                loop = asyncio.get_running_loop()
                if loop.is_running():
                    # We're in an async context, need to create a new loop
                    import concurrent.futures
                    with concurrent.futures.ThreadPoolExecutor() as executor:
                        future = executor.submit(
                            asyncio.run,
                            ConfigFlattener.write_config_to_kv(config, kv_store, prefix)
                        )
                        return future.result()
            except RuntimeError:
                # No running loop, use asyncio.run directly
                pass

            # Not in async context, run directly
            return asyncio.run(ConfigFlattener.write_config_to_kv(config, kv_store, prefix))

        except Exception as e:
            logger.error(f"Failed to write configuration to KV store (sync): {e}")
            return False


def initialize_config_kv_store(config: Optional[Dict[str, Any]] = None) -> Optional['AsyncKeyValue']:
    """
    Initialize the configuration KV store with fallback handling.

    Args:
        config: Validated configuration dictionary (from T2)

    Returns:
        AsyncKeyValue KV store instance or None if initialization fails
    """
    logger.info("Initializing configuration KV store...")

    # Try to create config KV store
    kv_store = ConfigFlattener.create_config_kv_store()

    if kv_store is None:
        logger.error("Failed to create configuration KV store")
        return None

    # Write configuration to KV store
    if config:
        success = ConfigFlattener.write_config_to_kv_sync(config, kv_store)
        if not success:
            logger.warning("Failed to write configuration to KV store, but store was created")
        else:
            logger.info("Configuration KV store initialized successfully")
    else:
        logger.warning("No configuration provided, KV store created but empty")

    return kv_store


def initialize_config_system_with_kv(config_path: Optional[Path] = None) -> Tuple[Dict[str, Any], Optional['AsyncKeyValue'], int]:
    """
    Complete configuration system initialization including KV storage.

    This function implements the full T1-T3 pipeline:
    1. Initialize file system (T1)
    2. Load and validate configuration (T2)
    3. Initialize KV store and write configuration (T3)

    Args:
        config_path: Path to configuration file (optional)

    Returns:
        Tuple of (validated_config, kv_store, warning_count)
    """
    logger.info("Initializing complete configuration system with KV storage...")

    # Step 1: Initialize file system (T1)
    file_system_success = initialize_config_system()
    if not file_system_success:
        logger.warning("File system initialization failed, continuing...")

    # Step 2: Load and validate configuration (T2)
    validated_config, warning_count = load_toml_config(config_path)

    # Step 3: Initialize KV store and write configuration (T3)
    kv_store = initialize_config_kv_store(validated_config)

    if kv_store is None:
        logger.error("Configuration KV store initialization failed")
        return validated_config, None, warning_count

    logger.info(f"Complete configuration system initialized with {warning_count} warnings")
    return validated_config, kv_store, warning_count


# =============================================================================
# T4: MCPStoreConfig Class and Global Access Entry Points
# =============================================================================

import asyncio
from typing import Protocol, runtime_checkable
from dataclasses import dataclass

# Import existing configuration classes for type conversion
try:
    from .config_dataclasses import ServiceLifecycleConfig
except ImportError as e:
    logger.warning(f"ServiceLifecycleConfig could not be imported: {e}")
    ServiceLifecycleConfig = None

try:
    from .cache_config import MemoryConfig, RedisConfig
except ImportError as e:
    logger.warning(f"Cache configuration classes could not be imported: {e}")
    MemoryConfig = None
    RedisConfig = None

try:
    from ..extensions.monitoring.config import MonitoringConfigProcessor
except ImportError as e:
    logger.warning(f"MonitoringConfigProcessor could not be imported: {e}")
    MonitoringConfigProcessor = None

try:
    from ..core.configuration.standalone_config import StandaloneConfig
except ImportError as e:
    logger.warning(f"StandaloneConfig could not be imported: {e}")
    StandaloneConfig = None


@runtime_checkable
class AsyncKeyValue(Protocol):
    """Protocol for AsyncKeyValue to avoid circular imports."""

    async def get(self, key: str, default: Any = None) -> Any:
        """Get value by key."""
        ...

    async def put(self, key: str, value: Any) -> None:
        """Put value by key."""
        ...

    async def delete(self, key: str) -> bool:
        """Delete value by key."""
        ...


@dataclass
class MonitoringConfig:
    """Monitoring configuration dataclass."""
    tools_update_hours: float = _monitoring_defaults.tools_update_hours
    reconnection_seconds: int = _monitoring_defaults.reconnection_seconds
    cleanup_hours: float = _monitoring_defaults.cleanup_hours
    enable_tools_update: bool = _monitoring_defaults.enable_tools_update
    enable_reconnection: bool = _monitoring_defaults.enable_reconnection
    update_tools_on_reconnection: bool = _monitoring_defaults.update_tools_on_reconnection
    detect_tools_changes: bool = _monitoring_defaults.detect_tools_changes
    local_service_ping_timeout: int = _monitoring_defaults.local_service_ping_timeout
    remote_service_ping_timeout: int = _monitoring_defaults.remote_service_ping_timeout
    enable_adaptive_timeout: bool = _monitoring_defaults.enable_adaptive_timeout
    adaptive_timeout_multiplier: float = _monitoring_defaults.adaptive_timeout_multiplier
    response_time_history_size: int = _monitoring_defaults.response_time_history_size


@dataclass
class LoggingConfig:
    """Logging configuration dataclass."""
    level: str = "INFO"
    format: str = "%(asctime)s - %(name)s - %(levelname)s - %(message)s"
    enable_debug: bool = False
    enable_file_logging: bool = True
    log_file_path: str = "logs/mcpstore.log"


@dataclass
class ContentUpdateConfig:
    """Content update configuration dataclass."""
    tools_update_interval: float = _content_defaults.tools_update_interval
    resources_update_interval: float = _content_defaults.resources_update_interval
    prompts_update_interval: float = _content_defaults.prompts_update_interval
    max_concurrent_updates: int = _content_defaults.max_concurrent_updates
    update_timeout: float = _content_defaults.update_timeout
    max_consecutive_failures: int = _content_defaults.max_consecutive_failures
    failure_backoff_multiplier: float = _content_defaults.failure_backoff_multiplier
    enable_auto_update: bool = True
    enable_content_validation: bool = True


class MCPStoreConfig:
    """
    MCPStore Configuration Class - Central configuration access point.

    This class serves as the unified entry point for all configuration operations,
    reading from KV storage and assembling strongly-typed configuration objects.
    """

    def __init__(self, kv_store: AsyncKeyValue, namespace: str = "config"):
        """
        Initialize MCPStoreConfig.

        Args:
            kv_store: AsyncKeyValue store instance containing configuration
            namespace: Configuration namespace prefix (default: "config")
        """
        self._kv = kv_store
        self._namespace = namespace
        self._cache = {}  # Simple in-memory cache for frequently accessed configs
        self._cache_enabled = True

        logger.info(f"MCPStoreConfig initialized with namespace: {namespace}")

    async def _get_config_value(self, key: str, default: Any = None) -> Any:
        """
        Get a configuration value from KV store with optional caching.

        Args:
            key: Configuration key (without namespace prefix)
            default: Default value if key not found

        Returns:
            Configuration value or default
        """
        full_key = f"{self._namespace}.{key}"

        # Check cache first if enabled
        if self._cache_enabled and full_key in self._cache:
            logger.debug(f"Cache hit for config key: {full_key}")
            return self._cache[full_key]

        try:
            # NOTE: py-key-value's get() method does NOT accept a 'default' parameter.
            # It only accepts 'key' (positional) and 'collection' (keyword-only).
            # When a key is not found, it returns None.
            raw_value = await self._kv.get(key=full_key)

            # If key not found, return the default value
            if raw_value is None:
                logger.debug(f"Config key not found: {full_key}, using default: {default}")
                return default

            logger.debug(f"Retrieved config key: {full_key} = {raw_value}")

            # Unwrap values stored as {"value": actual} by the config
            # flattener, while leaving normal dict config objects untouched.
            if isinstance(raw_value, dict) and "value" in raw_value and len(raw_value) == 1:
                value = raw_value["value"]
            else:
                value = raw_value

            # Cache the value if caching is enabled
            if self._cache_enabled:
                self._cache[full_key] = value

            return value

        except Exception as e:
            logger.error(f"Failed to get config value for key {full_key}: {e}")
            return default

    async def get_lifecycle_config(self) -> ServiceLifecycleConfig:
        """
        Get service lifecycle configuration.

        Returns:
            ServiceLifecycleConfig: Lifecycle configuration object
        """
        if ServiceLifecycleConfig is None:
            # Fallback if import failed
            logger.warning("ServiceLifecycleConfig not available, returning dict")
            return {
                "startup_interval": await self._get_config_value("health_check.startup_interval", _health_defaults.startup_interval),
                "startup_timeout": await self._get_config_value("health_check.startup_timeout", _health_defaults.startup_timeout),
                "startup_hard_timeout": await self._get_config_value("health_check.startup_hard_timeout", _health_defaults.startup_hard_timeout),
                "readiness_interval": await self._get_config_value("health_check.readiness_interval", _health_defaults.readiness_interval),
                "readiness_success_threshold": await self._get_config_value("health_check.readiness_success_threshold", _health_defaults.readiness_success_threshold),
                "readiness_failure_threshold": await self._get_config_value("health_check.readiness_failure_threshold", _health_defaults.readiness_failure_threshold),
                "liveness_interval": await self._get_config_value("health_check.liveness_interval", _health_defaults.liveness_interval),
                "liveness_failure_threshold": await self._get_config_value("health_check.liveness_failure_threshold", _health_defaults.liveness_failure_threshold),
                "ping_timeout_http": await self._get_config_value("health_check.ping_timeout_http", _health_defaults.ping_timeout_http),
                "ping_timeout_sse": await self._get_config_value("health_check.ping_timeout_sse", _health_defaults.ping_timeout_sse),
                "ping_timeout_stdio": await self._get_config_value("health_check.ping_timeout_stdio", _health_defaults.ping_timeout_stdio),
                "warning_ping_timeout": await self._get_config_value("health_check.warning_ping_timeout", _health_defaults.warning_ping_timeout),
                "window_size": await self._get_config_value("health_check.window_size", _health_defaults.window_size),
                "window_min_calls": await self._get_config_value("health_check.window_min_calls", _health_defaults.window_min_calls),
                "error_rate_threshold": await self._get_config_value("health_check.error_rate_threshold", _health_defaults.error_rate_threshold),
                "latency_p95_warn": await self._get_config_value("health_check.latency_p95_warn", _health_defaults.latency_p95_warn),
                "latency_p99_critical": await self._get_config_value("health_check.latency_p99_critical", _health_defaults.latency_p99_critical),
                "max_reconnect_attempts": await self._get_config_value("health_check.max_reconnect_attempts", _health_defaults.max_reconnect_attempts),
                "backoff_base": await self._get_config_value("health_check.backoff_base", _health_defaults.backoff_base),
                "backoff_max": await self._get_config_value("health_check.backoff_max", _health_defaults.backoff_max),
                "backoff_jitter": await self._get_config_value("health_check.backoff_jitter", _health_defaults.backoff_jitter),
                "backoff_max_duration": await self._get_config_value("health_check.backoff_max_duration", _health_defaults.backoff_max_duration),
                "half_open_max_calls": await self._get_config_value("health_check.half_open_max_calls", _health_defaults.half_open_max_calls),
                "half_open_success_rate_threshold": await self._get_config_value("health_check.half_open_success_rate_threshold", _health_defaults.half_open_success_rate_threshold),
                "reconnect_hard_timeout": await self._get_config_value("health_check.reconnect_hard_timeout", _health_defaults.reconnect_hard_timeout),
                "lease_ttl": await self._get_config_value("health_check.lease_ttl", _health_defaults.lease_ttl),
                "lease_renew_interval": await self._get_config_value("health_check.lease_renew_interval", _health_defaults.lease_renew_interval),
                "initialization_timeout": await self._get_config_value("health_check.startup_hard_timeout", _health_defaults.startup_hard_timeout),
                "termination_timeout": _service_defaults.termination_timeout,
                "shutdown_timeout": _service_defaults.shutdown_timeout,
            }

        # Create ServiceLifecycleConfig from KV values
        return ServiceLifecycleConfig(
            startup_interval=await self._get_config_value("health_check.startup_interval", _health_defaults.startup_interval),
            startup_timeout=await self._get_config_value("health_check.startup_timeout", _health_defaults.startup_timeout),
            startup_hard_timeout=await self._get_config_value("health_check.startup_hard_timeout", _health_defaults.startup_hard_timeout),
            readiness_interval=await self._get_config_value("health_check.readiness_interval", _health_defaults.readiness_interval),
            readiness_success_threshold=await self._get_config_value("health_check.readiness_success_threshold", _health_defaults.readiness_success_threshold),
            readiness_failure_threshold=await self._get_config_value("health_check.readiness_failure_threshold", _health_defaults.readiness_failure_threshold),
            liveness_interval=await self._get_config_value("health_check.liveness_interval", _health_defaults.liveness_interval),
            liveness_failure_threshold=await self._get_config_value("health_check.liveness_failure_threshold", _health_defaults.liveness_failure_threshold),
            ping_timeout_http=await self._get_config_value("health_check.ping_timeout_http", _health_defaults.ping_timeout_http),
            ping_timeout_sse=await self._get_config_value("health_check.ping_timeout_sse", _health_defaults.ping_timeout_sse),
            ping_timeout_stdio=await self._get_config_value("health_check.ping_timeout_stdio", _health_defaults.ping_timeout_stdio),
            warning_ping_timeout=await self._get_config_value("health_check.warning_ping_timeout", _health_defaults.warning_ping_timeout),
            window_size=await self._get_config_value("health_check.window_size", _health_defaults.window_size),
            window_min_calls=await self._get_config_value("health_check.window_min_calls", _health_defaults.window_min_calls),
            error_rate_threshold=await self._get_config_value("health_check.error_rate_threshold", _health_defaults.error_rate_threshold),
            latency_p95_warn=await self._get_config_value("health_check.latency_p95_warn", _health_defaults.latency_p95_warn),
            latency_p99_critical=await self._get_config_value("health_check.latency_p99_critical", _health_defaults.latency_p99_critical),
            max_reconnect_attempts=await self._get_config_value("health_check.max_reconnect_attempts", _health_defaults.max_reconnect_attempts),
            backoff_base=await self._get_config_value("health_check.backoff_base", _health_defaults.backoff_base),
            backoff_max=await self._get_config_value("health_check.backoff_max", _health_defaults.backoff_max),
            backoff_jitter=await self._get_config_value("health_check.backoff_jitter", _health_defaults.backoff_jitter),
            backoff_max_duration=await self._get_config_value("health_check.backoff_max_duration", _health_defaults.backoff_max_duration),
            half_open_max_calls=await self._get_config_value("health_check.half_open_max_calls", _health_defaults.half_open_max_calls),
            half_open_success_rate_threshold=await self._get_config_value("health_check.half_open_success_rate_threshold", _health_defaults.half_open_success_rate_threshold),
            reconnect_hard_timeout=await self._get_config_value("health_check.reconnect_hard_timeout", _health_defaults.reconnect_hard_timeout),
            lease_ttl=await self._get_config_value("health_check.lease_ttl", _health_defaults.lease_ttl),
            lease_renew_interval=await self._get_config_value("health_check.lease_renew_interval", _health_defaults.lease_renew_interval),
            initialization_timeout=await self._get_config_value("health_check.startup_hard_timeout", _health_defaults.startup_hard_timeout),
            termination_timeout=_service_defaults.termination_timeout,
            shutdown_timeout=_service_defaults.shutdown_timeout,
        )

    async def get_cache_memory_config(self) -> MemoryConfig:
        """
        Get memory cache configuration.

        Returns:
            MemoryConfig: Memory cache configuration object
        """
        if MemoryConfig is None:
            # Fallback if import failed
            logger.warning("MemoryConfig not available, returning dict")
            return {
                "timeout": await self._get_config_value("cache.memory.timeout", 2.0),
                "retry_attempts": await self._get_config_value("cache.memory.retry_attempts", 3),
                "health_check": await self._get_config_value("cache.memory.health_check", True),
                "max_size": await self._get_config_value("cache.memory.max_size", None),
                "cleanup_interval": await self._get_config_value("cache.memory.cleanup_interval", 300),
            }

        return MemoryConfig(
            timeout=await self._get_config_value("cache.memory.timeout", 2.0),
            retry_attempts=await self._get_config_value("cache.memory.retry_attempts", 3),
            health_check=await self._get_config_value("cache.memory.health_check", True),
            max_size=await self._get_config_value("cache.memory.max_size", None),
            cleanup_interval=await self._get_config_value("cache.memory.cleanup_interval", 300),
        )

    async def get_cache_redis_config(self) -> RedisConfig:
        """
        Get Redis cache configuration (non-sensitive parts only).

        Returns:
            RedisConfig: Redis cache configuration object (without sensitive data)
        """
        if RedisConfig is None:
            # Fallback if import failed
            logger.warning("RedisConfig not available, returning dict")
            return {
                "timeout": await self._get_config_value("cache.redis.timeout", 2.0),
                "retry_attempts": await self._get_config_value("cache.redis.retry_attempts", 3),
                "health_check": await self._get_config_value("cache.redis.health_check", True),
                "max_connections": await self._get_config_value("cache.redis.max_connections", 50),
                "retry_on_timeout": await self._get_config_value("cache.redis.retry_on_timeout", True),
                "socket_keepalive": await self._get_config_value("cache.redis.socket_keepalive", True),
                "socket_connect_timeout": await self._get_config_value("cache.redis.socket_connect_timeout", 5.0),
                "socket_timeout": await self._get_config_value("cache.redis.socket_timeout", 5.0),
                "health_check_interval": await self._get_config_value("cache.redis.health_check_interval", 30),
            }

        return RedisConfig(
            timeout=await self._get_config_value("cache.redis.timeout", 2.0),
            retry_attempts=await self._get_config_value("cache.redis.retry_attempts", 3),
            health_check=await self._get_config_value("cache.redis.health_check", True),
            max_connections=await self._get_config_value("cache.redis.max_connections", 50),
            retry_on_timeout=await self._get_config_value("cache.redis.retry_on_timeout", True),
            socket_keepalive=await self._get_config_value("cache.redis.socket_keepalive", True),
            socket_connect_timeout=await self._get_config_value("cache.redis.socket_connect_timeout", 5.0),
            socket_timeout=await self._get_config_value("cache.redis.socket_timeout", 5.0),
            health_check_interval=await self._get_config_value("cache.redis.health_check_interval", 30),
            allow_partial=True,  # Allow partial config for non-sensitive fields only
        )

    async def get_monitoring_config(self) -> MonitoringConfig:
        """
        Get monitoring configuration.

        Returns:
            MonitoringConfig: Monitoring configuration object
        """
        return MonitoringConfig(
            tools_update_hours=await self._get_config_value("monitoring.tools_update_hours", 2),
            reconnection_seconds=await self._get_config_value("monitoring.reconnection_seconds", 60),
            cleanup_hours=await self._get_config_value("monitoring.cleanup_hours", 24),
            enable_tools_update=await self._get_config_value("monitoring.enable_tools_update", True),
            enable_reconnection=await self._get_config_value("monitoring.enable_reconnection", True),
            update_tools_on_reconnection=await self._get_config_value("monitoring.update_tools_on_reconnection", True),
            detect_tools_changes=await self._get_config_value("monitoring.detect_tools_changes", False),
            local_service_ping_timeout=await self._get_config_value("monitoring.local_service_ping_timeout", 3),
            remote_service_ping_timeout=await self._get_config_value("monitoring.remote_service_ping_timeout", 5),
            enable_adaptive_timeout=await self._get_config_value("monitoring.enable_adaptive_timeout", True),
            adaptive_timeout_multiplier=await self._get_config_value("monitoring.adaptive_timeout_multiplier", 2.0),
            response_time_history_size=await self._get_config_value("monitoring.response_time_history_size", 10),
        )

    async def get_content_update_config(self) -> ContentUpdateConfig:
        """
        Get content update configuration.

        Returns:
            ContentUpdateConfig: Content update configuration object
        """
        return ContentUpdateConfig(
            tools_update_interval=await self._get_config_value("content_update.tools_update_interval", 300.0),
            resources_update_interval=await self._get_config_value("content_update.resources_update_interval", 600.0),
            prompts_update_interval=await self._get_config_value("content_update.prompts_update_interval", 600.0),
            max_concurrent_updates=await self._get_config_value("content_update.max_concurrent_updates", 3),
            update_timeout=await self._get_config_value("content_update.update_timeout", 30.0),
            max_consecutive_failures=await self._get_config_value("content_update.max_consecutive_failures", 3),
            failure_backoff_multiplier=await self._get_config_value("content_update.failure_backoff_multiplier", 2.0),
            enable_auto_update=await self._get_config_value("content_update.enable_auto_update", True),
            enable_content_validation=await self._get_config_value("content_update.enable_content_validation", True),
        )

    async def get_logging_config(self) -> LoggingConfig:
        """
        Get logging configuration.

        Returns:
            LoggingConfig: Logging configuration object
        """
        return LoggingConfig(
            level=await self._get_config_value("logging.level", "INFO"),
            format=await self._get_config_value("logging.format", "%(asctime)s - %(name)s - %(levelname)s - %(message)s"),
            enable_debug=await self._get_config_value("logging.enable_debug", False),
            enable_file_logging=await self._get_config_value("logging.enable_file_logging", True),
            log_file_path=await self._get_config_value("logging.log_file_path", "logs/mcpstore.log"),
        )

    async def get_standalone_config(self):
        """
        Get standalone configuration.

        Returns:
            StandaloneConfig or dict: Standalone configuration object
        """
        if StandaloneConfig is None:
            # Fallback if import failed
            logger.warning("StandaloneConfig not available, returning dict")
            return {
                "heartbeat_interval_seconds": await self._get_config_value("standalone.heartbeat_interval_seconds", 60),
                "http_timeout_seconds": await self._get_config_value("standalone.http_timeout_seconds", 30),
                "reconnection_interval_seconds": await self._get_config_value("standalone.reconnection_interval_seconds", 300),
                "cleanup_interval_seconds": await self._get_config_value("standalone.cleanup_interval_seconds", 3600),
                "streamable_http_endpoint": await self._get_config_value("standalone.streamable_http_endpoint", "/mcp"),
                "default_transport": await self._get_config_value("standalone.default_transport", "http"),
                "log_level": await self._get_config_value("standalone.log_level", "INFO"),
                "enable_debug": await self._get_config_value("standalone.enable_debug", False),
            }

        if StandaloneConfig is None:
            # Fallback if import failed
            return {
                "heartbeat_interval_seconds": await self._get_config_value("standalone.heartbeat_interval_seconds", 60),
                "http_timeout_seconds": await self._get_config_value("standalone.http_timeout_seconds", 30),
                "reconnection_interval_seconds": await self._get_config_value("standalone.reconnection_interval_seconds", 300),
                "cleanup_interval_seconds": await self._get_config_value("standalone.cleanup_interval_seconds", 3600),
                "streamable_http_endpoint": await self._get_config_value("standalone.streamable_http_endpoint", "/mcp"),
                "default_transport": await self._get_config_value("standalone.default_transport", "http"),
                "log_level": await self._get_config_value("standalone.log_level", "INFO"),
                "enable_debug": await self._get_config_value("standalone.enable_debug", False),
            }

        return StandaloneConfig(
            heartbeat_interval_seconds=await self._get_config_value("standalone.heartbeat_interval_seconds", 60),
            http_timeout_seconds=await self._get_config_value("standalone.http_timeout_seconds", 30),
            reconnection_interval_seconds=await self._get_config_value("standalone.reconnection_interval_seconds", 300),
            cleanup_interval_seconds=await self._get_config_value("standalone.cleanup_interval_seconds", 3600),
            streamable_http_endpoint=await self._get_config_value("standalone.streamable_http_endpoint", "/mcp"),
            default_transport=await self._get_config_value("standalone.default_transport", "http"),
            log_level=await self._get_config_value("standalone.log_level", "INFO"),
            enable_debug=await self._get_config_value("standalone.enable_debug", False),
        )

    async def get_server_config(self) -> Dict[str, Any]:
        """
        Get API server configuration.

        Returns:
            Dict containing server configuration
        """
        return {
            "host": await self._get_config_value("server.host", "0.0.0.0"),
            "port": await self._get_config_value("server.port", 18200),
            "reload": await self._get_config_value("server.reload", False),
            "auto_open_browser": await self._get_config_value("server.auto_open_browser", False),
            "show_startup_info": await self._get_config_value("server.show_startup_info", True),
            "log_level": await self._get_config_value("server.log_level", "info"),
            "url_prefix": await self._get_config_value("server.url_prefix", ""),
        }

    async def get_tool_set_config_async(self) -> Dict[str, Any]:
        """
        获取工具集配置

        Returns:
            Dict: 工具集配置字典，包含以下键：
                - enable_tool_set: 是否启用工具集管理功能
                - cache_ttl_seconds: 缓存过期时间（秒）
                - max_tools_per_service: 每个服务的最大工具数量
        """
        return {
            "enable_tool_set": await self._get_config_value("tool_set.enable_tool_set", True),
            "cache_ttl_seconds": await self._get_config_value("tool_set.cache_ttl_seconds", 3600),
            "max_tools_per_service": await self._get_config_value("tool_set.max_tools_per_service", 1000),
        }

    async def get_raw_config_section(self, section: str) -> Dict[str, Any]:
        """
        Get a raw configuration section as dictionary.

        Args:
            section: Configuration section name (e.g., "server", "cache")

        Returns:
            Dict containing the configuration section
        """
        section_config = {}
        prefix = f"{self._namespace}.{section}"

        # Get all keys in the section
        try:
            # This is a simplified implementation - in a real scenario,
            # you might want to add a method to list all keys in the KV store
            # For now, we'll construct the section from known keys

            if section == "health_check":
                section_config = {
                    "startup_interval": await self._get_config_value("health_check.startup_interval", _health_defaults.startup_interval),
                    "startup_timeout": await self._get_config_value("health_check.startup_timeout", _health_defaults.startup_timeout),
                    "startup_hard_timeout": await self._get_config_value("health_check.startup_hard_timeout", _health_defaults.startup_hard_timeout),
                    "readiness_interval": await self._get_config_value("health_check.readiness_interval", _health_defaults.readiness_interval),
                    "readiness_success_threshold": await self._get_config_value("health_check.readiness_success_threshold", _health_defaults.readiness_success_threshold),
                    "readiness_failure_threshold": await self._get_config_value("health_check.readiness_failure_threshold", _health_defaults.readiness_failure_threshold),
                    "liveness_interval": await self._get_config_value("health_check.liveness_interval", _health_defaults.liveness_interval),
                    "liveness_failure_threshold": await self._get_config_value("health_check.liveness_failure_threshold", _health_defaults.liveness_failure_threshold),
                    "ping_timeout_http": await self._get_config_value("health_check.ping_timeout_http", _health_defaults.ping_timeout_http),
                    "ping_timeout_sse": await self._get_config_value("health_check.ping_timeout_sse", _health_defaults.ping_timeout_sse),
                    "ping_timeout_stdio": await self._get_config_value("health_check.ping_timeout_stdio", _health_defaults.ping_timeout_stdio),
                    "warning_ping_timeout": await self._get_config_value("health_check.warning_ping_timeout", _health_defaults.warning_ping_timeout),
                    "window_size": await self._get_config_value("health_check.window_size", _health_defaults.window_size),
                    "window_min_calls": await self._get_config_value("health_check.window_min_calls", _health_defaults.window_min_calls),
                    "error_rate_threshold": await self._get_config_value("health_check.error_rate_threshold", _health_defaults.error_rate_threshold),
                    "latency_p95_warn": await self._get_config_value("health_check.latency_p95_warn", _health_defaults.latency_p95_warn),
                    "latency_p99_critical": await self._get_config_value("health_check.latency_p99_critical", _health_defaults.latency_p99_critical),
                    "max_reconnect_attempts": await self._get_config_value("health_check.max_reconnect_attempts", _health_defaults.max_reconnect_attempts),
                    "backoff_base": await self._get_config_value("health_check.backoff_base", _health_defaults.backoff_base),
                    "backoff_max": await self._get_config_value("health_check.backoff_max", _health_defaults.backoff_max),
                    "backoff_jitter": await self._get_config_value("health_check.backoff_jitter", _health_defaults.backoff_jitter),
                    "backoff_max_duration": await self._get_config_value("health_check.backoff_max_duration", _health_defaults.backoff_max_duration),
                    "half_open_max_calls": await self._get_config_value("health_check.half_open_max_calls", _health_defaults.half_open_max_calls),
                    "half_open_success_rate_threshold": await self._get_config_value("health_check.half_open_success_rate_threshold", _health_defaults.half_open_success_rate_threshold),
                    "reconnect_hard_timeout": await self._get_config_value("health_check.reconnect_hard_timeout", _health_defaults.reconnect_hard_timeout),
                    "lease_ttl": await self._get_config_value("health_check.lease_ttl", _health_defaults.lease_ttl),
                    "lease_renew_interval": await self._get_config_value("health_check.lease_renew_interval", _health_defaults.lease_renew_interval),
                }
            elif section == "monitoring":
                section_config = {
                    "tools_update_hours": await self._get_config_value("monitoring.tools_update_hours", 2),
                    "reconnection_seconds": await self._get_config_value("monitoring.reconnection_seconds", 60),
                    "cleanup_hours": await self._get_config_value("monitoring.cleanup_hours", 24),
                    "enable_tools_update": await self._get_config_value("monitoring.enable_tools_update", True),
                    "enable_reconnection": await self._get_config_value("monitoring.enable_reconnection", True),
                    "update_tools_on_reconnection": await self._get_config_value("monitoring.update_tools_on_reconnection", True),
                    "detect_tools_changes": await self._get_config_value("monitoring.detect_tools_changes", False),
                }
            # Add more sections as needed

            logger.debug(f"Retrieved raw config section '{section}' with {len(section_config)} keys")
            return section_config

        except Exception as e:
            logger.error(f"Failed to get raw config section '{section}': {e}")
            return {}

    def clear_cache(self):
        """Clear the internal configuration cache."""
        self._cache.clear()
        logger.debug("Configuration cache cleared")

    def enable_caching(self, enabled: bool = True):
        """
        Enable or disable configuration caching.

        Args:
            enabled: Whether to enable caching (default: True)
        """
        self._cache_enabled = enabled
        if not enabled:
            self._cache.clear()
        logger.info(f"Configuration caching {'enabled' if enabled else 'disabled'}")

    async def get_lifecycle_config_sync(self) -> 'ServiceLifecycleConfig':
        """
        Get service lifecycle configuration (synchronous fallback).

        This method attempts to get lifecycle configuration synchronously.
        If async operations are not available, it falls back to defaults
        but logs the situation for debugging.

        Returns:
            ServiceLifecycleConfig: Lifecycle configuration object
        """
        try:
            # Try async approach first
            return await self.get_lifecycle_config()
        except Exception as e:
            # Fall back to default configuration
            logger.warning(f"Async config retrieval failed: {e}, using default lifecycle config")
            if ServiceLifecycleConfig is None:
                return {}
            return ServiceLifecycleConfig()

    # T11: Configuration snapshot and debugging observability
    async def generate_config_snapshot(self,
                                     categories: Optional[List[str]] = None,
                                     key_pattern: Optional[str] = None,
                                     include_sensitive: bool = True) -> 'ConfigSnapshot':
        """
        生成配置快照

        Args:
            categories: 要包含的配置分类列表
            key_pattern: 键名过滤模式（正则表达式）
            include_sensitive: 是否包含敏感配置

        Returns:
            ConfigSnapshot: 配置快照对象
        """
        from .core.configuration.config_snapshot_generator import ConfigSnapshotGenerator
        generator = ConfigSnapshotGenerator(self)
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
        from .core.configuration.config_export_service import ConfigExportService
        export_service = ConfigExportService()
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
        from .core.configuration.config_export_service import ConfigExportService
        export_service = ConfigExportService()
        return await export_service.get_config_summary()


# Global configuration instance
_global_config: Optional[MCPStoreConfig] = None
_config_lock = asyncio.Lock()


async def init_config(config_path: Optional[Path] = None) -> MCPStoreConfig:
    """
    Initialize the global configuration system.

    This function completes the full T1-T3 pipeline and creates the global MCPStoreConfig instance:
    1. Initialize file system (T1)
    2. Load and validate configuration (T2)
    3. Initialize KV store and write configuration (T3)
    4. Create and return MCPStoreConfig instance (T4)

    Args:
        config_path: Path to configuration file (optional)

    Returns:
        MCPStoreConfig: The global configuration instance

    Raises:
        RuntimeError: If configuration initialization fails
    """
    global _global_config

    async with _config_lock:
        if _global_config is not None:
            logger.info("Configuration already initialized, returning existing instance")
            return _global_config

        logger.info("Initializing global configuration system...")

        # Complete T1-T3 pipeline
        validated_config, kv_store, warning_count = initialize_config_system_with_kv(config_path)

        if kv_store is None:
            raise RuntimeError("Failed to initialize configuration KV store")

        # Create MCPStoreConfig instance (T4)
        _global_config = MCPStoreConfig(kv_store)

        logger.info(f"Global configuration system initialized with {warning_count} warnings")
        return _global_config


def get_config() -> Optional[MCPStoreConfig]:
    """
    Get the global configuration instance.

    Returns:
        MCPStoreConfig: The global configuration instance, or None if not initialized

    Raises:
        RuntimeError: If configuration has not been initialized
    """
    if _global_config is None:
        logger.warning("Configuration not initialized. Call init_config() first.")
        return None

    return _global_config


async def get_config_async() -> Optional[MCPStoreConfig]:
    """
    Get the global configuration instance (async version).

    Returns:
        MCPStoreConfig: The global configuration instance, or None if not initialized
    """
    async with _config_lock:
        return _global_config


def is_config_initialized() -> bool:
    """
    Check if the global configuration has been initialized.

    Returns:
        bool: True if configuration is initialized, False otherwise
    """
    return _global_config is not None


async def shutdown_config():
    """Shutdown the global configuration system."""
    global _global_config

    async with _config_lock:
        if _global_config is not None:
            logger.info("Shutting down global configuration system...")
            _global_config.clear_cache()
            _global_config = None
            logger.info("Global configuration system shutdown complete")


def get_lifecycle_config_with_defaults() -> 'ServiceLifecycleConfig':
    """Get lifecycle configuration.

    This helper attempts to load from MCPStoreConfig. If called in an async context
    or if config is not available, returns default ServiceLifecycleConfig.
    """
    config = get_config()
    if config is None:
        # Config not initialized, return defaults
        logger.warning("MCPStoreConfig not initialized, using default ServiceLifecycleConfig")
        try:
            from mcpstore.config.config_dataclasses import ServiceLifecycleConfig
            return ServiceLifecycleConfig()
        except ImportError:
            logger.error("Cannot import ServiceLifecycleConfig, returning empty dict")
            return {}

    import asyncio
    try:
        loop = asyncio.get_running_loop()
        # In async context, cannot use asyncio.run - return defaults
        logger.warning("Cannot load config in async context, using default ServiceLifecycleConfig")
        try:
            from mcpstore.config.config_dataclasses import ServiceLifecycleConfig
            return ServiceLifecycleConfig()
        except ImportError:
            return {}
    except RuntimeError:
        # No running loop, safe to use asyncio.run
        try:
            return asyncio.run(config.get_lifecycle_config())
        except Exception as e:
            logger.warning(f"Failed to load lifecycle config: {e}, using defaults")
            try:
                from mcpstore.config.config_dataclasses import ServiceLifecycleConfig
                return ServiceLifecycleConfig()
            except ImportError:
                return {}


def get_content_update_config_with_defaults() -> ContentUpdateConfig:
    """Get content update configuration.

    Attempts to load from MCPStoreConfig. Falls back to defaults if in async context
    or if config is unavailable.
    """
    config = get_config()
    if config is None:
        logger.warning("MCPStoreConfig not initialized, using default ContentUpdateConfig")
        return ContentUpdateConfig()

    import asyncio
    try:
        loop = asyncio.get_running_loop()
        # In async context, return defaults
        logger.warning("Cannot load config in async context, using default ContentUpdateConfig")
        return ContentUpdateConfig()
    except RuntimeError:
        # No running loop, safe to use asyncio.run
        try:
            return asyncio.run(config.get_content_update_config())
        except Exception as e:
            logger.warning(f"Failed to load content update config: {e}, using defaults")
            return ContentUpdateConfig()


def get_monitoring_config_with_defaults() -> MonitoringConfig:
    """Get monitoring configuration.

    Attempts to load from MCPStoreConfig. Falls back to defaults if in async context
    or if config is unavailable.
    """
    config = get_config()
    if config is None:
        logger.warning("MCPStoreConfig not initialized, using default MonitoringConfig")
        return MonitoringConfig()

    import asyncio
    try:
        loop = asyncio.get_running_loop()
        logger.warning("Cannot load config in async context, using default MonitoringConfig")
        return MonitoringConfig()
    except RuntimeError:
        try:
            return asyncio.run(config.get_monitoring_config())
        except Exception as e:
            logger.warning(f"Failed to load monitoring config: {e}, using defaults")
            return MonitoringConfig()


def get_cache_memory_config_with_defaults() -> MemoryConfig:
    """Get memory cache configuration.

    Attempts to load from MCPStoreConfig. Falls back to defaults if in async context
    or if config is unavailable.
    """
    config = get_config()
    if config is None:
        logger.warning("MCPStoreConfig not initialized, using default MemoryConfig")
        return MemoryConfig()

    import asyncio
    try:
        loop = asyncio.get_running_loop()
        logger.warning("Cannot load config in async context, using default MemoryConfig")
        return MemoryConfig()
    except RuntimeError:
        try:
            return asyncio.run(config.get_cache_memory_config())
        except Exception as e:
            logger.warning(f"Failed to load memory config: {e}, using defaults")
            return MemoryConfig()


def get_cache_redis_config_with_defaults() -> RedisConfig:
    """Get Redis cache configuration.

    Attempts to load from MCPStoreConfig. Falls back to defaults if in async context
    or if config is unavailable.

    Returns:
        RedisConfig: Redis cache configuration object (non-sensitive fields only)
    """
    config = get_config()
    if config is None:
        logger.warning("MCPStoreConfig not initialized, using default RedisConfig")
        return RedisConfig()

    import asyncio
    try:
        loop = asyncio.get_running_loop()
        logger.warning("Cannot load config in async context, using default RedisConfig")
        return RedisConfig()
    except RuntimeError:
        try:
            return asyncio.run(config.get_cache_redis_config())
        except Exception as e:
            logger.warning(f"Failed to load redis config: {e}, using defaults")
            return RedisConfig()


def get_standalone_config_with_defaults() -> Union[Any, Dict[str, Any]]:
    """Get standalone configuration.

    Attempts to load from MCPStoreConfig. Falls back to empty dict if in async context
    or if config is unavailable.

    Returns:
        StandaloneConfig or dict with standalone configuration values
    """
    config = get_config()
    if config is None:
        logger.warning("MCPStoreConfig not initialized, using empty dict for standalone config")
        return {}

    import asyncio
    try:
        loop = asyncio.get_running_loop()
        # In async context, return empty dict
        logger.warning("Cannot load config in async context, using empty dict for standalone config")
        return {}
    except RuntimeError:
        # No running loop, safe to use asyncio.run
        try:
            return asyncio.run(config.get_standalone_config())
        except Exception as e:
            logger.warning(f"Failed to load standalone config: {e}, using empty dict")
            return {}


def get_server_config_with_defaults() -> Dict[str, Any]:
    """Get API server configuration.

    Attempts to load from MCPStoreConfig. Falls back to empty dict if in async context
    or if config is unavailable.

    Returns:
        Dict with server configuration values
    """
    config = get_config()
    if config is None:
        logger.warning("MCPStoreConfig not initialized, using empty dict for server config")
        return {}

    import asyncio
    try:
        loop = asyncio.get_running_loop()
        logger.warning("Cannot load config in async context, using empty dict for server config")
        return {}
    except RuntimeError:
        # No running loop, safe to use asyncio.run
        try:
            return asyncio.run(config.get_server_config())
        except Exception as e:
            logger.warning(f"Failed to load server config: {e}, using empty dict")
            return {}


# Export the main initialization function and new classes
__all__ = [
    'ensure_config_directory',
    'get_default_config_template',
    'create_default_config_if_not_exists',
    'initialize_config_system',
    'get_user_config_path',
    'ConfigValidator',
    'load_toml_config',
    '_apply_consistency_checks',
    'ConfigFlattener',
    'initialize_config_kv_store',
    'initialize_config_system_with_kv',
    # T4 exports
    'MCPStoreConfig',
    'AsyncKeyValue',
    'MonitoringConfig',
    'LoggingConfig',
    'ContentUpdateConfig',
    'init_config',
    'get_config',
    'get_config_async',
    'is_config_initialized',
    'shutdown_config',
    # T5/T6/T7/T8/T9 helper exports
    'get_lifecycle_config_with_defaults',
    'get_content_update_config_with_defaults',
    'get_monitoring_config_with_defaults',
    'get_cache_memory_config_with_defaults',
    'get_cache_redis_config_with_defaults',
    'get_standalone_config_with_defaults',
    'get_server_config_with_defaults',
]
