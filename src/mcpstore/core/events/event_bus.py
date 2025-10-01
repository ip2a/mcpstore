"""
事件总线 - 异步事件分发系统

特性:
- 异步事件分发
- 优先级处理
- 事件过滤
- 错误隔离（一个handler失败不影响其他）
- 事件历史记录（可选）
"""

import asyncio
import logging
from typing import Callable, Dict, List, Type, Any, Optional
from collections import defaultdict
from dataclasses import dataclass
from datetime import datetime

from .service_events import DomainEvent, ServiceOperationFailed

logger = logging.getLogger(__name__)


@dataclass
class EventSubscription:
    """事件订阅信息"""
    event_type: Type[DomainEvent]
    handler: Callable
    priority: int = 0
    filter_func: Optional[Callable[[DomainEvent], bool]] = None


class EventBus:
    """
    事件总线 - 核心事件分发系统
    
    职责:
    1. 管理事件订阅
    2. 异步分发事件
    3. 错误隔离
    4. 事件历史记录（可选）
    """
    
    def __init__(self, enable_history: bool = False, history_size: int = 1000):
        self._subscribers: Dict[Type[DomainEvent], List[EventSubscription]] = defaultdict(list)
        self._enable_history = enable_history
        self._history: List[tuple[datetime, DomainEvent]] = []
        self._history_size = history_size
        self._lock = asyncio.Lock()
        
        logger.info("EventBus initialized")
    
    def subscribe(
        self, 
        event_type: Type[DomainEvent], 
        handler: Callable,
        priority: int = 0,
        filter_func: Optional[Callable[[DomainEvent], bool]] = None
    ):
        """
        订阅事件
        
        Args:
            event_type: 事件类型
            handler: 处理函数（必须是 async 函数）
            priority: 优先级（数字越大越先执行）
            filter_func: 过滤函数（返回True才处理）
        """
        if not asyncio.iscoroutinefunction(handler):
            raise ValueError(f"Handler {handler.__name__} must be async function")
        
        subscription = EventSubscription(
            event_type=event_type,
            handler=handler,
            priority=priority,
            filter_func=filter_func
        )
        
        self._subscribers[event_type].append(subscription)
        
        # 按优先级排序（降序）
        self._subscribers[event_type].sort(key=lambda s: s.priority, reverse=True)
        
        logger.debug(f"Subscribed {handler.__name__} to {event_type.__name__} (priority={priority})")
    
    async def publish(self, event: DomainEvent, wait: bool = False):
        """
        发布事件
        
        Args:
            event: 领域事件
            wait: 是否等待所有handler执行完成
        """
        logger.debug(f"Publishing event: {event.__class__.__name__} (id={event.event_id})")
        
        # 记录历史
        if self._enable_history:
            async with self._lock:
                self._history.append((datetime.now(), event))
                if len(self._history) > self._history_size:
                    self._history.pop(0)
        
        # 获取订阅者
        subscribers = self._subscribers.get(type(event), [])
        
        if not subscribers:
            logger.debug(f"No subscribers for {event.__class__.__name__}")
            return
        
        # 创建处理任务
        tasks = []
        for subscription in subscribers:
            # 应用过滤器
            if subscription.filter_func and not subscription.filter_func(event):
                continue
            
            task = asyncio.create_task(
                self._handle_event_safely(subscription.handler, event)
            )
            tasks.append(task)
        
        if wait:
            # 等待所有handler完成
            await asyncio.gather(*tasks, return_exceptions=True)
        else:
            # 不等待，让任务在后台运行
            pass
    
    async def _handle_event_safely(self, handler: Callable, event: DomainEvent):
        """
        安全地处理事件（隔离错误）
        """
        try:
            await handler(event)
            logger.debug(f"Handler {handler.__name__} completed for {event.__class__.__name__}")
        except Exception as e:
            logger.error(
                f"Handler {handler.__name__} failed for {event.__class__.__name__}: {e}",
                exc_info=True
            )
            # 发布错误事件（避免递归）
            if not isinstance(event, ServiceOperationFailed):
                error_event = ServiceOperationFailed(
                    agent_id=getattr(event, 'agent_id', 'unknown'),
                    service_name=getattr(event, 'service_name', 'unknown'),
                    operation=f"handle_{event.__class__.__name__}",
                    error_message=str(e),
                    original_event=event
                )
                await self.publish(error_event, wait=False)
    
    def get_history(self, event_type: Optional[Type[DomainEvent]] = None) -> List[DomainEvent]:
        """获取事件历史"""
        if not self._enable_history:
            return []
        
        if event_type:
            return [e for _, e in self._history if isinstance(e, event_type)]
        return [e for _, e in self._history]
    
    def clear_history(self):
        """清空事件历史"""
        self._history.clear()
    
    def get_subscriber_count(self, event_type: Type[DomainEvent]) -> int:
        """获取某个事件类型的订阅者数量"""
        return len(self._subscribers.get(event_type, []))
    
    def unsubscribe_all(self, event_type: Optional[Type[DomainEvent]] = None):
        """
        取消订阅
        
        Args:
            event_type: 事件类型，如果为None则取消所有订阅
        """
        if event_type:
            if event_type in self._subscribers:
                del self._subscribers[event_type]
                logger.debug(f"Unsubscribed all handlers from {event_type.__name__}")
        else:
            self._subscribers.clear()
            logger.debug("Unsubscribed all handlers from all events")

