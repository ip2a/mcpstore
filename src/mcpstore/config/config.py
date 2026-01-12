"""
Optimized configuration module
Remove sys.path operations to improve import performance
"""
import logging
from typing import Dict, Any, Union

from .config_defaults import StandaloneConfigDefaults
from .toml_config import get_standalone_config_with_defaults

# Remove sys.path.append() operations to improve import performance
# If you need to import other modules, please use relative imports or correct package structure

# Define custom DEGRADED log level (between INFO and WARNING)
DEGRADED = 25  # INFO=20, WARNING=30
logging.addLevelName(DEGRADED, "DEGRADED")

logger = logging.getLogger(__name__)

_standalone_defaults = StandaloneConfigDefaults()

class LoggingConfig:
    """Logging configuration manager"""

    _debug_enabled = False
    _configured = False
    _current_level: int = DEGRADED

    @classmethod
    def setup_logging(cls, debug: Union[bool, str, int] = False, force_reconfigure: bool = False):
        """
        Setup logging configuration.

        Args:
            debug: Backward-compatible log control. Supports:
                   - True  -> DEBUG
                   - False -> DEGRADED (was ERROR before; now more practical)
                   - "DEBUG"/"INFO"/"DEGRADED"/"ERROR"/"CRITICAL" -> exact level
                   - int   -> logging level constant
            force_reconfigure: Whether to force reconfiguration
        """
        def _to_level(v: Union[bool, str, int]) -> int:
            if isinstance(v, bool):
                # False means fully mute logs by setting an OFF-level above CRITICAL
                return logging.DEBUG if v else (logging.CRITICAL + 50)
            if isinstance(v, int):
                return v
            if isinstance(v, str):
                m = v.strip().upper()
                return {
                    "DEBUG": logging.DEBUG,
                    "INFO": logging.INFO,
                    "DEGRADED": DEGRADED,
                    "ERROR": logging.ERROR,
                    "CRITICAL": logging.CRITICAL,
                }.get(m, DEGRADED)
            return DEGRADED

        level = _to_level(debug)

        if cls._configured and not force_reconfigure:
            # Only update levels if changed
            if level != cls._current_level:
                cls._set_log_level(level)
            return

        # Configure log format
        if level <= logging.DEBUG:
            log_format = '%(asctime)s - %(name)s - %(levelname)s - %(message)s'
        else:
            log_format = '%(levelname)s - %(message)s'

        # Get root logger
        root_logger = logging.getLogger()

        # Clear existing handlers
        for handler in root_logger.handlers[:]:
            root_logger.removeHandler(handler)

        # Create new handler
        handler = logging.StreamHandler()
        formatter = logging.Formatter(log_format)
        handler.setFormatter(formatter)

        # Set log level
        root_logger.setLevel(level)
        handler.setLevel(level)

        # Add handler
        root_logger.addHandler(handler)

        # Set specific module log levels
        cls._configure_module_loggers(level)

        cls._debug_enabled = (level <= logging.DEBUG)
        cls._current_level = level
        cls._configured = True

    @classmethod
    def _set_log_level(cls, level_or_flag: Union[bool, str, int]):
        """Set log level dynamically without reconfiguring handlers."""
        # Normalize
        if isinstance(level_or_flag, bool):
            # False means fully mute logs by setting an OFF-level above CRITICAL
            level = logging.DEBUG if level_or_flag else (logging.CRITICAL + 50)
        elif isinstance(level_or_flag, int):
            level = level_or_flag
        else:
            m = str(level_or_flag).strip().upper()
            level = {
                "DEBUG": logging.DEBUG,
                "INFO": logging.INFO,
                "DEGRADED": DEGRADED,
                "ERROR": logging.ERROR,
                "CRITICAL": logging.CRITICAL,
            }.get(m, DEGRADED)

        # Update root logger level
        root_logger = logging.getLogger()
        root_logger.setLevel(level)

        # Update all handler levels
        for handler in root_logger.handlers:
            handler.setLevel(level)

        # Update specific module log levels
        cls._configure_module_loggers(level)

        cls._debug_enabled = (level <= logging.DEBUG)
        cls._current_level = level

    @classmethod
    def _configure_module_loggers(cls, level: int):
        """Configure specific module loggers with a unified level."""
        mcpstore_loggers = [
            'mcpstore',
            'mcpstore.core',
            'mcpstore.core.store',
            'mcpstore.core.context',
            'mcpstore.core.orchestrator',
            'mcpstore.core.registry',
            'mcpstore.core.store.client_manager',
            'mcpstore.core.agents.session_manager',
            'mcpstore.core.tool_resolver',
            'mcpstore.plugins.json_mcp',
            'mcpstore.adapters.langchain_adapter'
        ]
        for logger_name in mcpstore_loggers:
            module_logger = logging.getLogger(logger_name)
            module_logger.setLevel(level)

    @classmethod
    def is_debug_enabled(cls) -> bool:
        """Check if debug mode is enabled"""
        return cls._debug_enabled

    @classmethod
    def enable_debug(cls):
        """Enable debug mode"""
        cls.setup_logging(debug=True, force_reconfigure=True)
        # Reduce noise from third-party loggers
        import logging as _logging
        for _name in ("asyncio", "watchfiles", "uvicorn"):
            try:
                _logging.getLogger(_name).setLevel(DEGRADED)
            except Exception:
                pass

    @classmethod
    def disable_debug(cls):
        """Disable debug mode"""
        cls.setup_logging(debug=False, force_reconfigure=True)
        import logging as _logging
        for _name in ("asyncio", "watchfiles", "uvicorn"):
            try:
                _logging.getLogger(_name).setLevel(DEGRADED)
            except Exception:
                pass

