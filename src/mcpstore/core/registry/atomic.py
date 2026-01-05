"""
Atomic write utilities for MCPStore cache backends.

Provides:
- @atomic_write decorator (async + sync) that wraps a method with:
  * optional per-agent write lock
  * backend.begin()/commit()/rollback()
- Async/sync context managers for manual composition

Design goals:
- Zero coupling to Redis. Works with any CacheBackend implementation
- Require only that the wrapped method's `self` provides either
  * self.cache_backend, or
  * self.registry.cache_backend
- Agent-level isolation via per-agent locks (asyncio for async, threading for sync)
"""
from __future__ import annotations

import asyncio
import functools
import inspect
import threading
from typing import Any, Callable, Optional, Dict


class AtomicWriteError(RuntimeError):
    pass


class AtomicWriteLocks:
    """Async per-agent locks.

    Stored on an owning object as `._atomic_write_locks`.

    NOTE: This class is now DEPRECATED in favor of AgentLocks.
    It's kept for backward compatibility but should not be used in new code.
    """

    def __init__(self) -> None:
        self._locks: Dict[str, asyncio.Lock] = {}
        # FIX: Use threading.Lock instead of asyncio.Lock for thread-safe creation
        self._global_lock = threading.Lock()

    def get(self, agent_id: str) -> asyncio.Lock:
        # Fast path if present
        lock = self._locks.get(agent_id)
        if lock is not None:
            return lock

        # FIX: Use threading lock to avoid deadlock when called from running event loop
        # This is safe because we're only protecting the dictionary mutation, not async operations
        with self._global_lock:
            # Double-check pattern
            lk = self._locks.get(agent_id)
            if lk is None:
                lk = asyncio.Lock()
                self._locks[agent_id] = lk
            return lk


class ThreadWriteLocks:
    """Sync per-agent locks based on threading.Lock.

    Stored on an owning object as `._atomic_write_thread_locks`.
    """

    def __init__(self) -> None:
        self._locks: Dict[str, threading.Lock] = {}
        self._global = threading.Lock()

    def get(self, agent_id: str) -> threading.Lock:
        lk = self._locks.get(agent_id)
        if lk is not None:
            return lk
        with self._global:
            lk = self._locks.get(agent_id)
            if lk is None:
                lk = threading.Lock()
                self._locks[agent_id] = lk
            return lk


def _resolve_backend(owner: Any):
    """Try to resolve a CacheBackend from an owner object."""
    be = getattr(owner, "cache_backend", None)
    if be is not None:
        return be
    registry = getattr(owner, "registry", None)
    if registry is not None:
        be = getattr(registry, "cache_backend", None)
        if be is not None:
            return be
    raise AtomicWriteError("atomic_write: cannot resolve cache_backend from owner. Expected 'self.cache_backend' or 'self.registry.cache_backend'.")


def _resolve_agent_id(fn: Callable[..., Any], args: tuple, kwargs: dict, param_name: str) -> Optional[str]:
    """Extract agent_id from function arguments by name."""
    try:
        sig = inspect.signature(fn)
        bound = sig.bind_partial(*args, **kwargs)
        bound.apply_defaults()
        if param_name in bound.arguments:
            return bound.arguments[param_name]
    except Exception:
        pass
    return kwargs.get(param_name)


def atomic_write(agent_id_param: str = "agent_id", use_lock: bool = True):
    """Decorator to make a method execute as an atomic write transaction.

    Behavior:
    - Resolve backend from `self.cache_backend` or `self.registry.cache_backend`
    - Optionally acquire per-agent write lock keyed by agent_id
    - Call backend.begin(); execute the function; backend.commit(); on error backend.rollback()

    Works with both async and sync methods.

    IMPORTANT: When use_lock=True for sync methods called from async contexts,
    the decorator will skip internal locking and rely on external AgentLocks to avoid deadlock.
    """

    def decorator(fn: Callable[..., Any]) -> Callable[..., Any]:
        is_async = inspect.iscoroutinefunction(fn)

        if is_async:
            @functools.wraps(fn)
            async def async_wrapper(*args, **kwargs):
                if not args:
                    raise AtomicWriteError("@atomic_write must decorate a bound method (expects 'self' as first arg)")
                owner = args[0]
                backend = _resolve_backend(owner)

                agent_id = _resolve_agent_id(fn, args, kwargs, agent_id_param)
                if use_lock and agent_id:
                    locks: AtomicWriteLocks = getattr(owner, "_atomic_write_locks", None)  # type: ignore[assignment]
                    if locks is None:
                        locks = AtomicWriteLocks()
                        setattr(owner, "_atomic_write_locks", locks)
                    lock = locks.get(str(agent_id))
                else:
                    lock = None

                async def _do():
                    backend.begin()
                    try:
                        result = await fn(*args, **kwargs)
                        backend.commit()
                        return result
                    except Exception:
                        try:
                            backend.rollback()
                        finally:
                            pass
                        raise

                if lock is None:
                    return await _do()
                async with lock:
                    return await _do()

            return async_wrapper

        else:
            @functools.wraps(fn)
            def sync_wrapper(*args, **kwargs):
                if not args:
                    raise AtomicWriteError("@atomic_write must decorate a bound method (expects 'self' as first arg)")
                owner = args[0]
                backend = _resolve_backend(owner)

                agent_id = _resolve_agent_id(fn, args, kwargs, agent_id_param)

                # FIX: Skip internal locking for sync methods to avoid deadlock
                # when called from async contexts that already hold AgentLocks
                lock = None
                if use_lock and agent_id:
                    # Check if we're being called from an async context
                    try:
                        asyncio.get_running_loop()
                        # We're in an async context - assume external AgentLocks are used
                        # Skip internal threading lock to avoid deadlock
                        lock = None
                    except RuntimeError:
                        # No running loop - safe to use threading locks
                        tlocks: ThreadWriteLocks = getattr(owner, "_atomic_write_thread_locks", None)  # type: ignore[assignment]
                        if tlocks is None:
                            tlocks = ThreadWriteLocks()
                            setattr(owner, "_atomic_write_thread_locks", tlocks)
                        lock = tlocks.get(str(agent_id))

                def _do():
                    backend.begin()
                    try:
                        result = fn(*args, **kwargs)
                        backend.commit()
                        return result
                    except Exception:
                        try:
                            backend.rollback()
                        finally:
                            pass
                        raise

                if lock is None:
                    return _do()
                with lock:
                    return _do()

            return sync_wrapper

    return decorator




