from __future__ import annotations

import asyncio
import threading
from collections import defaultdict
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from typing import Any, Coroutine, Dict, Optional

from .async_orchestrated_bridge import get_async_bridge


@dataclass(frozen=True)
class BridgeExecutionStats:
    sync_calls: int
    sync_thread_handoff_calls: int
    async_calls: int
    async_same_loop_direct_await_calls: int
    async_thread_handoff_calls: int
    per_operation: Dict[str, int]


class UnifiedBridgeExecutor:
    def __init__(self) -> None:
        self._bridge = get_async_bridge()
        self._lock = threading.Lock()
        self._sync_calls = 0
        self._sync_thread_handoff_calls = 0
        self._async_calls = 0
        self._async_same_loop_direct_await_calls = 0
        self._async_thread_handoff_calls = 0
        self._per_operation: Dict[str, int] = defaultdict(int)

    def run_sync(self, coro: Coroutine[Any, Any, Any], *, op_name: str, timeout: Optional[float] = None) -> Any:
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

        self._record_sync(op_name=op_name, used_thread_handoff=running_in_loop)

        if running_in_loop:
            with ThreadPoolExecutor(max_workers=1) as executor:
                future = executor.submit(self._bridge.run, coro, timeout=timeout, op_name=op_name)
                return future.result()

        return self._bridge.run(coro, timeout=timeout, op_name=op_name)

    async def execute(self, coro: Coroutine[Any, Any, Any], *, op_name: str, timeout: Optional[float] = None) -> Any:
        effective_timeout = timeout
        if effective_timeout is None:
            effective_timeout = getattr(self._bridge, "_default_timeout", None)

        bridge_loop = getattr(self._bridge, "_loop", None)
        try:
            running_loop = asyncio.get_running_loop()
        except RuntimeError:
            running_loop = None

        if bridge_loop and running_loop is bridge_loop:
            self._record_async(op_name=op_name, same_loop_direct_await=True, used_thread_handoff=False)
            if effective_timeout is not None:
                return await asyncio.wait_for(coro, timeout=effective_timeout)
            return await coro

        if running_loop is None:
            self._record_async(op_name=op_name, same_loop_direct_await=False, used_thread_handoff=False)
            return self._bridge.run(coro, timeout=effective_timeout, op_name=op_name)

        self._record_async(op_name=op_name, same_loop_direct_await=False, used_thread_handoff=True)
        return await asyncio.to_thread(self._bridge.run, coro, timeout=effective_timeout, op_name=op_name)

    def get_stats(self) -> BridgeExecutionStats:
        with self._lock:
            return BridgeExecutionStats(
                sync_calls=self._sync_calls,
                sync_thread_handoff_calls=self._sync_thread_handoff_calls,
                async_calls=self._async_calls,
                async_same_loop_direct_await_calls=self._async_same_loop_direct_await_calls,
                async_thread_handoff_calls=self._async_thread_handoff_calls,
                per_operation=dict(self._per_operation),
            )

    def reset_stats(self) -> None:
        with self._lock:
            self._sync_calls = 0
            self._sync_thread_handoff_calls = 0
            self._async_calls = 0
            self._async_same_loop_direct_await_calls = 0
            self._async_thread_handoff_calls = 0
            self._per_operation.clear()

    def _record_sync(self, *, op_name: str, used_thread_handoff: bool) -> None:
        with self._lock:
            self._sync_calls += 1
            if used_thread_handoff:
                self._sync_thread_handoff_calls += 1
            self._per_operation[op_name] += 1

    def _record_async(self, *, op_name: str, same_loop_direct_await: bool, used_thread_handoff: bool) -> None:
        with self._lock:
            self._async_calls += 1
            if same_loop_direct_await:
                self._async_same_loop_direct_await_calls += 1
            if used_thread_handoff:
                self._async_thread_handoff_calls += 1
            self._per_operation[op_name] += 1


_GLOBAL_EXECUTOR: Optional[UnifiedBridgeExecutor] = None
_GLOBAL_EXECUTOR_LOCK = threading.Lock()


def get_bridge_executor() -> UnifiedBridgeExecutor:
    global _GLOBAL_EXECUTOR
    if _GLOBAL_EXECUTOR is None:
        with _GLOBAL_EXECUTOR_LOCK:
            if _GLOBAL_EXECUTOR is None:
                _GLOBAL_EXECUTOR = UnifiedBridgeExecutor()
    return _GLOBAL_EXECUTOR


def get_bridge_execution_stats() -> BridgeExecutionStats:
    return get_bridge_executor().get_stats()


def reset_bridge_execution_stats() -> None:
    get_bridge_executor().reset_stats()
