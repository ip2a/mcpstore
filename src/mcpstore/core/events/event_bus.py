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
from collections import defaultdict
from dataclasses import dataclass
from datetime import datetime
from typing import Callable, Dict, List, Type, Optional, Tuple

from .service_events import (
    DomainEvent,
    ServiceOperationFailed,
    ServiceInitialized,
    ServiceConnectionRequested,
    HealthCheckRequested,
    ServiceAddRequested,
)

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

    def __init__(self, enable_history: bool = False, history_size: int = 1000, handler_timeout: Optional[float] = None):
        self._subscribers: Dict[Type[DomainEvent], List[EventSubscription]] = defaultdict(list)
        self._enable_history = enable_history
        self._history: List[Tuple[datetime, DomainEvent]] = []
        self._history_size = history_size
        self._lock = asyncio.Lock()
        self._handler_timeout = handler_timeout

        logger.info(f"EventBus initialized id={hex(id(self))}")
        # 关键事件白名单：这些事件将被强制以同步方式派发（wait=True）
        # ServiceAddRequested 必须同步执行，确保缓存操作完成后再继续
        self._critical_sync_events = (
            ServiceInitialized,
            ServiceConnectionRequested,
            HealthCheckRequested,
            ServiceAddRequested,
        )

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

        logger.debug(f"[BUS {hex(id(self))}] Subscribed {handler.__name__} to {event_type.__name__} (priority={priority})")

    def unsubscribe(self, event_type: Type[DomainEvent], handler: Callable) -> bool:
        """取消订阅指定 handler（精确移除）。"""
        subs = self._subscribers.get(event_type, [])
        before = len(subs)
        self._subscribers[event_type] = [s for s in subs if s.handler is not handler]
        removed = before != len(self._subscribers[event_type])
        if removed:
            logger.debug(f"[BUS {hex(id(self))}] Unsubscribed {getattr(handler,'__name__',repr(handler))} from {event_type.__name__}")
        return removed

    async def publish(self, event: DomainEvent, wait: bool = False):
        """
        发布事件

        Args:
            event: 领域事件
            wait: 是否等待所有handler执行完成
        """
        # 
        #  NOTE: 
        #  在 Windows+asyncio    
        #     loop   
        #     
        is_critical = isinstance(event, self._critical_sync_events)
        if is_critical and not wait:
            logger.debug(f"[BUS {hex(id(self))}] Critical event {event.__class__.__name__} forcing wait=True")
            wait = True

        # 使 ServiceCached 也同步执行，避免生命周期初始化被取消
        from .service_events import ServiceCached
        if isinstance(event, ServiceCached) and not wait:
            logger.debug(f"[BUS {hex(id(self))}] ServiceCached forcing wait=True to ensure lifecycle init")
            wait = True
        logger.debug(f"[BUS {hex(id(self))}] Publishing event: {event.__class__.__name__} (id={event.event_id}) wait={wait}")

        # 记录历史
        if self._enable_history:
            async with self._lock:
                self._history.append((datetime.now(), event))
                if len(self._history) > self._history_size:
                    self._history.pop(0)

        # 获取订阅者
        subscribers = self._subscribers.get(type(event), [])
        # Diagnostics: subscriber details
        try:
            handler_names = [getattr(s.handler, "__name__", repr(s.handler)) for s in subscribers]
        except Exception:
            handler_names = ["<inspect_error>"]
        logger.debug(f"[BUS {hex(id(self))}] {event.__class__.__name__} subs={len(subscribers)} handlers={handler_names}")


        if not subscribers:
            logger.debug(f"No subscribers for {event.__class__.__name__}")
            return

        if wait:
            # #region agent log
            wait_start = __import__("time").time()
            try:
                import json
                from pathlib import Path
                log_path = Path("/home/yuuu/app/2025/2025_6/mcpstore/.cursor/debug.log")
                log_record = {
                    "sessionId": "debug-session",
                    "runId": "timeout-investigation",
                    "hypothesisId": "H3",
                    "location": "event_bus.py:publish",
                    "message": "wait_start",
                    "data": {
                        "event_type": event.__class__.__name__,
                        "event_id": str(event.event_id) if hasattr(event, 'event_id') else None,
                        "subscribers_count": len(subscribers),
                    },
                    "timestamp": int(wait_start * 1000),
                }
                log_path.parent.mkdir(parents=True, exist_ok=True)
                with log_path.open("a", encoding="utf-8") as f:
                    f.write(json.dumps(log_record, ensure_ascii=False) + "\n")
            except Exception:
                pass
            # #endregion
            
            # 同步顺序执行，保证关键事件在当前上下文中完成处理
            handler_index = 0
            for subscription in subscribers:
                if subscription.filter_func and not subscription.filter_func(event):
                    continue
                # #region agent log
                handler_start = __import__("time").time()
                # #endregion
                await self._handle_event_safely(subscription.handler, event)
                # #region agent log
                handler_time = __import__("time").time() - handler_start
                handler_index += 1
                try:
                    import json
                    from pathlib import Path
                    log_path = Path("/home/yuuu/app/2025/2025_6/mcpstore/.cursor/debug.log")
                    log_record = {
                        "sessionId": "debug-session",
                        "runId": "timeout-investigation",
                        "hypothesisId": "H3",
                        "location": "event_bus.py:publish",
                        "message": "handler_completed",
                        "data": {
                            "event_type": event.__class__.__name__,
                            "handler_name": subscription.handler.__name__,
                            "handler_index": handler_index,
                            "handler_time_ms": handler_time * 1000,
                        },
                        "timestamp": int(__import__("time").time() * 1000),
                    }
                    log_path.parent.mkdir(parents=True, exist_ok=True)
                    with log_path.open("a", encoding="utf-8") as f:
                        f.write(json.dumps(log_record, ensure_ascii=False) + "\n")
                except Exception:
                    pass
                # #endregion
            
            # #region agent log
            wait_time = __import__("time").time() - wait_start
            try:
                import json
                from pathlib import Path
                log_path = Path("/home/yuuu/app/2025/2025_6/mcpstore/.cursor/debug.log")
                log_record = {
                    "sessionId": "debug-session",
                    "runId": "timeout-investigation",
                    "hypothesisId": "H3",
                    "location": "event_bus.py:publish",
                    "message": "wait_completed",
                    "data": {
                        "event_type": event.__class__.__name__,
                        "total_wait_time_ms": wait_time * 1000,
                        "handlers_count": handler_index,
                    },
                    "timestamp": int(__import__("time").time() * 1000),
                }
                log_path.parent.mkdir(parents=True, exist_ok=True)
                with log_path.open("a", encoding="utf-8") as f:
                    f.write(json.dumps(log_record, ensure_ascii=False) + "\n")
            except Exception:
                pass
            # #endregion
        else:
            # 异步后台执行（fire-and-forget）
            for subscription in subscribers:
                if subscription.filter_func and not subscription.filter_func(event):
                    continue
                asyncio.create_task(self._handle_event_safely(subscription.handler, event))

    async def _handle_event_safely(self, handler: Callable, event: DomainEvent):
        """
        安全地处理事件（隔离错误）
        """
        # #region agent log
        try:
            import json
            from pathlib import Path
            import time as time_module
            log_path = Path("/home/yuuu/app/2025/2025_6/mcpstore/.cursor/debug.log")
            log_record = {
                "sessionId": "debug-session",
                "runId": "pre-fix",
                "hypothesisId": "H1,H3",
                "location": "event_bus.py:_handle_event_safely",
                "message": "handler_start",
                "data": {
                    "handler_name": handler.__name__,
                    "event_type": event.__class__.__name__,
                    "event_id": str(event.event_id) if hasattr(event, 'event_id') else None,
                },
                "timestamp": int(time_module.time() * 1000),
            }
            log_path.parent.mkdir(parents=True, exist_ok=True)
            with log_path.open("a", encoding="utf-8") as f:
                f.write(json.dumps(log_record, ensure_ascii=False) + "\n")
        except Exception:
            pass
        # #endregion
        try:
            if self._handler_timeout and self._handler_timeout > 0:
                await asyncio.wait_for(handler(event), timeout=self._handler_timeout)
            else:
                await handler(event)
            logger.debug(f"Handler {handler.__name__} completed for {event.__class__.__name__}")
            # #region agent log
            try:
                import json
                from pathlib import Path
                import time as time_module
                log_path = Path("/home/yuuu/app/2025/2025_6/mcpstore/.cursor/debug.log")
                log_record = {
                    "sessionId": "debug-session",
                    "runId": "pre-fix",
                    "hypothesisId": "H1,H3",
                    "location": "event_bus.py:_handle_event_safely",
                    "message": "handler_completed",
                    "data": {
                        "handler_name": handler.__name__,
                        "event_type": event.__class__.__name__,
                    },
                    "timestamp": int(time_module.time() * 1000),
                }
                log_path.parent.mkdir(parents=True, exist_ok=True)
                with log_path.open("a", encoding="utf-8") as f:
                    f.write(json.dumps(log_record, ensure_ascii=False) + "\n")
            except Exception:
                pass
            # #endregion
        except asyncio.CancelledError as ce:
            logger.warning(f"Handler {handler.__name__} cancelled for {event.__class__.__name__}: {ce}")
            # #region agent log
            try:
                import json
                from pathlib import Path
                import time as time_module
                log_path = Path("/home/yuuu/app/2025/2025_6/mcpstore/.cursor/debug.log")
                log_record = {
                    "sessionId": "debug-session",
                    "runId": "pre-fix",
                    "hypothesisId": "H1,H3",
                    "location": "event_bus.py:_handle_event_safely",
                    "message": "handler_cancelled",
                    "data": {
                        "handler_name": handler.__name__,
                        "event_type": event.__class__.__name__,
                        "error": str(ce),
                    },
                    "timestamp": int(time_module.time() * 1000),
                }
                log_path.parent.mkdir(parents=True, exist_ok=True)
                with log_path.open("a", encoding="utf-8") as f:
                    f.write(json.dumps(log_record, ensure_ascii=False) + "\n")
            except Exception:
                pass
            # #endregion
            # do not re-raise to avoid noisy loop exceptions
        except GeneratorExit as ge:
            logger.warning(f"Handler {handler.__name__} generator-exit for {event.__class__.__name__}: {ge}")
            # do not re-raise to avoid noisy loop exceptions
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
