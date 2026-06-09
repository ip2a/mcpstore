"""
Async Orchestrated Bridge (AOB)

为同步 API 提供一个持久化的异步执行通道，避免在多个事件循环之间切换导致的冲突。
"""

from __future__ import annotations

import asyncio
import logging
import threading
import time
import uuid
from concurrent.futures import ThreadPoolExecutor, TimeoutError as FutureTimeoutError
from typing import Any, Coroutine, Dict, Optional

logger = logging.getLogger(__name__)


class AsyncOrchestratedBridge:
    """进程级的异步执行桥梁。"""

    def __init__(self, default_timeout: float = 60.0):
        self._default_timeout = default_timeout
        self._loop: Optional[asyncio.AbstractEventLoop] = None
        self._thread: Optional[threading.Thread] = None
        self._loop_lock = threading.RLock()
        self._stop_event = threading.Event()
        self._active_calls: Dict[str, Dict[str, Any]] = {}
        self._heartbeat_task: Optional[asyncio.Task[Any]] = None
        self._heartbeat_interval = 0.05

    def run(
        self,
        coro: asyncio.coroutines.Coroutine[Any, Any, Any],
        *,
        timeout: Optional[float] = None,
        op_name: str = "unknown",
    ) -> Any:
        """
        在稳定事件循环中运行协程。

        Args:
            coro: 要执行的协程
            timeout: 超时时间（秒），默认使用实例的 default_timeout
            op_name: 操作名称，用于日志/诊断
        """
        if timeout is None:
            timeout = self._default_timeout

        if self._in_async_context():
            raise RuntimeError(
                f"检测到正在运行的事件循环：请使用 {op_name}_async() 接口。"
            )

        loop = self._ensure_loop()
        call_id = self._register_call(op_name)

        async def runner():
            return await asyncio.wait_for(coro, timeout=timeout)

        future = asyncio.run_coroutine_threadsafe(runner(), loop)
        try:
            result = future.result(timeout=timeout)
            return result
        except FutureTimeoutError as exc:
            logger.error("[AOB] %s timed out after %.1fs", op_name, timeout)
            future.cancel()
            raise TimeoutError(f"{op_name} timed out after {timeout}s") from exc
        finally:
            self._unregister_call(call_id)

    def close(self) -> None:
        """停止后台循环并清理资源。"""
        with self._loop_lock:
            if not self._loop:
                return
            logger.info("[AOB] shutting down bridge loop")
            self._stop_event.set()
            loop = self._loop
            loop.call_soon_threadsafe(loop.stop)
            if self._thread and self._thread.is_alive():
                self._thread.join(timeout=2)
            self._loop = None
            self._thread = None

    # ------------------------------------------------------------------ #
    # 内部实现
    # ------------------------------------------------------------------ #

    def _ensure_loop(self) -> asyncio.AbstractEventLoop:
        with self._loop_lock:
            if self._loop and self._loop.is_running():
                return self._loop
            self._stop_event.clear()
            loop_ready = threading.Event()

            def _run_loop():
                loop = asyncio.new_event_loop()
                asyncio.set_event_loop(loop)
                self._loop = loop
                self._heartbeat_task = loop.create_task(
                    self._loop_heartbeat(),
                    name="AOB:heartbeat",
                )
                self._heartbeat_task.add_done_callback(
                    lambda task: logger.debug("[AOB] Heartbeat stopped: %s", task)
                )
                loop_ready.set()
                logger.info("[AOB] event loop started (thread=%s)", threading.current_thread().name)
                loop.run_forever()
                if self._heartbeat_task and not self._heartbeat_task.done():
                    self._heartbeat_task.cancel()
                    try:
                        loop.run_until_complete(self._heartbeat_task)
                    except Exception:
                        pass
                self._heartbeat_task = None
                loop.close()
                logger.info("[AOB] event loop stopped")

            self._thread = threading.Thread(target=_run_loop, name="async_bridge_loop", daemon=True)
            self._thread.start()
            if not loop_ready.wait(timeout=5):
                raise RuntimeError("Async bridge loop failed to start")
            return self._loop  # type: ignore[return-value]

    def _register_call(self, op_name: str) -> str:
        call_id = f"{op_name}:{uuid.uuid4()}"
        self._active_calls[call_id] = {
            "operation": op_name,
            "start_time": time.time(),
            "thread": threading.current_thread().name,
        }
        return call_id

    def _unregister_call(self, call_id: str) -> None:
        self._active_calls.pop(call_id, None)

    async def _loop_heartbeat(self) -> None:
        """
        Keep the async loop from idling forever on selectors.

        Some environments don't deliver selector wakeups reliably when the loop
        is completely idle, so we yield periodically to guarantee forward
        progress for run_coroutine_threadsafe() calls.
        """
        try:
            while not self._stop_event.is_set():
                await asyncio.sleep(self._heartbeat_interval)
        except asyncio.CancelledError:
            pass

    @staticmethod
    def _in_async_context() -> bool:
        try:
            asyncio.get_running_loop()
            return True
        except RuntimeError:
            return False


_GLOBAL_BRIDGE: Optional[AsyncOrchestratedBridge] = None
_GLOBAL_LOCK = threading.Lock()


def get_async_bridge() -> AsyncOrchestratedBridge:
    """获取全局 AOB 实例。"""
    global _GLOBAL_BRIDGE
    if _GLOBAL_BRIDGE is None:
        with _GLOBAL_LOCK:
            if _GLOBAL_BRIDGE is None:
                _GLOBAL_BRIDGE = AsyncOrchestratedBridge()
    return _GLOBAL_BRIDGE


class UnifiedBridgeExecutor:
    """统一桥接执行器，包装 AsyncOrchestratedBridge 提供 sync/async 双模式执行。"""

    def __init__(self) -> None:
        self._bridge = get_async_bridge()

    def run_sync(
        self,
        coro: Coroutine[Any, Any, Any],
        *,
        op_name: str,
        timeout: Optional[float] = None,
    ) -> Any:
        bridge_loop = getattr(self._bridge, "_loop", None)
        try:
            running_loop = asyncio.get_running_loop()
            running_in_loop = True
        except RuntimeError:
            running_loop = None
            running_in_loop = False

        if running_in_loop and bridge_loop and running_loop is bridge_loop:
            if hasattr(coro, "close"):
                coro.close()
            raise RuntimeError(
                f"检测到在 AOB 事件循环内调用同步桥接：请改用 {op_name}_async() 接口。"
            )

        if running_in_loop:
            with ThreadPoolExecutor(max_workers=1) as executor:
                future = executor.submit(
                    self._bridge.run, coro, timeout=timeout, op_name=op_name
                )
                return future.result()

        return self._bridge.run(coro, timeout=timeout, op_name=op_name)


_GLOBAL_EXECUTOR: Optional[UnifiedBridgeExecutor] = None
_GLOBAL_EXECUTOR_LOCK = threading.Lock()


def get_bridge_executor() -> UnifiedBridgeExecutor:
    """获取全局 UnifiedBridgeExecutor 实例。"""
    global _GLOBAL_EXECUTOR
    if _GLOBAL_EXECUTOR is None:
        with _GLOBAL_EXECUTOR_LOCK:
            if _GLOBAL_EXECUTOR is None:
                _GLOBAL_EXECUTOR = UnifiedBridgeExecutor()
    return _GLOBAL_EXECUTOR
