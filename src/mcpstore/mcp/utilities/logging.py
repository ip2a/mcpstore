"""Logging utilities for MCPStore."""

import contextlib
import logging
from typing import Any, Literal, cast

from typing_extensions import override

import mcpstore.mcp as mcpstore_mcp
from mcpstore.config.config import LoggingConfig


def get_logger(name: str) -> logging.Logger:
    """Get a logger nested under MCPStore namespace.

    Args:
        name: the name of the logger, which will be prefixed with 'MCPStore.'

    Returns:
        a configured logger instance
    """
    if name.startswith("mcpstore_mcp."):
        return logging.getLogger(name=name)

    return logging.getLogger(name=f"mcpstore_mcp.{name}")


def configure_logging(
    level: Literal["DEBUG", "INFO", "WARNING", "ERROR", "CRITICAL"] | int = "INFO",
    logger: logging.Logger | None = None,
    enable_rich_tracebacks: bool | None = None,
    **rich_kwargs: Any,
) -> None:
    """
    统一入口：委托 LoggingConfig.setup_logging，保持全局格式一致。

    Args:
        logger: 保留兼容参数，当前不再单独配置子 logger
        level: 目标日志级别
        enable_rich_tracebacks: 是否开启富格式 traceback（仅在 use_rich=True 时生效）
        rich_kwargs: 兼容参数，当前不使用
    """
    if not mcpstore_mcp.settings.log_enabled:
        return

    if enable_rich_tracebacks is None:
        enable_rich_tracebacks = mcpstore_mcp.settings.enable_rich_tracebacks

    # 统一改由 LoggingConfig 处理（root + 子 logger）
    LoggingConfig.setup_logging(
        debug=level,
        use_rich=mcpstore_mcp.settings.enable_rich_logging,
        rich_traceback=enable_rich_tracebacks,
        force_reconfigure=True,
    )


@contextlib.contextmanager
def temporary_log_level(
    level: str | None,
    logger: logging.Logger | None = None,
    enable_rich_tracebacks: bool | None = None,
    **rich_kwargs: Any,
):
    """Context manager to temporarily set log level and restore it afterwards.

    Args:
        level: The temporary log level to set (e.g., "DEBUG", "INFO")
        logger: Optional logger to configure (defaults to MCPStore logger)
        enable_rich_tracebacks: Whether to enable rich tracebacks
        **rich_kwargs: Additional parameters for RichHandler

    Usage:
        with temporary_log_level("DEBUG"):
            # Code that runs with DEBUG logging
            pass
        # Original log level is restored here
    """
    if level:
        # Get the original log level from settings
        original_level = mcpstore_mcp.settings.log_level

        # Configure with new level
        # Cast to proper type for type checker
        log_level_literal = cast(
            Literal["DEBUG", "INFO", "WARNING", "ERROR", "CRITICAL"],
            level.upper(),
        )
        configure_logging(
            level=log_level_literal,
            logger=logger,
            enable_rich_tracebacks=enable_rich_tracebacks,
            **rich_kwargs,
        )
        try:
            yield
        finally:
            # Restore original configuration using configure_logging
            # This will respect the log_enabled setting
            configure_logging(
                level=original_level,
                logger=logger,
                enable_rich_tracebacks=enable_rich_tracebacks,
                **rich_kwargs,
            )
    else:
        yield


_level_to_no: dict[
    Literal["DEBUG", "INFO", "WARNING", "ERROR", "CRITICAL"] | None, int | None
] = {
    "DEBUG": logging.DEBUG,
    "INFO": logging.INFO,
    "WARNING": logging.WARNING,
    "ERROR": logging.ERROR,
    "CRITICAL": logging.CRITICAL,
    None: None,
}


class _ClampedLogFilter(logging.Filter):
    min_level: tuple[int, str] | None
    max_level: tuple[int, str] | None

    def __init__(
        self,
        min_level: Literal["DEBUG", "INFO", "WARNING", "ERROR", "CRITICAL"]
        | None = None,
        max_level: Literal["DEBUG", "INFO", "WARNING", "ERROR", "CRITICAL"]
        | None = None,
    ):
        self.min_level = None
        self.max_level = None

        if min_level_no := _level_to_no.get(min_level):
            self.min_level = (min_level_no, str(min_level))
        if max_level_no := _level_to_no.get(max_level):
            self.max_level = (max_level_no, str(max_level))

        super().__init__()

    @override
    def filter(self, record: logging.LogRecord) -> bool:
        if self.max_level:
            max_level_no, max_level_name = self.max_level

            if record.levelno > max_level_no:
                record.levelno = max_level_no
                record.levelname = max_level_name
                return True

        if self.min_level:
            min_level_no, min_level_name = self.min_level
            if record.levelno < min_level_no:
                record.levelno = min_level_no
                record.levelname = min_level_name
                return True

        return True


def _clamp_logger(
    logger: logging.Logger,
    min_level: Literal["DEBUG", "INFO", "WARNING", "ERROR", "CRITICAL"] | None = None,
    max_level: Literal["DEBUG", "INFO", "WARNING", "ERROR", "CRITICAL"] | None = None,
) -> None:
    """Clamp the logger to a minimum and maximum level.

    If min_level is provided, messages logged at a lower level than `min_level` will have their level increased to `min_level`.
    If max_level is provided, messages logged at a higher level than `max_level` will have their level decreased to `max_level`.

    Args:
        min_level: The lower bound of the clamp
        max_level: The upper bound of the clamp
    """
    _unclamp_logger(logger=logger)

    logger.addFilter(filter=_ClampedLogFilter(min_level=min_level, max_level=max_level))


def _unclamp_logger(logger: logging.Logger) -> None:
    """Remove all clamped log filters from the logger."""
    for filter in logger.filters[:]:
        if isinstance(filter, _ClampedLogFilter):
            logger.removeFilter(filter)
