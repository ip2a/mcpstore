from collections import defaultdict
from typing import Dict, Any

from .cache import LRUCache
from .discovery_cache import ServiceDiscoveryCache
from .prefetch import PrefetchManager


class ConnectionPoolManager:
    """Simple connection pool skeleton (service-name keyed)."""

    def __init__(self, max_connections: int = 50):
        self.max_connections = max_connections
        # Lazy-initialized per-service pools and counters
        self._pools: Dict[str, Any] = {}
        self._connection_counts: Dict[str, int] = defaultdict(int)

    # Placeholders for future concrete implementations
    async def get_connection(self, service_name: str):  # pragma: no cover - behavior depends on adapters
        return None

    async def return_connection(self, service_name: str, connection: Any):  # pragma: no cover
        return None


class PerformanceOptimizer:
    """Core performance optimizer composed from cache / prefetch / pool."""

    def __init__(self):
        self.service_cache = ServiceDiscoveryCache()
        self.prefetch_manager = PrefetchManager()
        self.connection_pool = ConnectionPoolManager()
        self._metrics: Dict[str, Any] = defaultdict(list)

    def enable_caching(self, patterns: Dict[str, int] = None):
        # Tool result caching is removed; keep method for compatibility
        return True

    def record_tool_execution(self, tool_name: str, execution_time: float, success: bool):
        self._metrics[tool_name].append({
            "execution_time": execution_time,
            "success": success,
        })
        if len(self._metrics[tool_name]) > 100:
            self._metrics[tool_name].pop(0)

    def get_performance_summary(self) -> Dict[str, Any]:
        service_cache_stats = self.service_cache.cache.get_stats()
        return {
            "service_cache": {
                "hit_rate": service_cache_stats.hit_rate,
                "entries": service_cache_stats.entry_count,
            },
            "connection_pools": dict(self.connection_pool._connection_counts),
            "tool_metrics": {
                tool: {
                    "avg_execution_time": sum(m["execution_time"] for m in metrics) / len(metrics),
                    "success_rate": sum(1 for m in metrics if m["success"]) / len(metrics),
                    "total_calls": len(metrics),
                }
                for tool, metrics in self._metrics.items() if metrics
            },
        }


_global_performance_optimizer = None


def get_performance_optimizer() -> PerformanceOptimizer:
    global _global_performance_optimizer
    if _global_performance_optimizer is None:
        _global_performance_optimizer = PerformanceOptimizer()
    return _global_performance_optimizer


__all__ = [
    "LRUCache",
    "ServiceDiscoveryCache",
    "PrefetchManager",
    "ConnectionPoolManager",
    "PerformanceOptimizer",
    "get_performance_optimizer",
]


