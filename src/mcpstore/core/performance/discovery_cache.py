from typing import Dict, List, Any, Optional

from .cache import LRUCache


class ServiceDiscoveryCache:
    """Cache for service info and tool lists with TTL semantics."""

    def __init__(self, ttl: int = 300):  # 5 minutes
        self.cache = LRUCache(max_size=100)
        self.ttl = ttl

    def get_service_info(self, service_name: str) -> Optional[Dict[str, Any]]:
        return self.cache.get(f"service:{service_name}")

    def cache_service_info(self, service_name: str, service_info: Dict[str, Any]):
        self.cache.put(f"service:{service_name}", service_info, self.ttl)

    def get_tools_for_service(self, service_name: str) -> Optional[List[Dict[str, Any]]]:
        return self.cache.get(f"tools:{service_name}")

    def cache_tools_for_service(self, service_name: str, tools: List[Dict[str, Any]]):
        self.cache.put(f"tools:{service_name}", tools, self.ttl)


__all__ = [
    "ServiceDiscoveryCache",
]


