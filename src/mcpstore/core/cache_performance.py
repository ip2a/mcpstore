#!/usr/bin/env python3
"""
Intelligent Caching and Performance Optimization
Tool result caching, service discovery caching, intelligent prefetching, connection pool management
"""

import asyncio
import hashlib
import logging
import pickle
from collections import OrderedDict, defaultdict
from dataclasses import dataclass
from datetime import datetime
from enum import Enum
from typing import Dict, List, Any, Optional

logger = logging.getLogger(__name__)

class CacheStrategy(Enum):
    """Cache strategies"""
    LRU = "lru"           # Least Recently Used
    LFU = "lfu"           # Least Frequently Used
    TTL = "ttl"           # Time To Live
    ADAPTIVE = "adaptive"  # Adaptive

@dataclass
class CacheEntry:
    """Cache entry"""
    key: str
    value: Any
    created_at: datetime
    last_accessed: datetime
    access_count: int = 0
    ttl: Optional[int] = None  # Time to live (seconds)
    size: int = 0  # Data size (bytes)

@dataclass
class CacheStats:
    """Cache statistics"""
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
    """LRU 缓存实现"""
    
    def __init__(self, max_size: int = 1000, max_memory: int = 100 * 1024 * 1024):  # 100MB
        self.max_size = max_size
        self.max_memory = max_memory
        self._cache: OrderedDict[str, CacheEntry] = OrderedDict()
        self._stats = CacheStats()
    
    def get(self, key: str) -> Optional[Any]:
        """获取缓存值"""
        if key in self._cache:
            entry = self._cache[key]
            
            # 检查 TTL
            if entry.ttl and (datetime.now() - entry.created_at).total_seconds() > entry.ttl:
                self._evict(key)
                self._stats.misses += 1
                return None
            
            # 更新访问信息
            entry.last_accessed = datetime.now()
            entry.access_count += 1
            
            # 移到末尾（最近使用）
            self._cache.move_to_end(key)
            
            self._stats.hits += 1
            return entry.value
        
        self._stats.misses += 1
        return None
    
    def put(self, key: str, value: Any, ttl: Optional[int] = None):
        """存储缓存值"""
        # 计算数据大小
        size = self._calculate_size(value)
        
        # 检查是否需要清理空间
        while (len(self._cache) >= self.max_size or 
               self._stats.total_size + size > self.max_memory):
            if not self._cache:
                break
            self._evict_lru()
        
        # 创建缓存条目
        entry = CacheEntry(
            key=key,
            value=value,
            created_at=datetime.now(),
            last_accessed=datetime.now(),
            ttl=ttl,
            size=size
        )
        
        # If key already exists, update statistics
        if key in self._cache:
            old_entry = self._cache[key]
            self._stats.total_size -= old_entry.size
        
        self._cache[key] = entry
        self._stats.total_size += size
        self._stats.entry_count = len(self._cache)
    
    def _evict_lru(self):
        """Evict least recently used entry"""
        if self._cache:
            key, entry = self._cache.popitem(last=False)
            self._stats.total_size -= entry.size
            self._stats.evictions += 1
            logger.debug(f"Evicted LRU cache entry: {key}")
    
    def _evict(self, key: str):
        """Evict specified entry"""
        if key in self._cache:
            entry = self._cache.pop(key)
            self._stats.total_size -= entry.size
            self._stats.evictions += 1
    
    def _calculate_size(self, value: Any) -> int:
        """Calculate size of value"""
        try:
            return len(pickle.dumps(value))
        except:
            return len(str(value).encode('utf-8'))
    
    def clear(self):
        """Clear cache"""
        self._cache.clear()
        self._stats = CacheStats()
    
    def get_stats(self) -> CacheStats:
        """Get cache statistics"""
        self._stats.entry_count = len(self._cache)
        return self._stats

# ToolResultCache 类已移除 - 不再支持工具结果缓存功能

class ServiceDiscoveryCache:
    """Service discovery cache"""

    def __init__(self, ttl: int = 300):  # 5 minutes
        self.cache = LRUCache(max_size=100)
        self.ttl = ttl
    
    def get_service_info(self, service_name: str) -> Optional[Dict[str, Any]]:
        """Get service information"""
        return self.cache.get(f"service:{service_name}")
    
    def cache_service_info(self, service_name: str, service_info: Dict[str, Any]):
        """缓存服务信息"""
        self.cache.put(f"service:{service_name}", service_info, self.ttl)
    
    def get_tools_for_service(self, service_name: str) -> Optional[List[Dict[str, Any]]]:
        """获取服务的工具列表"""
        return self.cache.get(f"tools:{service_name}")
    
    def cache_tools_for_service(self, service_name: str, tools: List[Dict[str, Any]]):
        """缓存服务的工具列表"""
        self.cache.put(f"tools:{service_name}", tools, self.ttl)

