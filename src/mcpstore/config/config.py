import os
import sys

sys.path.append(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
import logging
from typing import Dict, Any, Union

logger = logging.getLogger(__name__)

class LoggingConfig:
    """Logging configuration manager"""

    _debug_enabled = False
    _configured = False
    _current_level: int = logging.WARNING

    @classmethod
    def setup_logging(cls, debug: Union[bool, str, int] = False, force_reconfigure: bool = False):
        """
        Setup logging configuration.

        Args:
            debug: Backward-compatible log control. Supports:
                   - True  -> DEBUG
                   - False -> WARNING (was ERROR before; now more practical)
                   - "DEBUG"/"INFO"/"WARNING"/"ERROR"/"CRITICAL" -> exact level
                   - int   -> logging level constant
            force_reconfigure: Whether to force reconfiguration
        """
        def _to_level(v: Union[bool, str, int]) -> int:
            if isinstance(v, bool):
                return logging.DEBUG if v else logging.WARNING
            if isinstance(v, int):
                return v
            if isinstance(v, str):
                m = v.strip().upper()
                return {
                    "DEBUG": logging.DEBUG,
                    "INFO": logging.INFO,
                    "WARNING": logging.WARNING,
                    "ERROR": logging.ERROR,
                    "CRITICAL": logging.CRITICAL,
                }.get(m, logging.WARNING)
            return logging.WARNING

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
            level = logging.DEBUG if level_or_flag else logging.WARNING
        elif isinstance(level_or_flag, int):
            level = level_or_flag
        else:
            m = str(level_or_flag).strip().upper()
            level = {
                "DEBUG": logging.DEBUG,
                "INFO": logging.INFO,
                "WARNING": logging.WARNING,
                "ERROR": logging.ERROR,
                "CRITICAL": logging.CRITICAL,
            }.get(m, logging.WARNING)

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
            'mcpstore.core.client_manager',
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
        # 降噪第三方logger
        import logging as _logging
        for _name in ("asyncio", "watchfiles", "uvicorn"):
            try:
                _logging.getLogger(_name).setLevel(_logging.WARNING)
            except Exception:
                pass

    @classmethod
    def disable_debug(cls):
        """Disable debug mode"""
        cls.setup_logging(debug=False, force_reconfigure=True)
        import logging as _logging
        for _name in ("asyncio", "watchfiles", "uvicorn"):
            try:
                _logging.getLogger(_name).setLevel(_logging.WARNING)
            except Exception:
                pass

# --- Configuration Constants (default values) ---
# Core monitoring configuration
HEARTBEAT_INTERVAL_SECONDS = 60  # Heartbeat check interval (seconds)
HTTP_TIMEOUT_SECONDS = 10        # HTTP request timeout (seconds)
RECONNECTION_INTERVAL_SECONDS = 60  # Reconnection attempt interval (seconds)

# HTTP endpoint configuration
STREAMABLE_HTTP_ENDPOINT = "/mcp"  # 流式HTTP端点路径

# @dataclass
# class LLMConfig:
#     provider: str = "openai_compatible"
#     api_key: str = ""
#     model: str = ""
#     base_url: Optional[str] = None

# def load_llm_config() -> LLMConfig:
#     """Load LLM configuration from environment variables (only supports openai compatible interfaces)"""
#     api_key = os.environ.get("OPENAI_API_KEY", "")
#     model = os.environ.get("OPENAI_MODEL", "")
#     base_url = os.environ.get("OPENAI_BASE_URL")
#     provider = "openai_compatible"
#     if not api_key:
#         logger.warning("OPENAI_API_KEY not set in environment.")
#     if not model:
#         logger.warning("OPENAI_MODEL not set in environment.")
#     return LLMConfig(provider=provider, api_key=api_key, model=model, base_url=base_url)

def _get_env_int(var: str, default: int) -> int:
    try:
        return int(os.environ.get(var, default))
    except Exception:
        logger.warning(f"Environment variable {var} format error, using default value {default}")
        return default

def _get_env_bool(var: str, default: bool) -> bool:
    val = os.environ.get(var)
    if val is None:
        return default
    return val.lower() in ("1", "true", "yes", "on")

def load_app_config() -> Dict[str, Any]:
    """Load global configuration from environment variables"""
    config_data = {
        # Core monitoring configuration
        "heartbeat_interval": _get_env_int("HEARTBEAT_INTERVAL_SECONDS", HEARTBEAT_INTERVAL_SECONDS),
        "http_timeout": _get_env_int("HTTP_TIMEOUT_SECONDS", HTTP_TIMEOUT_SECONDS),
        "reconnection_interval": _get_env_int("RECONNECTION_INTERVAL_SECONDS", RECONNECTION_INTERVAL_SECONDS),

        # HTTP endpoint configuration
        "streamable_http_endpoint": os.environ.get("STREAMABLE_HTTP_ENDPOINT", STREAMABLE_HTTP_ENDPOINT),
    }
    # Load LLM configuration
    # config_data["llm_config"] = load_llm_config()
    # logger.info(f"Loaded configuration from environment: {config_data}")
    return config_data
