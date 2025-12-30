"""
死锁安全的异步同步助手

解决嵌套事件循环死锁的根本性修复方案
"""

import asyncio
import logging
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from contextlib import contextmanager
from typing import Any, Coroutine, Optional, Dict, List

logger = logging.getLogger(__name__)


class DeadlockSafeAsyncHelper:
    """
    死锁安全的异步同步助手

    核心特性：
    1. 检测并防止嵌套调用死锁
    2. 重入锁机制支持递归调用
    3. 调用链追踪和超时检测
    4. 线程本地存储避免跨线程冲突
    """

    def __init__(self, max_concurrent_calls: int = 10, default_timeout: float = 30.0):
        """
        初始化死锁安全的异步助手

        Args:
            max_concurrent_calls: 最大并发调用数
            default_timeout: 默认超时时间
        """
        self.max_concurrent_calls = max_concurrent_calls
        self.default_timeout = default_timeout

        # 使用可重入锁而不是普通锁
        self._lock = threading.RLock()
        self._loop: Optional[asyncio.AbstractEventLoop] = None
        self._loop_thread: Optional[threading.Thread] = None

        # 调用链追踪
        self._active_calls: Dict[str, float] = {}  # operation_name -> start_time
        self._call_stack: threading.local = threading.local()

        # 线程池执行器，避免创建过多线程
        self._executor = ThreadPoolExecutor(
            max_workers=4,
            thread_name_prefix="deadlock_safe_async"
        )

        logger.debug("DeadlockSafeAsyncHelper initialized with reentrant lock")

    def run_async(self, coro: Coroutine[Any, Any, Any], timeout: Optional[float] = None,
                  operation_name: str = "unknown", force_background: bool = False) -> Any:
        """
        运行异步协程，防止死锁

        Args:
            coro: 要运行的协程
            timeout: 超时时间
            operation_name: 操作名称，用于调试
            force_background: 是否强制使用后台线程

        Returns:
            协程的执行结果

        Raises:
            RuntimeError: 如果检测到潜在的死锁
            TimeoutError: 如果执行超时
        """
        if timeout is None:
            timeout = self.default_timeout

        import time as _t
        start_time = _t.perf_counter()

        try:
            # 检查当前调用上下文
            current_context = self._get_call_context()

            # 检测潜在的死锁
            if self._detect_potential_deadlock(operation_name, current_context):
                raise RuntimeError(
                    f"Potential deadlock detected for operation '{operation_name}'. "
                    f"Current context: {current_context}, active calls: {list(self._active_calls.keys())}"
                )

            # 记录调用开始
            self._record_call_start(operation_name, current_context)

            try:
                # 选择执行策略
                if self._is_in_async_context():
                    result = self._run_in_existing_loop(coro, timeout, operation_name)
                elif force_background:
                    result = self._run_in_background_thread(coro, timeout, operation_name)
                else:
                    result = self._run_in_new_loop(coro, timeout, operation_name)

                elapsed = _t.perf_counter() - start_time
                logger.debug(f"[DEADLOCK_SAFE] {operation_name} completed in {elapsed:.3f}s")
                return result

            finally:
                # 记录调用结束
                self._record_call_end(operation_name)

        except Exception as e:
            elapsed = _t.perf_counter() - start_time
            logger.error(f"[DEADLOCK_SAFE] {operation_name} failed after {elapsed:.3f}s: {e}")
            raise

    def _detect_potential_deadlock(self, operation_name: str, context: str) -> bool:
        """
        检测潜在的死锁情况

        Returns:
            True 如果检测到潜在死锁
        """
        current_time = time.time()

        # 检查1: 同一操作的重入
        if operation_name in self._active_calls:
            elapsed = current_time - self._active_calls[operation_name]
            if elapsed < 1.0:  # 1秒内的重入可能是死锁
                logger.warning(f"[DEADLOCK_SAFE] Potential recursive deadlock: {operation_name}")
                return True

        # 检查2: 活动调用数量过多
        if len(self._active_calls) >= self.max_concurrent_calls:
            logger.warning(f"[DEADLOCK_SAFE] Too many concurrent calls: {len(self._active_calls)}")
            return True

        # 检查3: 长时间运行的操作
        for op_name, start_time in self._active_calls.items():
            if current_time - start_time > timeout * 0.8:  # 80% 超时时间
                logger.warning(f"[DEADLOCK_SAFE] Long running operation detected: {op_name}")
                # 不直接返回False，允许继续但记录警告

        return False

    def _record_call_start(self, operation_name: str, context: str):
        """记录调用开始"""
        self._active_calls[operation_name] = time.time()

        # 初始化线程本地的调用栈
        if not hasattr(self._call_stack, 'stack'):
            self._call_stack.stack = []

        self._call_stack.stack.append({
            'operation': operation_name,
            'context': context,
            'start_time': time.time()
        })

    def _record_call_end(self, operation_name: str):
        """记录调用结束"""
        self._active_calls.pop(operation_name, None)

        # 更新调用栈
        if hasattr(self._call_stack, 'stack') and self._call_stack.stack:
            call_info = self._call_stack.stack[-1]
            if call_info['operation'] == operation_name:
                self._call_stack.stack.pop()

    def _get_call_context(self) -> str:
        """获取当前调用上下文"""
        try:
            import inspect
            frame = inspect.currentframe()
            context_parts = []

            # 获取调用栈信息
            for _ in range(5):  # 最多5层调用栈
                frame = frame.f_back
                if frame is None:
                    break

                func_name = frame.f_code.co_name
                filename = frame.f_code.co_filename
                line_no = frame.f_lineno

                # 简化文件名
                simple_filename = filename.split('/')[-1] if '/' in filename else filename

                context_parts.append(f"{func_name}({simple_filename}:{line_no})")

            return " -> ".join(context_parts)

        except Exception as e:
            logger.debug(f"[DEADLOCK_SAFE] Failed to get call context: {e}")
            return "unknown_context"

    def _is_in_async_context(self) -> bool:
        """检查是否在异步上下文中"""
        try:
            asyncio.get_running_loop()
            return True
        except RuntimeError:
            return False

    def _run_in_existing_loop(self, coro: Coroutine, timeout: float, operation_name: str) -> Any:
        """在现有事件循环中运行"""
        logger.debug(f"[DEADLOCK_SAFE] Running {operation_name} in existing async loop")

        try:
            # 使用 asyncio.create_task 在当前循环中调度
            task = asyncio.create_task(coro)
            return asyncio.wait_for(task, timeout=timeout)
        except RuntimeError as e:
            if "no running event loop" in str(e):
                # 事件循环已经关闭，降级到后台线程
                logger.warning(f"[DEADLOCK_SAFE] Event loop closed, falling back to background: {operation_name}")
                return self._run_in_background_thread(coro, timeout, operation_name)
            raise

    def _run_in_background_thread(self, coro: Coroutine, timeout: float, operation_name: str) -> Any:
        """在后台线程中运行"""
        logger.debug(f"[DEADLOCK_SAFE] Running {operation_name} in background thread")

        # 确保后台循环存在
        loop = self._ensure_background_loop()

        # 提交任务到后台循环
        future = asyncio.run_coroutine_threadsafe(coro, loop)

        try:
            return future.result(timeout=timeout)
        except Exception as e:
            logger.error(f"[DEADLOCK_SAFE] Background thread execution failed for {operation_name}: {e}")
            raise

    def _run_in_new_loop(self, coro: Coroutine, timeout: float, operation_name: str) -> Any:
        """在新的事件循环中运行"""
        logger.debug(f"[DEADLOCK_SAFE] Running {operation_name} in new event loop")

        try:
            return asyncio.run(coro)
        except Exception as e:
            logger.error(f"[DEADLOCK_SAFE] New loop execution failed for {operation_name}: {e}")
            raise

    def _ensure_background_loop(self) -> asyncio.AbstractEventLoop:
        """确保后台事件循环存在"""
        if self._loop is None or self._loop.is_closed():
            with self._lock:
                # 双重检查锁定
                if self._loop is None or self._loop.is_closed():
                    self._create_background_loop()

        return self._loop

    def _create_background_loop(self):
        """创建后台事件循环"""
        loop_ready = threading.Event()

        def run_loop():
            """在独立线程中运行事件循环"""
            try:
                # 设置线程本地的事件循环
                self._loop = asyncio.new_event_loop()
                asyncio.set_event_loop(self._loop)

                # 设置异常处理器
                self._setup_exception_handler()

                logger.debug("[DEADLOCK_SAFE] Background event loop started")
                loop_ready.set()

                # 运行事件循环
                self._loop.run_forever()

            except Exception as e:
                logger.error(f"[DEADLOCK_SAFE] Background loop error: {e}")
            finally:
                logger.debug("[DEADLOCK_SAFE] Background event loop stopped")

        # 启动后台线程
        self._loop_thread = threading.Thread(
            target=run_loop,
            daemon=True,
            name="deadlock_safe_event_loop"
        )
        self._loop_thread.start()

        # 等待循环启动
        if not loop_ready.wait(timeout=5):
            raise RuntimeError("Failed to start background event loop")

    def _setup_exception_handler(self):
        """设置异常处理器"""
        def _exception_handler(loop, context):
            try:
                exc = context.get("exception")
                msg = context.get("message", "")

                if exc is not None:
                    logger.warning(f"[DEADLOCK_SAFE] Background task error: {exc}")
                else:
                    logger.warning(f"[DEADLOCK_SAFE] Background loop warning: {msg}")

            except Exception:
                # 异常处理器本身不应抛出异常
                pass

        self._loop.set_exception_handler(_exception_handler)

    @contextmanager
    def call_context(self, operation_name: str):
        """
        调用上下文管理器，用于自动记录调用

        Usage:
            with async_helper.call_context("my_operation"):
                result = await some_async_operation()
        """
        context = self._get_call_context()
        self._record_call_start(operation_name, context)

        try:
            yield
        finally:
            self._record_call_end(operation_name)

    def get_active_calls(self) -> Dict[str, Dict[str, Any]]:
        """获取当前活动调用信息"""
        current_time = time.time()
        active_calls_info = {}

        for op_name, start_time in self._active_calls.items():
            active_calls_info[op_name] = {
                "start_time": start_time,
                "duration": current_time - start_time,
                "status": "running"
            }

        return active_calls_info

    def get_call_stack_info(self) -> List[Dict[str, Any]]:
        """获取当前线程的调用栈信息"""
        if not hasattr(self._call_stack, 'stack'):
            return []

        current_time = time.time()
        stack_info = []

        for call_info in self._call_stack.stack:
            duration = current_time - call_info['start_time']
            stack_info.append({
                **call_info,
                "duration": duration
            })

        return stack_info

    def cleanup(self):
        """清理资源"""
        try:
            logger.debug("[DEADLOCK_SAFE] Cleaning up resources")

            # 取消所有活动调用
            if self._active_calls:
                logger.warning(f"[DEADLOCK_SAFE] {len(self._active_calls)} active calls during cleanup")

            # 停止后台循环
            if self._loop and not self._loop.is_closed():
                self._loop.call_soon_threadsafe(self._loop.stop)

            # 等待线程结束
            if self._loop_thread and self._loop_thread.is_alive():
                self._loop_thread.join(timeout=2)

            # 关闭线程池
            if self._executor:
                self._executor.shutdown(wait=True, timeout=3)

            logger.debug("[DEADLOCK_SAFE] Cleanup completed")

        except Exception as e:
            logger.error(f"[DEADLOCK_SAFE] Error during cleanup: {e}")

    def __del__(self):
        """析构函数"""
        try:
            self.cleanup()
        except:
            pass


# 全局实例管理
_global_deadlock_safe_helper = None
_helper_lock = threading.Lock()


def get_deadlock_safe_helper() -> DeadlockSafeAsyncHelper:
    """获取全局的死锁安全异步助手实例"""
    global _global_deadlock_safe_helper

    if _global_deadlock_safe_helper is None:
        with _helper_lock:
            if _global_deadlock_safe_helper is None:
                _global_deadlock_safe_helper = DeadlockSafeAsyncHelper()
                logger.debug("Global DeadlockSafeAsyncHelper created")

    return _global_deadlock_safe_helper


def reset_deadlock_safe_helper():
    """重置全局的死锁安全异步助手实例（用于测试）"""
    global _global_deadlock_safe_helper

    with _helper_lock:
        if _global_deadlock_safe_helper is not None:
            _global_deadlock_safe_helper.cleanup()
            _global_deadlock_safe_helper = None

        logger.debug("Global DeadlockSafeAsyncHelper reset")