class PrefetchManager:
    """智能预取管理器（简化版，移除工具使用模式记录）"""

    def __init__(self):
        self._prefetch_queue: asyncio.Queue = asyncio.Queue()
        self._running = False


    
    async def start_prefetch_worker(self):
        """启动预取工作器"""
        self._running = True
        while self._running:
            try:
                prefetch_task = await asyncio.wait_for(
                    self._prefetch_queue.get(), timeout=1.0
                )
                await self._execute_prefetch(prefetch_task)
            except asyncio.TimeoutError:
                continue
            except Exception as e:
                logger.error(f"Prefetch error: {e}")
    
    def stop_prefetch_worker(self):
        """停止预取工作器"""
        self._running = False
    
    async def _execute_prefetch(self, task: Dict[str, Any]):
        """执行预取任务"""
        # 这里可以实现具体的预取逻辑
        logger.debug(f"Executing prefetch task: {task}")

class ConnectionPoolManager:
    """连接池管理器"""
    
    def __init__(self, max_connections: int = 50):
        self.max_connections = max_connections
        self._pools: Dict[str, asyncio.Queue] = {}
        self._connection_counts: Dict[str, int] = defaultdict(int)
        self._lock = asyncio.Lock()
    
    async def get_connection(self, service_name: str) -> Optional[Any]:
        """获取连接"""
        async with self._lock:
            if service_name not in self._pools:
                self._pools[service_name] = asyncio.Queue(maxsize=self.max_connections)
            
            pool = self._pools[service_name]
            
            try:
                # 尝试从池中获取连接
                connection = pool.get_nowait()
                logger.debug(f"Reused connection for service {service_name}")
                return connection
            except asyncio.QueueEmpty:
                # 创建新连接
                if self._connection_counts[service_name] < self.max_connections:
                    connection = await self._create_connection(service_name)
                    if connection:
                        self._connection_counts[service_name] += 1
                        logger.debug(f"Created new connection for service {service_name}")
                        return connection
                
                logger.warning(f"Connection pool exhausted for service {service_name}")
                return None
    
    async def return_connection(self, service_name: str, connection: Any):
        """归还连接"""
        if service_name in self._pools:
            pool = self._pools[service_name]
            try:
                pool.put_nowait(connection)
                logger.debug(f"Returned connection for service {service_name}")
            except asyncio.QueueFull:
                # 池已满，关闭连接
                await self._close_connection(connection)
                self._connection_counts[service_name] -= 1
    
    async def _create_connection(self, service_name: str) -> Optional[Any]:
        """创建连接（需要子类实现）"""
        # 这里应该根据服务类型创建相应的连接
        return None
    
    async def _close_connection(self, connection: Any):
        """关闭连接（需要子类实现）"""
        pass

class PerformanceOptimizer:
    """性能优化器"""

    def __init__(self):
        # 移除工具结果缓存功能
        self.service_cache = ServiceDiscoveryCache()
        self.prefetch_manager = PrefetchManager()
        self.connection_pool = ConnectionPoolManager()
        self._metrics: Dict[str, Any] = defaultdict(list)

    def enable_caching(self, patterns: Dict[str, int] = None):
        """启用缓存（工具结果缓存已移除，仅保留服务发现缓存）"""
        logger.info("Tool result caching has been removed. Only service discovery caching is available.")
        return True
    
    def record_tool_execution(self, tool_name: str, execution_time: float, success: bool):
        """记录工具执行指标"""
        self._metrics[tool_name].append({
            "execution_time": execution_time,
            "success": success,
            "timestamp": datetime.now()
        })
        
        # 保持最近的100条记录
        if len(self._metrics[tool_name]) > 100:
            self._metrics[tool_name].pop(0)
    
    def get_performance_summary(self) -> Dict[str, Any]:
        """获取性能摘要（已移除工具结果缓存统计）"""
        service_cache_stats = self.service_cache.cache.get_stats()

        return {
            "service_cache": {
                "hit_rate": service_cache_stats.hit_rate,
                "entries": service_cache_stats.entry_count
            },
            "connection_pools": {
                service: count for service, count in self.connection_pool._connection_counts.items()
            },
            "tool_metrics": {
                tool: {
                    "avg_execution_time": sum(m["execution_time"] for m in metrics) / len(metrics),
                    "success_rate": sum(1 for m in metrics if m["success"]) / len(metrics),
                    "total_calls": len(metrics)
                }
                for tool, metrics in self._metrics.items() if metrics
            }
        }

# 全局实例
_global_performance_optimizer = None

def get_performance_optimizer() -> PerformanceOptimizer:
    """获取全局性能优化器"""
    global _global_performance_optimizer
    if _global_performance_optimizer is None:
        _global_performance_optimizer = PerformanceOptimizer()
    return _global_performance_optimizer
