"""
Unified sync wrapper utilities for bridging async methods into sync API surfaces
without scattering run_async calls and magic flags across the codebase.

Design goals:
- Centralize timeout and background policy
- Avoid nested event loop pitfalls
- Keep zero behavior change for current defaults

This module introduces two helpers:
- run_sync(coro, *, timeout=None, force_background=None): thin facade over the
  existing global helper to preserve current behavior.
- sync_api(...): decorator for future adoption; not applied anywhere yet.
"""

import asyncio
import functools
import logging
from typing import Any, Callable, Optional

logger = logging.getLogger(__name__)


def run_sync(coro, *, timeout: Optional[float] = None, force_background: Optional[bool] = None):
    """Run an async coroutine from sync code using asyncio.run.

    根据MCPStore核心架构原则，使用最简单的asyncio.run()来桥接同步和异步代码。

    Args:
        coro: Awaitable to execute
        timeout: Optional timeout seconds (暂时忽略，因为asyncio.run()不支持超时)
        force_background: Optional policy to force background loop (忽略，违反核心原则)

    Returns:
        Any: Result of the coroutine
    """
    if force_background:
        logger.warning("force_background=True parameter violates core architecture principles, will be ignored")

    # 简单使用asyncio.run()，符合核心原则
    if timeout is not None:
        logger.warning("timeout parameter is not currently supported, will be ignored")

    return asyncio.run(coro)


def sync_api(*, timeout: Optional[float] = None, force_background: Optional[bool] = None) -> Callable:
    """Decorator to expose async implementations as sync functions with unified policy.

    Usage (planned for future refactors, not applied yet):

        @sync_api(timeout=60.0)
        def list_tools(self):
            return self._list_tools_async()

    The wrapper will detect coroutine return and run via run_sync; otherwise
    it returns the value directly, enabling gradual migration.
    """

    def decorator(func: Callable) -> Callable:
        @functools.wraps(func)
        def wrapper(*args, **kwargs) -> Any:
            result = func(*args, **kwargs)
            # If the function returns a coroutine/awaitable, drive it
            if hasattr(result, "__await__"):
                return run_sync(result, timeout=timeout, force_background=force_background)
            return result

        return wrapper

    return decorator


