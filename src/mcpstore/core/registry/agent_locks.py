"""
AgentLocks - Per-agent 异步读写锁

提供细粒度的并发控制，确保同一 agent 的操作串行执行，
不同 agent 的操作可以并行执行。

设计原则：
1. 每个 agent_id 拥有独立的锁，避免全局锁竞争
2. 支持读写锁语义（当前实现为写锁，可扩展为读写锁）
3. 提供诊断能力，便于排查死锁和性能问题
4. 懒加载锁实例，避免内存浪费
"""

import asyncio
import logging
import time
from contextlib import asynccontextmanager
from dataclasses import dataclass
from typing import Dict, AsyncIterator, Optional, Set

from mcpstore.core.bridge import get_async_bridge

logger = logging.getLogger(__name__)


@dataclass
class LockStats:
    """锁统计信息"""
    agent_id: str
    acquired_count: int = 0
    total_wait_time_ms: float = 0.0
    max_wait_time_ms: float = 0.0
    current_holder: Optional[str] = None
    waiting_count: int = 0


@dataclass
class LockContext:
    """锁上下文信息，用于诊断"""
    agent_id: str
    operation: str
    acquired_at: float
    caller: str = ""


class AgentLocks:
    """
    Per-agent 异步锁管理器
    
    提供细粒度的并发控制：
    - 同一 agent_id 的操作串行执行
    - 不同 agent_id 的操作可以并行执行
    - 支持诊断和监控
    
    使用示例：
        async with locks.write(agent_id, operation="update_cache"):
            # 多步骤缓存更新操作
            await step1()
            await step2()
    """

    def __init__(self, enable_diagnostics: bool = True) -> None:
        """
        初始化锁管理器
        
        Args:
            enable_diagnostics: 是否启用诊断功能（记录等待时间、持有者等）
        """
        self._bridge = get_async_bridge()
        self._bridge_loop = getattr(self._bridge, "_loop", None)
        # 每个 agent_id 对应一个锁
        self._locks: Dict[str, asyncio.Lock] = {}
        # 全局锁，用于保护 _locks 字典的创建
        self._global_lock = asyncio.Lock()
        # 诊断开关
        self._enable_diagnostics = enable_diagnostics
        # 锁统计信息
        self._stats: Dict[str, LockStats] = {}
        # 当前持有锁的上下文
        self._active_contexts: Dict[str, LockContext] = {}
        # 等待锁的操作集合
        self._waiting: Dict[str, Set[str]] = {}
        
        logger.debug("[AgentLocks] Initialization completed, diagnostics enabled: %s", enable_diagnostics)

    async def _ensure_lock(self, agent_id: str) -> asyncio.Lock:
        """
        确保指定 agent_id 的锁存在（懒加载）
        
        Args:
            agent_id: Agent ID
            
        Returns:
            对应的 asyncio.Lock 实例
        """
        # 快速路径：锁已存在
        lock = self._locks.get(agent_id)
        if lock is not None:
            return lock
        
        # 慢路径：需要创建锁
        async with self._global_lock:
            # 双重检查
            if agent_id not in self._locks:
                # 在有事件循环的线程直接创建锁，避免在运行中的 loop 上调用 bridge.run 触发 RuntimeError
                try:
                    running_loop = asyncio.get_running_loop()
                except RuntimeError:
                    running_loop = None

                if running_loop:
                    lock = asyncio.Lock()
                elif self._bridge_loop and self._bridge_loop.is_running():
                    async def _create_lock():
                        return asyncio.Lock()
                    lock = self._bridge.run(_create_lock(), op_name="agent_locks.create_lock")
                else:
                    lock = asyncio.Lock()
                self._locks[agent_id] = lock
                if self._enable_diagnostics:
                    self._stats[agent_id] = LockStats(agent_id=agent_id)
                    self._waiting[agent_id] = set()
                logger.debug("[AgentLocks] Creating new lock for agent_id=%s", agent_id)
            return self._locks[agent_id]

    @asynccontextmanager
    async def write(
        self, 
        agent_id: str, 
        operation: str = "unknown",
        timeout: Optional[float] = None
    ) -> AsyncIterator[None]:
        """
        获取写锁（独占锁）
        
        Args:
            agent_id: Agent ID
            operation: 操作名称（用于诊断）
            timeout: 超时时间（秒），None 表示无限等待
            
        Yields:
            None
            
        Raises:
            asyncio.TimeoutError: 如果指定了 timeout 且超时
            
        使用示例：
            async with locks.write(agent_id, operation="update_service_status"):
                await update_status()
        """
        lock = await self._ensure_lock(agent_id)
        start_time = time.monotonic()
        operation_id = f"{operation}_{id(asyncio.current_task())}"
        
        # 记录等待状态
        if self._enable_diagnostics:
            self._waiting.setdefault(agent_id, set()).add(operation_id)
            if self._stats.get(agent_id):
                self._stats[agent_id].waiting_count = len(self._waiting[agent_id])
        
        try:
            # 获取锁（支持超时）
            async def _acquire():
                if timeout is not None:
                    await asyncio.wait_for(lock.acquire(), timeout=timeout)
                else:
                    await lock.acquire()

            # 在锁所属的桥接 loop 上执行，避免跨 loop 错误
            try:
                running_loop = asyncio.get_running_loop()
            except RuntimeError:
                running_loop = None

            if self._bridge_loop and running_loop is not self._bridge_loop:
                await asyncio.to_thread(
                    self._bridge.run,
                    _acquire(),
                    op_name=f"agent_locks.acquire.{agent_id}"
                )
            else:
                await _acquire()
            
            # 记录诊断信息
            wait_time_ms = (time.monotonic() - start_time) * 1000
            if self._enable_diagnostics:
                self._waiting[agent_id].discard(operation_id)
                stats = self._stats.get(agent_id)
                if stats:
                    stats.acquired_count += 1
                    stats.total_wait_time_ms += wait_time_ms
                    stats.max_wait_time_ms = max(stats.max_wait_time_ms, wait_time_ms)
                    stats.current_holder = operation
                    stats.waiting_count = len(self._waiting[agent_id])
                
                self._active_contexts[agent_id] = LockContext(
                    agent_id=agent_id,
                    operation=operation,
                    acquired_at=time.monotonic()
                )
            
            # 如果等待时间过长，记录警告
            if wait_time_ms > 100:  # 超过 100ms
                logger.warning(
                    "[AgentLocks] Lock wait time too long: agent_id=%s, operation=%s, wait_time=%.2fms",
                    agent_id, operation, wait_time_ms
                )
            else:
                logger.debug(
                    "[AgentLocks] Lock acquired successfully: agent_id=%s, operation=%s, wait_time=%.2fms",
                    agent_id, operation, wait_time_ms
                )
            
            yield
            
        finally:
            # 释放锁
            async def _release():
                lock.release()

            try:
                running_loop = asyncio.get_running_loop()
            except RuntimeError:
                running_loop = None

            if self._bridge_loop and running_loop is not self._bridge_loop:
                await asyncio.to_thread(
                    self._bridge.run,
                    _release(),
                    op_name=f"agent_locks.release.{agent_id}"
                )
            else:
                await _release()
            
            # 清理诊断信息
            if self._enable_diagnostics:
                self._waiting.get(agent_id, set()).discard(operation_id)
                if agent_id in self._active_contexts:
                    ctx = self._active_contexts.pop(agent_id)
                    hold_time_ms = (time.monotonic() - ctx.acquired_at) * 1000
                    if hold_time_ms > 500:  # 持有超过 500ms
                        logger.warning(
                            "[AgentLocks] Lock hold time too long: agent_id=%s, operation=%s, hold_time=%.2fms",
                            agent_id, operation, hold_time_ms
                        )
                
                stats = self._stats.get(agent_id)
                if stats:
                    stats.current_holder = None
                    stats.waiting_count = len(self._waiting.get(agent_id, set()))
            
            logger.debug("[AgentLocks] Releasing lock: agent_id=%s, operation=%s", agent_id, operation)

    def get_stats(self, agent_id: Optional[str] = None) -> Dict[str, LockStats]:
        """
        获取锁统计信息
        
        Args:
            agent_id: 指定 agent_id，None 表示获取所有
            
        Returns:
            锁统计信息字典
        """
        if not self._enable_diagnostics:
            return {}
        
        if agent_id:
            stats = self._stats.get(agent_id)
            return {agent_id: stats} if stats else {}
        
        return dict(self._stats)

    def get_active_locks(self) -> Dict[str, LockContext]:
        """
        获取当前持有的锁信息
        
        Returns:
            当前活跃的锁上下文字典
        """
        if not self._enable_diagnostics:
            return {}
        return dict(self._active_contexts)

    def is_locked(self, agent_id: str) -> bool:
        """
        检查指定 agent_id 的锁是否被持有
        
        Args:
            agent_id: Agent ID
            
        Returns:
            True 如果锁被持有
        """
        lock = self._locks.get(agent_id)
        return lock is not None and lock.locked()

    def get_waiting_count(self, agent_id: str) -> int:
        """
        获取等待指定锁的操作数量
        
        Args:
            agent_id: Agent ID
            
        Returns:
            等待中的操作数量
        """
        return len(self._waiting.get(agent_id, set()))

    def cleanup(self, agent_id: str) -> None:
        """
        清理指定 agent_id 的锁资源
        
        注意：只有在确定该 agent 不再使用时才调用
        
        Args:
            agent_id: Agent ID
        """
        lock = self._locks.pop(agent_id, None)
        if lock and lock.locked():
            logger.warning(
                "[AgentLocks] Lock still held during cleanup: agent_id=%s",
                agent_id
            )
        
        self._stats.pop(agent_id, None)
        self._active_contexts.pop(agent_id, None)
        self._waiting.pop(agent_id, None)
        
        logger.debug("[AgentLocks] Cleaning up lock resources: agent_id=%s", agent_id)

    def __repr__(self) -> str:
        active_count = sum(1 for lock in self._locks.values() if lock.locked())
        return (
            f"AgentLocks(total={len(self._locks)}, "
            f"active={active_count}, "
            f"diagnostics={self._enable_diagnostics})"
        )
