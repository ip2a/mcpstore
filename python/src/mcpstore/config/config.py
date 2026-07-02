"""
Optimized configuration module
Remove sys.path operations to improve import performance
"""
import logging
from typing import Any, Dict, Union

# Remove sys.path.append() operations to improve import performance
# If you need to import other modules, please use relative imports or correct package structure

# Define custom DEGRADED log level (between INFO and WARNING)
DEGRADED = 25  # INFO=20, WARNING=30
logging.addLevelName(DEGRADED, "DEGRADED")

class LoggingConfig:
    """Logging configuration manager"""

    _debug_enabled = False
    _configured = False
    _current_level: int = DEGRADED
    _use_rich: bool = False
    _rich_traceback: bool = False

    @classmethod
    def setup_logging(
        cls,
        debug: Union[bool, str, int] = False,
        *,
        use_rich: bool = False,
        rich_traceback: bool = False,
        force_reconfigure: bool = False,
    ):
        """
        Setup logging configuration.

        Args:
            debug: Log level control. Supports:
                   - True  -> DEBUG
                   - False -> logging disabled
                   - "DEBUG"/"INFO"/"DEGRADED"/"ERROR"/"CRITICAL" -> exact level
                   - int   -> logging level constant
            use_rich: Whether to enable rich formatted logging globally
            rich_traceback: Whether to show rich tracebacks when using rich logging
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
            # 若仅调整等级则快速返回；格式变更需重新配置
            if level == cls._current_level and use_rich == cls._use_rich and rich_traceback == cls._rich_traceback:
                return
            if level != cls._current_level:
                cls._set_log_level(level)
            if use_rich != cls._use_rich or rich_traceback != cls._rich_traceback:
                force_reconfigure = True
            if not force_reconfigure:
                return

        # Configure log format/handler
        if use_rich:
            from rich.console import Console
            from rich.logging import RichHandler

            console = Console(stderr=True, width=180, soft_wrap=True, overflow="ignore")
            handler = RichHandler(
                console=console,
                show_path=False,
                rich_tracebacks=rich_traceback,
            )
            formatter = logging.Formatter("%(message)s")
        else:
            if level <= logging.DEBUG:
                log_format = '%(asctime)s - %(name)s - %(levelname)s - %(message)s'
            else:
                log_format = '%(levelname)s - %(message)s'
            handler = logging.StreamHandler()
            formatter = logging.Formatter(log_format)

        # Get root logger
        root_logger = logging.getLogger()

        # Clear existing handlers
        for h in root_logger.handlers[:]:
            root_logger.removeHandler(h)

        handler.setFormatter(formatter)

        # Set log level
        root_logger.setLevel(level)
        handler.setLevel(level)

        # Add handler
        root_logger.addHandler(handler)

        # Set specific module log levels
        cls._configure_module_loggers(level, propagate=True)

        cls._debug_enabled = level <= logging.DEBUG
        cls._current_level = level
        cls._use_rich = use_rich
        cls._rich_traceback = rich_traceback
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
        cls._configure_module_loggers(level, propagate=True)

        cls._debug_enabled = level <= logging.DEBUG
        cls._current_level = level

    @classmethod
    def _configure_module_loggers(cls, level: int, propagate: bool = False):
        """Configure specific module loggers with a unified level and propagation."""
        mcpstore_loggers = [
            'mcpstore',
            'mcpstore.core',
            'mcpstore.core.store',
            'mcpstore.adapters.langchain_adapter',
        ]
        for logger_name in mcpstore_loggers:
            module_logger = logging.getLogger(logger_name)
            module_logger.setLevel(level)
            module_logger.propagate = propagate
            # 清空子 logger 自带的 handler，避免重复输出
            for h in module_logger.handlers[:]:
                module_logger.removeHandler(h)

    @classmethod
    def is_debug_enabled(cls) -> bool:
        """Return whether MCPStore debug logging is enabled."""
        return cls._debug_enabled

    @classmethod
    def enable_debug(cls):
        """Enable MCPStore debug logging."""
        cls.setup_logging(debug=True, force_reconfigure=True)
        for name in ("asyncio", "watchfiles", "uvicorn", "httpx", "httpcore"):
            logging.getLogger(name).setLevel(DEGRADED)

    @classmethod
    def disable_debug(cls):
        """Disable MCPStore debug logging."""
        cls.setup_logging(debug=False, force_reconfigure=True)
        for name in ("asyncio", "watchfiles", "uvicorn", "httpx", "httpcore"):
            logging.getLogger(name).setLevel(DEGRADED)


HEARTBEAT_INTERVAL_SECONDS = 30
HTTP_TIMEOUT_SECONDS = 30
RECONNECTION_INTERVAL_SECONDS = 5
STREAMABLE_HTTP_ENDPOINT = "/mcp"


def load_app_config() -> Dict[str, Any]:
    """Return Python compatibility defaults for legacy config callers.

    Runtime behavior is configured by the Rust core. This function preserves the
    old read-only shape used by Python integrations that inspect app defaults.
    """

    return {
        "heartbeat_interval": HEARTBEAT_INTERVAL_SECONDS,
        "http_timeout": HTTP_TIMEOUT_SECONDS,
        "reconnection_interval": RECONNECTION_INTERVAL_SECONDS,
        "streamable_http_endpoint": STREAMABLE_HTTP_ENDPOINT,
    }
