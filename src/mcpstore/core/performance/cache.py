import logging
import pickle
from collections import OrderedDict
from dataclasses import dataclass
from datetime import datetime
from enum import Enum
from typing import Dict, Any, Optional

logger = logging.getLogger(__name__)


class CacheStrategy(Enum):
    """Cache strategies (reserved for future extension)."""
    LRU = "lru"
    LFU = "lfu"
    TTL = "ttl"
    ADAPTIVE = "adaptive"


@dataclass
class CacheEntry:
    key: str
    value: Any
    created_at: datetime
    last_accessed: datetime
    access_count: int = 0
    ttl: Optional[int] = None
    size: int = 0


@dataclass
class CacheStats:
    hits: int = 0
    misses: int = 0
    evictions: int = 0
    total_size: int = 0
    entry_count: int = 0

    @property
    def hit_rate(self) -> float:
        total = self.hits + self.misses
        return self.hits / total if total > 0 else 0.0


class LRUCache:
    """LRU cache with optional TTL per entry and total size accounting."""

    def __init__(self, max_size: int = 1000, max_memory: int = 100 * 1024 * 1024):
        self.max_size = max_size
        self.max_memory = max_memory
        self._cache: OrderedDict[str, CacheEntry] = OrderedDict()
        self._stats = CacheStats()

    def get(self, key: str) -> Optional[Any]:
        if key in self._cache:
            entry = self._cache[key]
            if entry.ttl and (datetime.now() - entry.created_at).total_seconds() > entry.ttl:
                self._evict(key)
                self._stats.misses += 1
                return None

            entry.last_accessed = datetime.now()
            entry.access_count += 1
            self._cache.move_to_end(key)
            self._stats.hits += 1
            return entry.value

        self._stats.misses += 1
        return None

    def put(self, key: str, value: Any, ttl: Optional[int] = None):
        size = self._calculate_size(value)
        while (len(self._cache) >= self.max_size or
               self._stats.total_size + size > self.max_memory):
            if not self._cache:
                break
            self._evict_lru()

        entry = CacheEntry(
            key=key,
            value=value,
            created_at=datetime.now(),
            last_accessed=datetime.now(),
            ttl=ttl,
            size=size,
        )

        if key in self._cache:
            old_entry = self._cache[key]
            self._stats.total_size -= old_entry.size

        self._cache[key] = entry
        self._stats.total_size += size
        self._stats.entry_count = len(self._cache)

    def _evict_lru(self):
        if self._cache:
            key, entry = self._cache.popitem(last=False)
            self._stats.total_size -= entry.size
            self._stats.evictions += 1
            logger.debug(f"Evicted LRU cache entry: {key}")

    def _evict(self, key: str):
        if key in self._cache:
            entry = self._cache.pop(key)
            self._stats.total_size -= entry.size
            self._stats.evictions += 1

    def _calculate_size(self, value: Any) -> int:
        try:
            return len(pickle.dumps(value))
        except Exception:
            return len(str(value).encode('utf-8'))

    def clear(self):
        self._cache.clear()
        self._stats = CacheStats()

    def get_stats(self) -> CacheStats:
        self._stats.entry_count = len(self._cache)
        return self._stats


__all__ = [
    "CacheStrategy",
    "CacheEntry",
    "CacheStats",
    "LRUCache",
]


