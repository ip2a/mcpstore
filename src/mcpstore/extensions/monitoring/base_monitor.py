"""
MCPStore monitoring and statistics module
Provides performance monitoring, tool usage statistics, alert management and other functions
"""

import asyncio
import json
import logging
import time
from collections import deque
from datetime import datetime
from pathlib import Path
from typing import Dict, List, Optional, Any

logger = logging.getLogger(__name__)

class MonitoringManager:
    """监控管理器"""

    def __init__(
        self,
        data_dir: Path,
        tool_record_max_file_size: int = 30,
        tool_record_retention_days: int = 7,
        max_cache_records: int = 2000,
        flush_interval: float = 1.0,
        max_batch_size: int = 128,
    ):
        self.data_dir = data_dir
        self.tool_records_file = data_dir / "tool_records.jsonl"  # 追加写，避免全量读写
        self.summary_file = data_dir / "tool_records_summary.json"

        # 工具记录配置
        self.max_file_size_mb = tool_record_max_file_size
        self.retention_days = tool_record_retention_days
        self.flush_interval = flush_interval
        self.max_batch_size = max_batch_size
        self.max_cache_records = max_cache_records

        # 记录启动时间用于计算运行时间
        self.start_time = time.time()
        
        # API 监控相关
        self.active_connections = 0
        self.api_call_count = 0
        self.total_response_time = 0.0

        # 异步写入组件（延迟创建队列/事件，避免无事件循环时报错）
        self._queue: Optional[asyncio.Queue] = None
        self._stop_event: Optional[asyncio.Event] = None
        self._worker_task: Optional[asyncio.Task] = None
        self._enabled = True

        # 内存缓存，快速返回最近记录
        self._recent_records: deque = deque(maxlen=max_cache_records)
        self._summary: Dict[str, Any] = self._default_summary()

        self._init_storage()

    def _default_summary(self) -> Dict[str, Any]:
        return {
            "total_executions": 0,
            "by_tool": {},
            "by_service": {}
        }

    def _init_storage(self) -> None:
        """初始化文件与内存状态，失败仅告警不阻塞"""
        try:
            self.data_dir.mkdir(parents=True, exist_ok=True)
            if self.summary_file.exists():
                try:
                    self._summary = json.loads(self.summary_file.read_text(encoding="utf-8"))
                except Exception:
                    logger.warning("[MONITORING] summary file corrupted, resetting.")
                    self._summary = self._default_summary()
                    self.summary_file.write_text(json.dumps(self._summary, indent=2, ensure_ascii=False), encoding="utf-8")
            else:
                self.summary_file.write_text(json.dumps(self._summary, indent=2, ensure_ascii=False), encoding="utf-8")

            # 预加载有限数量的历史记录，避免大文件阻塞
            if self.tool_records_file.exists():
                self._load_recent_records_from_file()
            else:
                self.tool_records_file.touch()
        except Exception as e:
            logger.warning(f"[MONITORING] init storage failed, monitoring disabled: {e}")
            self._enabled = False

    def _load_recent_records_from_file(self) -> None:
        """仅加载最后 max_cache_records 条，避免大文件带来的阻塞"""
        try:
            with open(self.tool_records_file, "r", encoding="utf-8") as f:
                recent = deque((json.loads(line) for line in f if line.strip()), maxlen=self.max_cache_records)
                self._recent_records.extend(recent)
        except Exception as e:
            logger.warning(f"[MONITORING] failed to load recent records: {e}")

    def _ensure_primitives(self) -> bool:
        """确保队列和停止事件存在"""
        if not self._enabled:
            return False
        try:
            if self._queue is None:
                self._queue = asyncio.Queue()
            if self._stop_event is None:
                self._stop_event = asyncio.Event()
            return True
        except Exception as e:
            logger.warning(f"[MONITORING] failed to init async primitives: {e}")
            return False

    # 旧的record_tool_execution方法已移除，使用record_tool_execution_detailed代替
    # 旧的get_tool_usage_stats方法已移除，使用get_tool_records代替
    
    def record_api_call(self, response_time: float):
        """记录 API 调用"""
        self.api_call_count += 1
        self.total_response_time += response_time
    
    def increment_active_connections(self):
        """增加活跃连接数"""
        self.active_connections += 1
    
    def decrement_active_connections(self):
        """减少活跃连接数"""
        self.active_connections = max(0, self.active_connections - 1)

    def _ensure_worker(self):
        """确保后台写入任务已启动，不会嵌套 AOB 事件循环"""
        if not self._enabled:
            return
        if self._worker_task and not self._worker_task.done():
            return
        if not self._ensure_primitives():
            return
        try:
            loop = asyncio.get_running_loop()
        except RuntimeError:
            logger.warning("[MONITORING] No running event loop; monitoring queue will start when loop is available.")
            return
        self._worker_task = loop.create_task(self._worker(), name="monitoring-writer")

    async def stop_worker(self):
        """停止后台任务（测试/关闭时调用）"""
        if not self._worker_task:
            return
        if self._stop_event:
            self._stop_event.set()
        await self._worker_task
        self._worker_task = None

    async def _worker(self):
        """后台批量落盘，避免阻塞工具调用"""
        batch: List[Dict[str, Any]] = []
        while self._stop_event and (not self._stop_event.is_set()):
            try:
                item = await asyncio.wait_for(self._queue.get(), timeout=self.flush_interval)
                batch.append(item)
                # 尝试一次性取满一批
                while len(batch) < self.max_batch_size:
                    try:
                        batch.append(self._queue.get_nowait())
                    except asyncio.QueueEmpty:
                        break
            except asyncio.TimeoutError:
                pass
            except Exception as e:
                logger.warning(f"[MONITORING] worker wait error: {e}")

            if not batch:
                continue

            try:
                await asyncio.to_thread(self._flush_batch, list(batch))
            except Exception as e:
                logger.warning(f"[MONITORING] flush failed: {e}")
            finally:
                batch.clear()

    def record_tool_execution_detailed(self, tool_name: str, service_name: str,
                                     params: Dict[str, Any], result: Optional[Any],
                                     error: Optional[str], response_time: float):
        """异步落盘入口：仅入队，不阻塞调用链"""
        if not self._enabled:
            logger.warning("[MONITORING] Monitoring disabled, skip record.")
            return

        try:
            # 如果没有事件循环且无法启动 worker，则同步落盘以避免堆积
            loop = None
            try:
                loop = asyncio.get_running_loop()
            except RuntimeError:
                loop = None

            execution_time = datetime.now()

            def _normalize_result(res):
                try:
                    if hasattr(res, 'content'):
                        items = []
                        for c in getattr(res, 'content', []) or []:
                            try:
                                if isinstance(c, dict):
                                    items.append(c)
                                elif hasattr(c, 'type') and hasattr(c, 'text'):
                                    items.append({"type": getattr(c, 'type', 'text'), "text": getattr(c, 'text', '')})
                                elif hasattr(c, 'type') and hasattr(c, 'uri'):
                                    items.append({"type": getattr(c, 'type', 'uri'), "uri": getattr(c, 'uri', '')})
                                else:
                                    items.append(str(c))
                            except Exception:
                                items.append(str(c))
                        return {"content": items, "is_error": bool(getattr(res, 'is_error', False))}
                    if isinstance(res, (dict, list)):
                        return res
                    return {"result": str(res)}
                except Exception:
                    return {"result": str(res)}

            record = {
                "id": f"{int(execution_time.timestamp() * 1000)}_{hash(tool_name) % 10000:04d}",
                "tool_name": tool_name,
                "service_name": service_name,
                "params": params,
                "result": _normalize_result(result),
                "error": error,
                "response_time": round(response_time, 2),
                "execution_time": execution_time.isoformat(),
                "timestamp": int(execution_time.timestamp())
            }

            self._recent_records.append(record)
            self._update_summary_in_memory(record)

            if loop is None:
                # 无事件循环：直接同步写，失败不抛出
                try:
                    self._flush_batch([record])
                except Exception as flush_error:
                    logger.warning(f"[MONITORING] sync flush failed (no event loop): {flush_error}")
                return

            try:
                if self._ensure_primitives() and self._queue:
                    self._queue.put_nowait(record)
                    self._ensure_worker()
            except Exception as queue_error:
                logger.warning(f"[MONITORING] enqueue failed: {queue_error}")
        except Exception as e:
            logger.warning(f"[MONITORING] Failed to prepare monitoring record: {e}")

    def _update_summary_in_memory(self, record: Dict[str, Any]) -> None:
        """增量更新汇总统计"""
        summary = self._summary
        tool_name = record.get("tool_name", "unknown")
        service_name = record.get("service_name", "unknown")
        response_time = record.get("response_time", 0.0)

        summary["total_executions"] += 1

        tool_stats = summary["by_tool"].setdefault(tool_name, {"count": 0, "total_response_time": 0.0})
        tool_stats["count"] += 1
        tool_stats["total_response_time"] += response_time
        tool_stats["avg_response_time"] = round(tool_stats["total_response_time"] / tool_stats["count"], 2)

        service_stats = summary["by_service"].setdefault(service_name, {"count": 0, "total_response_time": 0.0})
        service_stats["count"] += 1
        service_stats["total_response_time"] += response_time
        service_stats["avg_response_time"] = round(service_stats["total_response_time"] / service_stats["count"], 2)

    def _flush_batch(self, batch: List[Dict[str, Any]]) -> None:
        """写入文件（在线程中执行），尽量减少阻塞"""
        try:
            self.data_dir.mkdir(parents=True, exist_ok=True)
        except Exception as e:
            logger.warning(f"[MONITORING] failed to ensure data dir: {e}")
            return

        try:
            with open(self.tool_records_file, "a", encoding="utf-8") as f:
                for record in batch:
                    f.write(json.dumps(record, ensure_ascii=False))
                    f.write("\n")
        except Exception as e:
            logger.warning(f"[MONITORING] write file failed: {e}")
            return

        # 写入汇总文件（小文件，不易阻塞）
        try:
            self.summary_file.write_text(json.dumps(self._summary, indent=2, ensure_ascii=False), encoding="utf-8")
        except Exception as e:
            logger.warning(f"[MONITORING] write summary failed: {e}")

        self._maybe_rotate()

    def _maybe_rotate(self) -> None:
        """简单的文件大小保护：超限时轮转"""
        if self.max_file_size_mb == -1:
            return
        try:
            size_mb = self.tool_records_file.stat().st_size / (1024 * 1024)
            if size_mb > self.max_file_size_mb:
                rotated = self.data_dir / f"tool_records_{int(time.time())}.bak"
                try:
                    self.tool_records_file.rename(rotated)
                    logger.warning(f"[MONITORING] tool_records file rotated to {rotated}")
                except Exception as e:
                    logger.warning(f"[MONITORING] rotate failed: {e}")
                finally:
                    # 新文件，保留最近缓存中的记录，以便后续继续追加
                    self.tool_records_file.touch()
                # 清理过期的备份文件（按天数）
                if self.retention_days != -1:
                    cutoff_ts = time.time() - self.retention_days * 86400
                    for bak in self.data_dir.glob("tool_records_*.bak"):
                        try:
                            if bak.stat().st_mtime < cutoff_ts:
                                bak.unlink()
                        except Exception:
                            continue
        except Exception as e:
            logger.debug(f"[MONITORING] rotate check failed: {e}")

    def get_tool_records(self, limit: int = 50) -> Dict[str, Any]:
        """获取工具执行记录（仅从内存缓存返回，避免大文件阻塞）"""
        if not self._enabled:
            return {
                "executions": [],
                "summary": self._default_summary(),
                "degraded": "Monitoring disabled"
            }
        try:
            executions = list(self._recent_records)

            # 按保留天数过滤（仅返回时过滤，不阻塞调用）
            if self.retention_days != -1:
                cutoff_ts = int(time.time() - self.retention_days * 86400)
                executions = [e for e in executions if e.get("timestamp", 0) >= cutoff_ts]

            executions.sort(key=lambda x: x.get("timestamp", 0), reverse=True)
            if limit > 0:
                executions = executions[:limit]

            # 返回时重建简易 summary（避免使用过期缓存）
            summary = self._default_summary()
            for ex in executions:
                tool = ex.get("tool_name", "unknown")
                svc = ex.get("service_name", "unknown")
                rt = ex.get("response_time", 0.0)
                summary["total_executions"] += 1
                ts = summary["by_tool"].setdefault(tool, {"count": 0, "total_response_time": 0.0})
                ts["count"] += 1
                ts["total_response_time"] += rt
                ts["avg_response_time"] = round(ts["total_response_time"] / ts["count"], 2)
                ss = summary["by_service"].setdefault(svc, {"count": 0, "total_response_time": 0.0})
                ss["count"] += 1
                ss["total_response_time"] += rt
                ss["avg_response_time"] = round(ss["total_response_time"] / ss["count"], 2)

            return {
                "executions": executions,
                "summary": summary
            }

        except Exception as e:
            logger.error(f"Failed to get tool records: {e}")
            return {
                "executions": [],
                "summary": self._default_summary()
            }
