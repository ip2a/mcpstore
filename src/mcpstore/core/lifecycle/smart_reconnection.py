"""
Smart Reconnection Manager
Implements exponential backoff reconnection strategy with support for reconnection priority and failure counting
"""

import logging
from dataclasses import dataclass
from datetime import datetime, timedelta
from enum import Enum
from typing import Dict, Set, Optional

logger = logging.getLogger(__name__)


class ReconnectionPriority(Enum):
    """Reconnection priority"""
    LOW = 1      # Low priority: non-critical services
    NORMAL = 2   # Normal priority: general services
    HIGH = 3     # High priority: important services
    CRITICAL = 4 # Critical priority: core services


@dataclass
class ReconnectionEntry:
    """Reconnection entry"""
    service_key: str                    # Service key (client_id:service_name)
    client_id: str                      # Client ID
    service_name: str                   # Service name
    priority: ReconnectionPriority      # Reconnection priority
    failure_count: int = 0              # Failure count
    last_attempt: Optional[datetime] = None  # Last attempt time
    next_attempt: Optional[datetime] = None  # 下次尝试时间
    created_at: datetime = None         # 创建时间
    
    def __post_init__(self):
        if self.created_at is None:
            self.created_at = datetime.now()


class SmartReconnectionManager:
    """智能重连管理器"""
    
    def __init__(self):
        self.entries: Dict[str, ReconnectionEntry] = {}
        
        # 重连策略配置
        self.base_delay_seconds = 60        # 基础延迟：1分钟
        self.max_delay_seconds = 600        # 最大延迟：10分钟
        self.max_failure_count = 10         # 最大失败次数
        self.cleanup_interval_hours = 24    # 清理间隔：24小时
        
        # 优先级权重（影响重连间隔）
        self.priority_weights = {
            ReconnectionPriority.CRITICAL: 0.5,  # 关键服务：更快重连
            ReconnectionPriority.HIGH: 0.7,      # 高优先级：较快重连
            ReconnectionPriority.NORMAL: 1.0,    # 普通优先级：标准重连
            ReconnectionPriority.LOW: 1.5        # 低优先级：较慢重连
        }
    
    def add_service(self, client_id: str, service_name: str, 
                   priority: ReconnectionPriority = ReconnectionPriority.NORMAL) -> str:
        """添加服务到重连队列"""
        service_key = f"{client_id}:{service_name}"
        
        if service_key in self.entries:
            # 如果已存在，增加失败计数
            entry = self.entries[service_key]
            entry.failure_count += 1
            self._calculate_next_attempt(entry)
            logger.debug(f"Updated reconnection entry for {service_key}, failure_count: {entry.failure_count}")
        else:
            # 创建新条目
            entry = ReconnectionEntry(
                service_key=service_key,
                client_id=client_id,
                service_name=service_name,
                priority=priority
            )
            self._calculate_next_attempt(entry)
            self.entries[service_key] = entry
            logger.info(f"Added new reconnection entry for {service_key} with priority {priority.name}")
        
        return service_key
    
    def remove_service(self, service_key: str) -> bool:
        """从重连队列中移除服务"""
        if service_key in self.entries:
            del self.entries[service_key]
            logger.info(f"Removed reconnection entry for {service_key}")
            return True
        return False
    
    def mark_success(self, service_key: str) -> bool:
        """标记服务重连成功"""
        return self.remove_service(service_key)
    
    def mark_failure(self, service_key: str) -> bool:
        """标记服务重连失败"""
        if service_key in self.entries:
            entry = self.entries[service_key]
            entry.failure_count += 1
            entry.last_attempt = datetime.now()
            
            # 检查是否超过最大失败次数
            if entry.failure_count >= self.max_failure_count:
                logger.warning(f"Service {service_key} exceeded max failure count ({self.max_failure_count}), removing from queue")
                self.remove_service(service_key)
                return False
            
            # 重新计算下次尝试时间
            self._calculate_next_attempt(entry)
            logger.debug(f"Marked failure for {service_key}, failure_count: {entry.failure_count}, next_attempt: {entry.next_attempt}")
            return True
        return False
    
    def get_services_ready_for_retry(self) -> list[ReconnectionEntry]:
        """获取准备重试的服务列表（按优先级排序）"""
        now = datetime.now()
        ready_services = []
        
        for entry in self.entries.values():
            if entry.next_attempt and entry.next_attempt <= now:
                ready_services.append(entry)
        
        # 按优先级排序（优先级高的先重连）
        ready_services.sort(key=lambda x: (x.priority.value, x.failure_count), reverse=True)
        
        return ready_services
    
    def get_queue_status(self) -> Dict:
        """获取重连队列状态"""
        now = datetime.now()
        status = {
            "total_entries": len(self.entries),
            "ready_for_retry": 0,
            "by_priority": {priority.name: 0 for priority in ReconnectionPriority},
            "by_failure_count": {},
            "oldest_entry": None,
            "next_retry_time": None
        }
        
        next_retry_times = []
        
        for entry in self.entries.values():
            # 统计优先级分布
            status["by_priority"][entry.priority.name] += 1
            
            # 统计失败次数分布
            failure_key = f"{entry.failure_count}_failures"
            status["by_failure_count"][failure_key] = status["by_failure_count"].get(failure_key, 0) + 1
            
            # 检查是否准备重试
            if entry.next_attempt and entry.next_attempt <= now:
                status["ready_for_retry"] += 1
            
            # 收集下次重试时间
            if entry.next_attempt:
                next_retry_times.append(entry.next_attempt)
            
            # 找到最旧的条目
            if status["oldest_entry"] is None or entry.created_at < status["oldest_entry"]:
                status["oldest_entry"] = entry.created_at
        
        # 找到最近的重试时间
        if next_retry_times:
            status["next_retry_time"] = min(next_retry_times)
        
        return status
    
    def cleanup_expired_entries(self) -> int:
        """清理过期的重连条目"""
        cutoff_time = datetime.now() - timedelta(hours=self.cleanup_interval_hours)
        expired_keys = []
        
        for service_key, entry in self.entries.items():
            if entry.created_at < cutoff_time:
                expired_keys.append(service_key)
        
        for key in expired_keys:
            del self.entries[key]
        
        if expired_keys:
            logger.info(f"Cleaned up {len(expired_keys)} expired reconnection entries")
        
        return len(expired_keys)
    
    def cleanup_invalid_clients(self, valid_client_ids: Set[str]) -> int:
        """清理无效客户端的重连条目"""
        invalid_keys = []
        
        for service_key, entry in self.entries.items():
            if entry.client_id not in valid_client_ids:
                invalid_keys.append(service_key)
        
        for key in invalid_keys:
            del self.entries[key]
        
        if invalid_keys:
            logger.info(f"Cleaned up {len(invalid_keys)} reconnection entries for invalid clients")
        
        return len(invalid_keys)
    
    def _calculate_next_attempt(self, entry: ReconnectionEntry):
        """计算下次尝试时间（指数退避）"""
        # 基础延迟 * 2^失败次数 * 优先级权重
        delay_seconds = min(
            self.base_delay_seconds * (2 ** entry.failure_count) * self.priority_weights[entry.priority],
            self.max_delay_seconds
        )
        
        entry.next_attempt = datetime.now() + timedelta(seconds=delay_seconds)
        entry.last_attempt = datetime.now()
        
        logger.debug(f"Calculated next attempt for {entry.service_key}: {entry.next_attempt} "
                    f"(delay: {delay_seconds}s, failures: {entry.failure_count}, priority: {entry.priority.name})")
    
    def _infer_service_priority(self, service_name: str) -> ReconnectionPriority:
        """根据服务名称推断优先级"""
        service_name_lower = service_name.lower()
        
        # 关键服务
        if any(keyword in service_name_lower for keyword in ['auth', 'security', 'core', 'main']):
            return ReconnectionPriority.CRITICAL
        
        # 高优先级服务
        if any(keyword in service_name_lower for keyword in ['api', 'gateway', 'proxy']):
            return ReconnectionPriority.HIGH
        
        # 低优先级服务
        if any(keyword in service_name_lower for keyword in ['test', 'debug', 'temp', 'sample']):
            return ReconnectionPriority.LOW
        
        # 默认普通优先级
        return ReconnectionPriority.NORMAL
