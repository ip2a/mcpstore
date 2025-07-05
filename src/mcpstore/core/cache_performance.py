#!/usr/bin/env python3
"""
智能缓存与性能优化
工具结果缓存、服务发现缓存、智能预取、连接池管理
"""

import logging
import asyncio
import hashlib
import pickle
import time
from typing import Dict, List, Any, Optional, Union, Callable, Tuple
from dataclasses import dataclass, field
from enum import Enum
from datetime import datetime, timedelta
import weakref
from collections import OrderedDict, defaultdict

logger = logging.getLogger(__name__)

class CacheStrategy(Enum):
    """缓存策略"""
    LRU = "lru"           # 最近最少使用
    LFU = "lfu"           # 最少使用频率
    TTL = "ttl"           # 时间过期
    ADAPTIVE = "adaptive"  # 自适应

@dataclass
class CacheEntry:
    """缓存条目"""
    key: str
    value: Any
    created_at: datetime
    last_accessed: datetime
    access_count: int = 0
    ttl: Optional[int] = None  # 生存时间（秒）
    size: int = 0  # 数据大小（字节）

@dataclass
class CacheStats:
    """缓存统计"""
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
        
        # 如果键已存在，更新统计
        if key in self._cache:
            old_entry = self._cache[key]
            self._stats.total_size -= old_entry.size
        
        self._cache[key] = entry
        self._stats.total_size += size
        self._stats.entry_count = len(self._cache)
    
    def _evict_lru(self):
        """驱逐最近最少使用的条目"""
        if self._cache:
            key, entry = self._cache.popitem(last=False)
            self._stats.total_size -= entry.size
            self._stats.evictions += 1
            logger.debug(f"Evicted LRU cache entry: {key}")
    
    def _evict(self, key: str):
        """驱逐指定条目"""
        if key in self._cache:
            entry = self._cache.pop(key)
            self._stats.total_size -= entry.size
            self._stats.evictions += 1
    
    def _calculate_size(self, value: Any) -> int:
        """计算值的大小"""
        try:
            return len(pickle.dumps(value))
        except:
            return len(str(value).encode('utf-8'))
    
    def clear(self):
        """清空缓存"""
        self._cache.clear()
        self._stats = CacheStats()
    
    def get_stats(self) -> CacheStats:
        """获取缓存统计"""
        self._stats.entry_count = len(self._cache)
        return self._stats

class ToolResultCache:
    """工具结果缓存"""
    
    def __init__(self, max_size: int = 500, default_ttl: int = 3600):
        self.cache = LRUCache(max_size)
        self.default_ttl = default_ttl
        self._cache_patterns: Dict[str, int] = {}  # tool_pattern -> ttl
    
    def get_cache_key(self, tool_name: str, args: Dict[str, Any]) -> str:
        """生成缓存键"""
        # 创建参数的哈希
        args_str = str(sorted(args.items()))
        args_hash = hashlib.md5(args_str.encode()).hexdigest()
        return f"tool:{tool_name}:{args_hash}"
    
    def get_result(self, tool_name: str, args: Dict[str, Any]) -> Optional[Any]:
        """获取缓存的工具结果"""
        cache_key = self.get_cache_key(tool_name, args)
        result = self.cache.get(cache_key)
        
        if result is not None:
            logger.debug(f"Cache hit for tool {tool_name}")
        
        return result
    
    def cache_result(self, tool_name: str, args: Dict[str, Any], result: Any):
        """缓存工具结果"""
        cache_key = self.get_cache_key(tool_name, args)
        ttl = self._get_ttl_for_tool(tool_name)
        
        self.cache.put(cache_key, result, ttl)
        logger.debug(f"Cached result for tool {tool_name} (TTL: {ttl}s)")
    
    def set_tool_cache_pattern(self, tool_pattern: str, ttl: int):
        """设置工具缓存模式"""
        self._cache_patterns[tool_pattern] = ttl
    
    def _get_ttl_for_tool(self, tool_name: str) -> int:
        """获取工具的 TTL"""
        for pattern, ttl in self._cache_patterns.items():
            if pattern in tool_name:
                return ttl
        return self.default_ttl

class ServiceDiscoveryCache:
    """服务发现缓存"""
    
    def __init__(self, ttl: int = 300):  # 5分钟
        self.cache = LRUCache(max_size=100)
        self.ttl = ttl
    
    def get_service_info(self, service_name: str) -> Optional[Dict[str, Any]]:
        """获取服务信息"""
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
    """智能预取管理器"""
    
    def __init__(self):
        self._usage_patterns: Dict[str, List[str]] = defaultdict(list)  # tool -> frequently_used_after
        self._prefetch_queue: asyncio.Queue = asyncio.Queue()
        self._running = False
    
    def record_tool_usage(self, tool_name: str, next_tool: Optional[str] = None):
        """记录工具使用模式"""
        if next_tool:
            patterns = self._usage_patterns[tool_name]
            patterns.append(next_tool)
            
            # 保持最近的100个模式
            if len(patterns) > 100:
                patterns.pop(0)
    
    def get_prefetch_suggestions(self, tool_name: str) -> List[str]:
        """获取预取建议"""
        patterns = self._usage_patterns.get(tool_name, [])
        if not patterns:
            return []
        
        # 统计频率
        frequency = defaultdict(int)
        for next_tool in patterns:
            frequency[next_tool] += 1
        
        # 返回最频繁的工具
        sorted_tools = sorted(frequency.items(), key=lambda x: x[1], reverse=True)
        return [tool for tool, freq in sorted_tools[:3] if freq > 1]
    
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
        self.tool_cache = ToolResultCache()
        self.service_cache = ServiceDiscoveryCache()
        self.prefetch_manager = PrefetchManager()
        self.connection_pool = ConnectionPoolManager()
        self._metrics: Dict[str, Any] = defaultdict(list)
    
    def setup_tool_caching(self, patterns: Dict[str, int] = None):
        """设置工具缓存"""
        default_patterns = {
            "weather": 300,      # 天气数据缓存5分钟
            "news": 600,         # 新闻缓存10分钟
            "search": 1800,      # 搜索结果缓存30分钟
            "translate": 86400,  # 翻译结果缓存1天
        }
        
        patterns = patterns or default_patterns
        for pattern, ttl in patterns.items():
            self.tool_cache.set_tool_cache_pattern(pattern, ttl)
        
        logger.info(f"Configured tool caching with {len(patterns)} patterns")
    
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
        """获取性能摘要"""
        tool_cache_stats = self.tool_cache.cache.get_stats()
        service_cache_stats = self.service_cache.cache.get_stats()
        
        return {
            "tool_cache": {
                "hit_rate": tool_cache_stats.hit_rate,
                "entries": tool_cache_stats.entry_count,
                "memory_usage": tool_cache_stats.total_size
            },
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
