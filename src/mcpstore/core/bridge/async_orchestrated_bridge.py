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
from concurrent.futures import Future, TimeoutError as FutureTimeoutError
from dataclasses import dataclass
from typing import Any, Dict, Optional

logger = logging.getLogger(__name__)


@dataclass
class BridgeTaskHandle:
    """桥接后台任务的同步控制句柄。"""

    _bridge: "AsyncOrchestratedBridge"
    _task_id: str

    def cancel(self) -> None:
        self._bridge._request_cancel_background_task(self._task_id)

    def done(self) -> bool:
        entry = self._bridge._background_tasks.get(self._task_id)
        if not entry:
            return True
        return entry.future.done()


class AsyncOrchestratedBridge:
    """进程级的异步执行桥梁。"""

    def __init__(self, default_timeout: float = 60.0):
        self._default_timeout = default_timeout
        self._loop: Optional[asyncio.AbstractEventLoop] = None
        self._thread: Optional[threading.Thread] = None
        self._loop_lock = threading.RLock()
        self._stop_event = threading.Event()
        self._active_calls: Dict[str, Dict[str, Any]] = {}
        self._background_tasks: Dict[str, "_BackgroundEntry"] = {}
        self._heartbeat_task: Optional[asyncio.Task[Any]] = None
        self._heartbeat_interval = 0.05

    # ------------------------------------------------------------------ #
    # 外部接口
    # ------------------------------------------------------------------ #

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
            allow_async: 在已有事件循环中是否允许直接 await（默认 False，保证同步 API 不被异步上下文误用）
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

    def create_background_task(
        self,
        coro: asyncio.coroutines.Coroutine[Any, Any, Any],
        *,
        op_name: str = "background",
    ) -> BridgeTaskHandle:
        """
        在后台循环中启动协程，返回同步侧的控制句柄。

        该接口适用于健康检查、监控等长期任务。
        """
        loop = self._ensure_loop()
        task_id = self._generate_task_id(op_name)
        entry = _BackgroundEntry(op_name=op_name, created=time.time())
        self._background_tasks[task_id] = entry

        async def starter():
            async def wrapped():
                try:
                    await coro
                except asyncio.CancelledError:
                    raise
                except Exception:
                    logger.exception("[AOB] Background task %s crashed", op_name)
                    raise

            entry.task = asyncio.create_task(wrapped(), name=f"AOB:{op_name}")

            def _cleanup(task: asyncio.Task[Any]):
                def _remove():
                    self._background_tasks.pop(task_id, None)

                try:
                    task.result()
                except asyncio.CancelledError:
                    logger.info("[AOB] Background task %s cancelled", op_name)
                except Exception:
                    logger.exception("[AOB] Background task %s finished with error", op_name)
                finally:
                    loop = self._loop
                    if loop and loop.is_running():
                        loop.call_soon_threadsafe(_remove)
                    else:
                        _remove()

            entry.task.add_done_callback(_cleanup)

        entry.future = asyncio.run_coroutine_threadsafe(starter(), loop)
        return BridgeTaskHandle(self, task_id)

    def inspect_active_calls(self) -> Dict[str, Dict[str, Any]]:
        """获取当前活跃的同步调用信息。"""
        return dict(self._active_calls)

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

    def _request_cancel_background_task(self, task_id: str) -> None:
        entry = self._background_tasks.get(task_id)
        if not entry:
            return
        loop = self._loop
        if not loop or not loop.is_running():
            return

        def _cancel():
            if entry.task and not entry.task.done():
                entry.task.cancel()

        loop.call_soon_threadsafe(_cancel)

    def _generate_task_id(self, op_name: str) -> str:
        return f"{op_name}:{uuid.uuid4()}"

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


@dataclass
class _BackgroundEntry:
    op_name: str
    created: float
    future: Future[None] | None = None
    task: asyncio.Task[Any] | None = None


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


def close_async_bridge() -> None:
    """关闭全局 AOB。"""
    global _GLOBAL_BRIDGE
    with _GLOBAL_LOCK:
        if _GLOBAL_BRIDGE is not None:
            _GLOBAL_BRIDGE.close()
            _GLOBAL_BRIDGE = None
