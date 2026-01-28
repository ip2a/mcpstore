"""
事件同步器：从共享事件队列拉取事件并投递到本地 EventBus。
"""

from __future__ import annotations

import asyncio
import logging
import os
import socket
import time
from typing import Dict, Optional, Type

from mcpstore.core.eventlog.event_models import record_to_domain_event
from mcpstore.core.eventlog.event_store import EventStore
from mcpstore.core.events.service_events import (
    DomainEvent,
    HealthCheckRequested,
    HealthCheckCompleted,
    ReconnectionRequested,
    ReconnectionScheduled,
    ServiceAddRequested,
    ServiceBootstrapRequested,
    ServiceConnectionRequested,
    ServiceRestartRequested,
    ServiceResetRequested,
    ServiceTimeout,
    ServiceStateChanged,
)

logger = logging.getLogger(__name__)


def _default_consumer_id() -> str:
    try:
        return f"{socket.gethostname()}_{os.getpid()}"
    except Exception:
        return "event_syncer"


DEFAULT_EVENT_MAPPING: Dict[str, Type[DomainEvent]] = {
    "ServiceAddRequested": ServiceAddRequested,
    "ServiceBootstrapRequested": ServiceBootstrapRequested,
    "HealthCheckRequested": HealthCheckRequested,
    "HealthCheckCompleted": HealthCheckCompleted,
    "ServiceTimeout": ServiceTimeout,
    "ReconnectionRequested": ReconnectionRequested,
    "ReconnectionScheduled": ReconnectionScheduled,
    "ServiceConnectionRequested": ServiceConnectionRequested,
    "ServiceRestartRequested": ServiceRestartRequested,
    "ServiceResetRequested": ServiceResetRequested,
    "ServiceStateChanged": ServiceStateChanged,
}


class EventSyncer:
    """事件同步器：消费事件日志并转发到本地 EventBus。"""

    def __init__(
        self,
        event_store: EventStore,
        event_bus,
        *,
        consumer_id: Optional[str] = None,
        batch_size: int = 100,
        poll_interval: float = 1.0,
        lease_ttl: float = 30.0,
        enable_lease: bool = True,
        dedup_enabled: bool = True,
        event_mapping: Optional[Dict[str, Type[DomainEvent]]] = None,
        lease_key: str = "lease:event_syncer",
    ):
        self._event_store = event_store
        self._event_bus = event_bus
        self._consumer_id = consumer_id or _default_consumer_id()
        self._batch_size = batch_size
        self._poll_interval = poll_interval
        self._lease_ttl = lease_ttl
        self._enable_lease = enable_lease
        self._dedup_enabled = dedup_enabled
        self._event_mapping = event_mapping or DEFAULT_EVENT_MAPPING
        self._lease_key = lease_key

        self._running = False
        self._task: Optional[asyncio.Task] = None
        self._last_lease_refresh = 0.0
        # 用于同步消费的锁，避免并发 _poll_once
        self._consume_lock = asyncio.Lock()

    async def start(self):
        if self._running:
            return
        self._running = True
        logger.info(f"[EVENT_SYNCER] start consumer={self._consumer_id}")
        self._task = asyncio.create_task(self._run_loop())

    async def stop(self):
        self._running = False
        if self._task:
            self._task.cancel()
            try:
                await self._task
            except asyncio.CancelledError:
                pass
        await self._release_lease()
        logger.info(f"[EVENT_SYNCER] stop consumer={self._consumer_id}")

    async def _run_loop(self):
        while self._running:
            try:
                lease_ok = await self._ensure_lease()
                if not lease_ok:
                    await asyncio.sleep(self._poll_interval)
                    continue

                await self._poll_once()
            except asyncio.CancelledError:
                break
            except Exception as e:
                logger.error(f"[EVENT_SYNCER] loop error: {e}", exc_info=True)
                await asyncio.sleep(self._poll_interval)
            else:
                await asyncio.sleep(self._poll_interval)

    async def _ensure_lease(self) -> bool:
        if not self._enable_lease:
            return True
        now = time.time()
        # 初次尝试
        if self._last_lease_refresh == 0.0:
            acquired = await self._event_store.acquire_lease(
                self._consumer_id,
                self._lease_ttl,
                lease_key=self._lease_key,
            )
            if acquired:
                self._last_lease_refresh = now
            return acquired
        # 续约
        if now - self._last_lease_refresh >= self._lease_ttl / 2:
            renewed = await self._event_store.renew_lease(
                self._consumer_id,
                self._lease_ttl,
                lease_key=self._lease_key,
            )
            if renewed:
                self._last_lease_refresh = now
            else:
                self._last_lease_refresh = 0.0
            return renewed
        return True

    async def _release_lease(self):
        if not self._enable_lease:
            return
        try:
            await self._event_store.release_lease(
                self._consumer_id,
                lease_key=self._lease_key,
            )
        except Exception:
            pass

    async def _poll_once(self):
        last_id = await self._event_store.get_offset(self._consumer_id)
        events = await self._event_store.fetch_events(last_id, limit=self._batch_size)
        if not events:
            return

        last_processed = None
        for record in events:
            if self._dedup_enabled and record.dedup_key:
                if await self._is_duplicate(record):
                    continue

            domain_event = record_to_domain_event(record, self._event_mapping)
            if domain_event is None:
                logger.debug(f"[EVENT_SYNCER] unknown event type: {record.type}")
                last_processed = record.id
                continue

            try:
                await self._event_bus.publish(domain_event, wait=False)
            except Exception as pub_error:
                logger.error(f"[EVENT_SYNCER] publish failed id={record.id} type={record.type}: {pub_error}", exc_info=True)
                break

            if self._dedup_enabled and record.dedup_key:
                await self._event_store.update_dedup_entry(record.dedup_key, record.id)
            last_processed = record.id

        if last_processed:
            await self._event_store.commit_offset(self._consumer_id, last_processed)

    async def consume_once_now(self) -> Optional[str]:
        """
        同步拉取并消费一次事件队列，返回最新提交的 offset（last_id）。
        - 遵循租约/去重，与后台循环逻辑一致
        - 若当前没有租约，将尝试获取；失败则返回当前 offset
        """
        async with self._consume_lock:
            try:
                lease_ok = await self._ensure_lease()
                if not lease_ok:
                    return await self._event_store.get_offset(self._consumer_id)
                await self._poll_once()
                return await self._event_store.get_offset(self._consumer_id)
            except Exception as e:
                logger.error(f"[EVENT_SYNCER] consume_once_now error: {e}", exc_info=True)
                return await self._event_store.get_offset(self._consumer_id)

    async def _is_duplicate(self, record) -> bool:
        entry = await self._event_store.get_dedup_entry(record.dedup_key)  # type: ignore[arg-type]
        if not entry:
            return False
        try:
            previous_id = int(entry.get("event_id", 0))
            current_id = int(record.id)
            return current_id <= previous_id
        except Exception:
            return False
