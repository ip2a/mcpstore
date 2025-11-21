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

import functools
from typing import Any, Callable, Optional

from .async_sync_helper import get_global_helper


def run_sync(coro, *, timeout: Optional[float] = None, force_background: Optional[bool] = None):
    """Run an async coroutine from sync code using the global helper.

    Args:
        coro: Awaitable to execute
        timeout: Optional timeout seconds
        force_background: Optional policy to force background loop

    Returns:
        Any: Result of the coroutine
    """
    helper = get_global_helper()
    if force_background is None and timeout is None:
        return helper.run_async(coro)
    if force_background is None:
        return helper.run_async(coro, timeout=timeout)
    if timeout is None:
        return helper.run_async(coro, force_background=force_background)
    return helper.run_async(coro, timeout=timeout, force_background=force_background)


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


