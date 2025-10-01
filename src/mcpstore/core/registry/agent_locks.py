import asyncio
from contextlib import asynccontextmanager
from typing import Dict, AsyncIterator


class AgentLocks:
    """
    Per-agent async RW-like lock (write-only for now).
    Provides a simple write lock to serialize multi-step cache updates for a given agent_id.
    """

    def __init__(self) -> None:
        self._locks: Dict[str, asyncio.Lock] = {}
        self._global_lock = asyncio.Lock()

    def _get_lock(self, agent_id: str) -> asyncio.Lock:
        # Double-checked pattern to avoid global contention
        lock = self._locks.get(agent_id)
        if lock is not None:
            return lock
        # Create lazily and store safely
        # The cost is negligible and only on first access per agent
        async def create() -> asyncio.Lock:
            async with self._global_lock:
                if agent_id not in self._locks:
                    self._locks[agent_id] = asyncio.Lock()
                return self._locks[agent_id]
        # We can't call async here; but we can do a best-effort non-atomic fallback.
        # Callers should prefer using `write()` which ensures creation via _ensure().
        return self._locks.setdefault(agent_id, asyncio.Lock())

    async def _ensure(self, agent_id: str) -> asyncio.Lock:
        async with self._global_lock:
            if agent_id not in self._locks:
                self._locks[agent_id] = asyncio.Lock()
            return self._locks[agent_id]

    @asynccontextmanager
    async def write(self, agent_id: str) -> AsyncIterator[None]:
        """
        Usage:
            async with locks.write(agent_id):
                ...  # multi-step cache updates
        """
        lock = await self._ensure(agent_id)
        async with lock:
            yield

