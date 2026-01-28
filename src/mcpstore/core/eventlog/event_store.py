"""
事件存储层：基于共享 KVStore 的事件日志实现。

- 使用 collection 分片：queue / offset / dedup / lease
- 事件按时间序递增 id（time_ns 字符串），便于顺序消费
"""

from __future__ import annotations

import asyncio
import logging
import time
from typing import Dict, List, Optional, Any

from mcpstore.core.eventlog.event_models import EventRecord

logger = logging.getLogger(__name__)


class EventStore:
    """事件存储抽象，封装 KV 操作与桥接。"""

    def __init__(
        self,
        kv_store,
        namespace: str,
        *,
        max_queue_length: int = 1_000_000,
        dedup_ttl_seconds: float = 86400.0,
    ):
        self._kv_store = kv_store
        self._namespace = namespace
        self._max_queue_length = max_queue_length
        self._dedup_ttl_seconds = dedup_ttl_seconds

        self._queue_collection = f"{namespace}:events:queue"
        self._offset_collection = f"{namespace}:events:offset"
        self._dedup_collection = f"{namespace}:events:dedup"
        self._lease_collection = f"{namespace}:events:lease"

        try:
            from mcpstore.core.bridge import get_async_bridge  # 延迟导入，避免循环
            self._bridge = get_async_bridge()
        except Exception:
            self._bridge = None

    async def _await_in_bridge(self, coro, op_name: str):
        """确保在统一事件循环中执行 KV 协程。"""
        if self._bridge is None:
            return await coro

        bridge_loop = getattr(self._bridge, "_loop", None)
        try:
            running_loop = asyncio.get_running_loop()
        except RuntimeError:
            running_loop = None

        if bridge_loop and running_loop is bridge_loop:
            return await coro

        if running_loop is None:
            return self._bridge.run(coro, op_name=op_name)

        return await asyncio.to_thread(self._bridge.run, coro, op_name=op_name)

    def _generate_event_id(self) -> str:
        """使用 time_ns 生成递增的字符串 id。"""
        return str(time.time_ns())

    async def append_event(self, record: EventRecord) -> EventRecord:
        """写入事件队列并返回带 id 的记录。"""
        event_id = self._generate_event_id()
        data = record.to_dict()
        data["id"] = event_id
        logger.debug(f"[EVENT_STORE] append_event id={event_id} type={record.type}")

        await self._await_in_bridge(
            self._kv_store.put(event_id, data, collection=self._queue_collection),
            op_name="event_store.append_event",
        )

        # 高水位裁剪（极少触发）
        if self._max_queue_length > 0:
            try:
                await self._maybe_trim_queue()
            except Exception as trim_error:
                logger.warning(f"[EVENT_STORE] trim queue failed (ignored): {trim_error}")

        return EventRecord.from_dict(data)

    async def fetch_events(self, last_id: Optional[str], limit: int = 100) -> List[EventRecord]:
        """按 id 顺序获取大于 last_id 的事件。"""
        last_numeric = int(last_id) if last_id else None

        def _filter_and_sort(keys: List[str]) -> List[str]:
            numeric_keys = []
            for key in keys:
                try:
                    num = int(key)
                except Exception:
                    continue
                if last_numeric is None or num > last_numeric:
                    numeric_keys.append(num)
            numeric_keys.sort()
            return [str(k) for k in numeric_keys[:limit]]

        keys = await self._await_in_bridge(
            self._kv_store.keys(collection=self._queue_collection),
            op_name="event_store.fetch_events.keys",
        )
        if not keys:
            return []

        target_keys = _filter_and_sort(keys)
        if not target_keys:
            return []

        results = await self._await_in_bridge(
            self._kv_store.get_many(target_keys, collection=self._queue_collection),
            op_name="event_store.fetch_events.get_many",
        )
        events: List[EventRecord] = []
        for item in results:
            if item is None:
                continue
            try:
                events.append(EventRecord.from_dict(item))
            except Exception as parse_error:
                logger.warning(f"[EVENT_STORE] skip invalid event: {parse_error}")
        return events

    async def get_offset(self, consumer_id: str) -> Optional[str]:
        """获取消费者已提交的最后 id。"""
        result = await self._await_in_bridge(
            self._kv_store.get(consumer_id, collection=self._offset_collection),
            op_name="event_store.get_offset",
        )
        if not result:
            return None
        return str(result.get("last_id"))

    async def commit_offset(self, consumer_id: str, last_id: str) -> None:
        """提交消费位点。"""
        payload = {"last_id": str(last_id), "updated_at": time.time()}
        await self._await_in_bridge(
            self._kv_store.put(consumer_id, payload, collection=self._offset_collection),
            op_name="event_store.commit_offset",
        )

    async def get_dedup_entry(self, dedup_key: str) -> Optional[Dict[str, Any]]:
        """读取去重记录，过期则视为不存在。"""
        entry = await self._await_in_bridge(
            self._kv_store.get(dedup_key, collection=self._dedup_collection),
            op_name="event_store.get_dedup",
        )
        if not entry:
            return None
        expires_at = entry.get("expires_at")
        if expires_at is not None and expires_at < time.time():
            return None
        return entry

    async def update_dedup_entry(self, dedup_key: str, event_id: str) -> None:
        """更新去重记录。"""
        now = time.time()
        entry = {
            "event_id": str(event_id),
            "updated_at": now,
            "expires_at": now + self._dedup_ttl_seconds,
        }
        await self._await_in_bridge(
            self._kv_store.put(dedup_key, entry, collection=self._dedup_collection),
            op_name="event_store.update_dedup",
        )

    async def acquire_lease(self, consumer_id: str, ttl: float, lease_key: str = "lease") -> bool:
        """
        尝试获取租约（非原子，尽力而为）。
        """
        now = time.time()
        entry = await self._await_in_bridge(
            self._kv_store.get(lease_key, collection=self._lease_collection),
            op_name="event_store.get_lease",
        )
        if entry:
            owner = entry.get("owner")
            expires_at = entry.get("expires_at", 0)
            if owner and owner != consumer_id and expires_at > now:
                return False
        lease = {"owner": consumer_id, "expires_at": now + ttl}
        await self._await_in_bridge(
            self._kv_store.put(lease_key, lease, collection=self._lease_collection),
            op_name="event_store.acquire_lease",
        )
        return True

    async def renew_lease(self, consumer_id: str, ttl: float, lease_key: str = "lease") -> bool:
        """续约租约。"""
        now = time.time()
        entry = await self._await_in_bridge(
            self._kv_store.get(lease_key, collection=self._lease_collection),
            op_name="event_store.get_lease",
        )
        if not entry or entry.get("owner") != consumer_id:
            return False
        lease = {"owner": consumer_id, "expires_at": now + ttl}
        await self._await_in_bridge(
            self._kv_store.put(lease_key, lease, collection=self._lease_collection),
            op_name="event_store.renew_lease",
        )
        return True

    async def release_lease(self, consumer_id: str, lease_key: str = "lease") -> None:
        """释放租约。"""
        entry = await self._await_in_bridge(
            self._kv_store.get(lease_key, collection=self._lease_collection),
            op_name="event_store.get_lease",
        )
        if not entry or entry.get("owner") != consumer_id:
            return
        await self._await_in_bridge(
            self._kv_store.delete(lease_key, collection=self._lease_collection),
            op_name="event_store.release_lease",
        )

    async def _get_min_offset(self) -> Optional[int]:
        """读取所有消费者的最小 offset（用于安全裁剪）。"""
        try:
            keys = await self._await_in_bridge(
                self._kv_store.keys(collection=self._offset_collection),
                op_name="event_store.offset.keys",
            )
            if not keys:
                return None
            offsets = await self._await_in_bridge(
                self._kv_store.get_many(keys, collection=self._offset_collection),
                op_name="event_store.offset.get_many",
            )
            values: List[int] = []
            for item in offsets:
                if not item:
                    continue
                try:
                    values.append(int(item.get("last_id")))
                except Exception:
                    continue
            if not values:
                return None
            return min(values)
        except Exception:
            return None

    async def _maybe_trim_queue(self) -> None:
        """高水位裁剪，保留最近 max_queue_length 条。"""
        keys = await self._await_in_bridge(
            self._kv_store.keys(collection=self._queue_collection),
            op_name="event_store.trim.keys",
        )
        if not keys:
            return

        numeric_keys = []
        for key in keys:
            try:
                numeric_keys.append(int(key))
            except Exception:
                continue
        if len(numeric_keys) <= self._max_queue_length:
            return
        numeric_keys.sort()
        # 基于最小 offset 限制裁剪范围，避免删除未消费事件
        min_offset = await self._get_min_offset()
        excess = len(numeric_keys) - self._max_queue_length
        candidates = numeric_keys[:excess]
        if min_offset is not None:
            candidates = [k for k in candidates if k < min_offset]
        if not candidates:
            return
        trim_keys = [str(k) for k in candidates]
        logger.warning(
            f"[EVENT_STORE] trimming queue: total={len(numeric_keys)} keep={self._max_queue_length} trim={len(trim_keys)} min_offset={min_offset}"
        )
        for key in trim_keys:
            try:
                await self._await_in_bridge(
                    self._kv_store.delete(key, collection=self._queue_collection),
                    op_name="event_store.trim.delete",
                )
            except Exception as delete_error:
                logger.warning(f"[EVENT_STORE] failed to delete trimmed key {key}: {delete_error}")