# --- Configuration Constants (default values) ---
# Core monitoring configuration
HEARTBEAT_INTERVAL_SECONDS = int(_standalone_defaults.heartbeat_interval_seconds)  # Heartbeat check interval (seconds)
HTTP_TIMEOUT_SECONDS = int(_standalone_defaults.http_timeout_seconds)        # HTTP request timeout (seconds)
RECONNECTION_INTERVAL_SECONDS = int(_standalone_defaults.reconnection_interval_seconds)  # Reconnection attempt interval (seconds)

# HTTP endpoint configuration
STREAMABLE_HTTP_ENDPOINT = "/mcp"  # Streamable HTTP endpoint path

def load_app_config() -> Dict[str, Any]:
    """Load global configuration"""
    try:
        standalone_config = get_standalone_config_with_defaults()
    except Exception as e:
        logger.warning("Failed to load standalone config from MCPStoreConfig, using defaults: %s", e)
        standalone_config = None

    def _get_value(obj: Any, attr_name: str, default: Any) -> Any:
        if obj is None:
            return default
        if hasattr(obj, attr_name):
            try:
                value = getattr(obj, attr_name)
                return value if value is not None else default
            except Exception:
                return default
        if isinstance(obj, dict):
            value = obj.get(attr_name, default)
            return value if value is not None else default
        return default

    config_data = {
        # Core monitoring configuration
        "heartbeat_interval": _get_value(standalone_config, "heartbeat_interval_seconds", HEARTBEAT_INTERVAL_SECONDS),
        "http_timeout": _get_value(standalone_config, "http_timeout_seconds", HTTP_TIMEOUT_SECONDS),
        "reconnection_interval": _get_value(standalone_config, "reconnection_interval_seconds", RECONNECTION_INTERVAL_SECONDS),

        # HTTP endpoint configuration
        "streamable_http_endpoint": _get_value(standalone_config, "streamable_http_endpoint", STREAMABLE_HTTP_ENDPOINT),
    }
    # Load LLM configuration
    # config_data["llm_config"] = load_llm_config()
    # logger.info(f"Loaded configuration from environment: {config_data}")
    return config_data
