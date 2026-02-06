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
    """Run an async coroutine from sync code using the global AOB bridge.

    Args:
        coro: Awaitable to execute
        timeout: Optional timeout seconds
        force_background: Optional policy to force background loop (保留兼容，无额外语义)

    Returns:
        Any: Result of the coroutine
    """
    if force_background:
        logger.warning("force_background=True parameter currently unused; bridge will be used regardless")

    try:
        from mcpstore.core.bridge import get_async_bridge
        bridge = get_async_bridge()
    except Exception:
        bridge = None

    # 优先使用持久 AOB，避免 asyncio.run 结束时取消后台任务
        if bridge is not None:
            try:
                running_loop = asyncio.get_running_loop()
            except RuntimeError:
                running_loop = None

        if running_loop:
            # 平滑处理：在当前事件循环中将 bridge.run 放到线程执行，避免直接抛错中断主逻辑
            logger.warning(
                "[sync_api] Detected running event loop; dispatching via to_thread + bridge.run "
                "for compatibility. Please call the async variant instead.",
            )
            try:
                return asyncio.to_thread(
                    bridge.run,
                    coro,
                    timeout=timeout,
                    op_name="sync_api.run_sync",
                )
            except Exception:
                logger.error("[sync_api] Failed to dispatch via bridge in to_thread; re-raising", exc_info=True)
                raise
        return bridge.run(coro, timeout=timeout, op_name="sync_api.run_sync")

    # 回退：无桥时才使用 asyncio.run
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
