"""
健康检查管理器（新模型，无兼容层）

- 分探针：Startup/Readiness/Liveness，调度节流按状态决定
- 滑窗/退避/半开/硬超时：此版本先提供基础探针与事件发布，滑窗与退避将由后续迭代补充
- 事件：发布 HealthCheckCompleted，供 LifecycleManager 处理状态迁移
"""

import asyncio
import logging
import time
from collections import deque
from typing import Dict, Tuple, Optional, Deque, Any

from mcpstore.config.config_dataclasses import ServiceLifecycleConfig
from mcpstore.core.events.event_bus import EventBus
from mcpstore.core.events.service_events import (
    ServiceConnected,
    HealthCheckRequested,
    HealthCheckCompleted,
    ServiceStateChanged,
    ServiceTimeout,
)
from mcpstore.core.models.service import ServiceConnectionState
from mcpstore.core.utils.mcp_client_helpers import temp_client_for_service

logger = logging.getLogger(__name__)


class HealthMonitor:
    """
    健康检查管理器（基础版）

    - 监听 ServiceConnected/HealthCheckRequested/ServiceStateChanged
    - 根据状态/配置执行探针，发布 HealthCheckCompleted
    - 采用按状态节流：正常用 liveness_interval，降级/熔断用 warning_ping_timeout
    """

    def __init__(
        self,
        event_bus: EventBus,
        registry: "CoreRegistry",
        lifecycle_config: ServiceLifecycleConfig,
        global_agent_store_id: str = "global_agent_store",
    ):
        self._event_bus = event_bus
        self._registry = registry
        self._config = lifecycle_config
        self._global_agent_store_id = global_agent_store_id

        # 周期与超时
        self._liveness_interval = lifecycle_config.liveness_interval
        self._ping_timeout_http = lifecycle_config.ping_timeout_http
        self._ping_timeout_sse = lifecycle_config.ping_timeout_sse
        self._ping_timeout_stdio = lifecycle_config.ping_timeout_stdio
        self._warning_ping_timeout = lifecycle_config.warning_ping_timeout
        self._readiness_success_threshold = lifecycle_config.readiness_success_threshold
        self._readiness_failure_threshold = lifecycle_config.readiness_failure_threshold
        # 窗口与阈值
        self._window_size = lifecycle_config.window_size
        self._window_min_calls = lifecycle_config.window_min_calls
        self._error_rate_threshold = lifecycle_config.error_rate_threshold
        self._latency_p95_warn = lifecycle_config.latency_p95_warn
        self._latency_p99_critical = lifecycle_config.latency_p99_critical
        # 退避与硬超时
        self._max_reconnect_attempts = lifecycle_config.max_reconnect_attempts
        self._backoff_base = lifecycle_config.backoff_base
        self._backoff_max = lifecycle_config.backoff_max
        self._backoff_jitter = lifecycle_config.backoff_jitter
        self._backoff_max_duration = lifecycle_config.backoff_max_duration
        self._reconnect_hard_timeout = lifecycle_config.reconnect_hard_timeout
        # 半开试探
        self._half_open_max_calls = lifecycle_config.half_open_max_calls
        self._half_open_success_rate_threshold = lifecycle_config.half_open_success_rate_threshold
        # 租约（预留）
        self._lease_ttl = lifecycle_config.lease_ttl
        self._lease_renew_interval = lifecycle_config.lease_renew_interval

        # 任务与节流
        self._last_check_time: Dict[Tuple[str, str], float] = {}
        self._health_check_tasks: Dict[Tuple[str, str], asyncio.Task] = {}
        self._is_running = False
        # 滑动窗口：key -> deque[(timestamp, success(bool), response_time(float|None))]
        self._windows: Dict[Tuple[str, str], Deque[Tuple[float, bool, Optional[float]]]] = {}
        # 退避/硬超时信息：key -> {"next_retry": ts, "attempts": n, "hard_deadline": ts}
        self._cooldowns: Dict[Tuple[str, str], Dict[str, float]] = {}
        # 半开统计：key -> {"attempts": n, "success": n, "failure": n}
        self._half_open_stats: Dict[Tuple[str, str], Dict[str, int]] = {}
        # 租约续约时间：key -> deadline_ts
        self._leases: Dict[Tuple[str, str], float] = {}
        # 启动/就绪连续成功计数
        self._readiness_success: Dict[Tuple[str, str], int] = {}
        self._readiness_failures: Dict[Tuple[str, str], int] = {}
        # 硬超时标记：key -> deadline_ts
        self._hard_timeouts: Dict[Tuple[str, str], float] = {}

        # 订阅事件
        self._event_bus.subscribe(ServiceConnected, self._on_service_connected, priority=30)
        self._event_bus.subscribe(HealthCheckRequested, self._on_health_check_requested, priority=100)
        self._event_bus.subscribe(ServiceStateChanged, self._on_state_changed, priority=20)

        logger.info(
            "HealthMonitor initialized (liveness=%.1fs, warn_ping=%.1fs)",
            self._liveness_interval,
            self._warning_ping_timeout,
        )

    async def start(self):
        if self._is_running:
            logger.warning("HealthMonitor is already running")
            return
        self._is_running = True
        logger.info("HealthMonitor started")

    async def stop(self):
        self._is_running = False
        for task in self._health_check_tasks.values():
            if not task.done():
                task.cancel()
        if self._health_check_tasks:
            await asyncio.gather(*self._health_check_tasks.values(), return_exceptions=True)
        self._health_check_tasks.clear()
        logger.info("HealthMonitor stopped")

    async def _on_service_connected(self, event: ServiceConnected):
        logger.info(f"[HEALTH] Service connected, scheduling initial check: {event.service_name}")
        await self.maybe_schedule_health_check(event.agent_id, event.service_name, force=True)

    async def _on_health_check_requested(self, event: HealthCheckRequested):
        logger.info(f"[HEALTH] Manual health check requested: {event.service_name}")
        await self._execute_health_check(event.agent_id, event.service_name, wait=True)

    async def _on_state_changed(self, event: ServiceStateChanged):
        terminal_states = {ServiceConnectionState.DISCONNECTED.value, ServiceConnectionState.DISCONNECTED}
        if event.new_state in terminal_states:
            task_key = (event.agent_id, event.service_name)
            if task_key in self._health_check_tasks:
                task = self._health_check_tasks.pop(task_key)
                if not task.done():
                    task.cancel()
                logger.info(f"[HEALTH] Stopped health check for terminated service: {event.service_name}")
            self._reset_half_open_stats(task_key)

    async def maybe_schedule_health_check(
        self,
        agent_id: str,
        service_name: str,
        current_state: Optional[ServiceConnectionState | str] = None,
        force: bool = False,
    ) -> bool:
        if not self._is_running:
            return False

        key = (agent_id, service_name)
        existing = self._health_check_tasks.get(key)
        if existing and not existing.done() and not force:
            return False

        state = current_state
        if isinstance(state, str):
            try:
                state = ServiceConnectionState(state)
            except ValueError:
                state = None
        if state is None:
            try:
                global_name = await self._to_global_name_async(agent_id, service_name)
                state = await self._registry.get_service_state_async(self._global_agent_store_id, global_name)
            except Exception:
                state = None

        now = time.time()
        cooldown = self._cooldowns.get(key)
        if cooldown and not force:
            if now < cooldown.get("next_retry", 0):
                return False
            hard_deadline = cooldown.get("hard_deadline")
            if hard_deadline and now >= hard_deadline:
                logger.info(f"[HEALTH] Hard timeout reached for {service_name}, emit timeout and skip")
                await self._event_bus.publish(
                    ServiceTimeout(
                        agent_id=agent_id,
                        service_name=service_name,
                        timeout_type="hard_timeout",
                        elapsed_time=hard_deadline,
                    ),
                    wait=False,
                )
                self._hard_timeouts[key] = hard_deadline
                return False
        # 租约过期触发超时事件
        lease_remaining = self._lease_remaining(key)
        if lease_remaining is not None and lease_remaining <= 0:
            try:
                await self._event_bus.publish(
                    ServiceTimeout(
                        agent_id=agent_id,
                        service_name=service_name,
                        timeout_type="lease_expired",
                        elapsed_time=self._lease_ttl,
                    ),
                    wait=False,
                )
            except Exception as e:
                logger.debug(f"[HEALTH] publish lease expired failed: {e}")
            return False
        last = self._last_check_time.get(key, 0)
        interval = self._liveness_interval
        if state in (ServiceConnectionState.DEGRADED, ServiceConnectionState.CIRCUIT_OPEN, ServiceConnectionState.HALF_OPEN):
            interval = max(self._liveness_interval, 1.0)

        # 半开试探：冷却结束后从 CIRCUIT_OPEN 进入单次试探
        if cooldown and state == ServiceConnectionState.CIRCUIT_OPEN:
            state = ServiceConnectionState.HALF_OPEN

        if not force and (now - last) < interval:
            return False

        self._last_check_time[key] = now
        task = asyncio.create_task(self._execute_health_check(agent_id, service_name))
        self._health_check_tasks[key] = task
        task.add_done_callback(lambda _: self._health_check_tasks.pop(key, None))
        return True

    async def _execute_health_check(self, agent_id: str, service_name: str, wait: bool = False):
        start_time = time.time()
        try:
            global_name = await self._to_global_name_async(agent_id, service_name)
            if not await self._registry.has_service_async(self._global_agent_store_id, global_name):
                logger.info(f"[HEALTH] Skip check for removed service: {service_name}")
                return

            service_entity = await self._registry._cache_service_manager.get_service(global_name)
            if service_entity is None:
                raise RuntimeError(f"Service entity missing: {service_name}")
            service_config = service_entity.config or {}

            try:
                current_state = await self._registry.get_service_state_async(self._global_agent_store_id, global_name)
            except Exception:
                current_state = None

            effective_timeout = self._infer_transport_timeout(service_config)
            if current_state in (
                ServiceConnectionState.DEGRADED,
                ServiceConnectionState.CIRCUIT_OPEN,
                ServiceConnectionState.HALF_OPEN,
            ):
                effective_timeout = max(effective_timeout, self._warning_ping_timeout)

            try:
                async with asyncio.timeout(effective_timeout):
                    async with temp_client_for_service(global_name, service_config, timeout=effective_timeout) as client:
                        await client.ping()
                response_time = time.time() - start_time
                suggested, metrics = self._update_window_and_suggest(agent_id, service_name, True, response_time, current_state)
                self._reset_cooldown((agent_id, service_name))
                self._renew_lease((agent_id, service_name))
                await self._publish_health_check_success(
                    agent_id,
                    service_name,
                    response_time,
                    suggested_state=suggested,
                    metrics=metrics,
                    wait=wait,
                )
            except asyncio.TimeoutError:
                response_time = time.time() - start_time
                suggested, metrics = self._update_window_and_suggest(agent_id, service_name, False, response_time, current_state)
                suggested = self._update_cooldown((agent_id, service_name), suggested)
                await self._publish_health_check_failed(
                    agent_id,
                    service_name,
                    response_time,
                    "Health check timeout",
                    suggested_state=suggested,
                    metrics=metrics,
                    wait=wait,
                )
            except Exception as e:
                response_time = time.time() - start_time
                suggested, metrics = self._update_window_and_suggest(agent_id, service_name, False, response_time, current_state)
                suggested = self._update_cooldown((agent_id, service_name), suggested)
                await self._publish_health_check_failed(
                    agent_id,
                    service_name,
                    response_time,
                    str(e),
                    suggested_state=suggested,
                    metrics=metrics,
                    wait=wait,
                )
        except Exception as e:
            logger.error(f"[HEALTH] Execute health check error: {service_name} - {e}", exc_info=True)

    async def _publish_health_check_success(
        self,
        agent_id: str,
        service_name: str,
        response_time: float,
        suggested_state: Optional[ServiceConnectionState],
        metrics: Dict[str, Any],
        wait: bool = False,
    ):
        event = HealthCheckCompleted(
            agent_id=agent_id,
            service_name=service_name,
            success=True,
            response_time=response_time,
            suggested_state=suggested_state,
            window_error_rate=metrics.get("error_rate"),
            latency_p95=metrics.get("latency_p95"),
            latency_p99=metrics.get("latency_p99"),
            sample_size=metrics.get("sample_size"),
            retry_in=self._get_retry_in((agent_id, service_name)),
            hard_timeout_in=self._get_hard_timeout_in((agent_id, service_name)),
            lease_remaining=self._lease_remaining((agent_id, service_name)),
            next_retry_time=self._next_retry_time((agent_id, service_name)),
            hard_deadline=self._hard_deadline((agent_id, service_name)),
            lease_deadline=self._lease_deadline((agent_id, service_name)),
        )
        await self._event_bus.publish(event, wait=wait)

    async def _publish_health_check_failed(
        self,
        agent_id: str,
        service_name: str,
        response_time: float,
        error_message: str,
        suggested_state: Optional[ServiceConnectionState],
        metrics: Dict[str, Any],
        wait: bool = False,
    ):
        event = HealthCheckCompleted(
            agent_id=agent_id,
            service_name=service_name,
            success=False,
            response_time=response_time,
            error_message=error_message,
            suggested_state=suggested_state,
            window_error_rate=metrics.get("error_rate"),
            latency_p95=metrics.get("latency_p95"),
            latency_p99=metrics.get("latency_p99"),
            sample_size=metrics.get("sample_size"),
            retry_in=self._get_retry_in((agent_id, service_name)),
            hard_timeout_in=self._get_hard_timeout_in((agent_id, service_name)),
            lease_remaining=self._lease_remaining((agent_id, service_name)),
            next_retry_time=self._next_retry_time((agent_id, service_name)),
            hard_deadline=self._hard_deadline((agent_id, service_name)),
            lease_deadline=self._lease_deadline((agent_id, service_name)),
        )
        await self._event_bus.publish(event, wait=wait)

    async def _to_global_name_async(self, agent_id: str, service_name: str) -> str:
        try:
            mapping = await self._registry.get_global_name_from_agent_service_async(agent_id, service_name)
            return mapping or service_name
        except Exception:
            return service_name

    def _infer_transport_timeout(self, service_config: dict) -> float:
        transport = str(service_config.get("transport", "")).lower()
        if not transport and service_config.get("url"):
            transport = "http"
        if not transport and (service_config.get("command") or service_config.get("args")):
            transport = "stdio"
        if "sse" in transport:
            return self._ping_timeout_sse
        if "stdio" in transport:
            return self._ping_timeout_stdio
        return self._ping_timeout_http

    # === 内部：滑动窗口与状态建议 ===
    def _update_window_and_suggest(
        self,
        agent_id: str,
        service_name: str,
        success: bool,
        response_time: Optional[float],
        current_state: Optional[ServiceConnectionState],
    ) -> tuple[Optional[ServiceConnectionState], Dict[str, Any]]:
        key = (agent_id, service_name)
        win = self._windows.setdefault(key, deque())
        win.append((time.time(), success, response_time))
        # 保持窗口大小
        while len(win) > self._window_size:
            win.popleft()

        if current_state != ServiceConnectionState.HALF_OPEN:
            self._reset_half_open_stats(key)
            # 非启动态重置就绪计数
            if current_state != ServiceConnectionState.STARTUP:
                self._readiness_success.pop(key, None)
                self._readiness_failures.pop(key, None)

        if current_state == ServiceConnectionState.HALF_OPEN:
            stats = self._half_open_stats.setdefault(key, {"attempts": 0, "success": 0, "failure": 0})
            stats["attempts"] += 1
            if success:
                stats["success"] += 1
            else:
                stats["failure"] += 1

            if stats["attempts"] >= self._half_open_max_calls:
                rate = stats["success"] / stats["attempts"] if stats["attempts"] else 0.0
                self._reset_half_open_stats(key)
                if rate >= self._half_open_success_rate_threshold:
                    return ServiceConnectionState.HEALTHY, self._empty_metrics(win)
                # 半开失败：回退熔断，并加长退避
                self._update_cooldown(key, ServiceConnectionState.CIRCUIT_OPEN)
                return ServiceConnectionState.CIRCUIT_OPEN, self._empty_metrics(win)
            # 未达到配额，保持半开试探
            return None, self._empty_metrics(win)

        if len(win) < self._window_min_calls:
            if success and current_state == ServiceConnectionState.STARTUP:
                self._readiness_success[key] = self._readiness_success.get(key, 0) + 1
                self._readiness_failures.pop(key, None)
                if self._readiness_success[key] >= self._readiness_success_threshold:
                    return ServiceConnectionState.READY, self._empty_metrics(win)
            elif current_state == ServiceConnectionState.STARTUP:
                self._readiness_failures[key] = self._readiness_failures.get(key, 0) + 1
                self._readiness_success.pop(key, None)
                if self._readiness_failures[key] >= self._readiness_failure_threshold:
                    # 启动期多次失败直接熔断
                    return ServiceConnectionState.CIRCUIT_OPEN, self._empty_metrics(win)
            return None, self._empty_metrics(win)

        total = len(win)
        fail_count = sum(1 for _, s, _ in win if not s)
        error_rate = fail_count / total
        latencies = [rt for _, s, rt in win if s and rt is not None]
        latencies.sort()
        def _percentile(vals: list[float], p: float) -> float:
            if not vals:
                return 0.0
            k = max(0, min(len(vals) - 1, int(len(vals) * p) - 1))
            return vals[k]
        p95 = _percentile(latencies, 0.95)
        p99 = _percentile(latencies, 0.99)

        # 基础建议：高错误率或极端延迟 -> CIRCUIT_OPEN；中等延迟/少量错误 -> DEGRADED
        metrics = {
            "error_rate": error_rate,
            "latency_p95": p95,
            "latency_p99": p99,
            "sample_size": total,
        }

        if error_rate >= self._error_rate_threshold or p99 >= self._latency_p99_critical:
            return ServiceConnectionState.CIRCUIT_OPEN, metrics
        if error_rate > 0 or p95 >= self._latency_p95_warn:
            return ServiceConnectionState.DEGRADED, metrics
        return ServiceConnectionState.HEALTHY, metrics

    def _reset_cooldown(self, key: Tuple[str, str]) -> None:
        self._cooldowns.pop(key, None)
        self._reset_half_open_stats(key)
        self._readiness_success.pop(key, None)
        self._readiness_failures.pop(key, None)

    def _update_cooldown(self, key: Tuple[str, str], suggested: Optional[ServiceConnectionState]) -> Optional[ServiceConnectionState]:
        if suggested != ServiceConnectionState.CIRCUIT_OPEN:
            self._reset_cooldown(key)
            return suggested

        now = time.time()
        data = self._cooldowns.get(key, {})
        attempts = int(data.get("attempts", 0)) + 1
        backoff = min(self._backoff_base * (2 ** (attempts - 1)), self._backoff_max)
        if self._backoff_jitter > 0:
            backoff = backoff * (1 + self._backoff_jitter)
        next_retry = now + min(backoff, self._backoff_max_duration)
        hard_deadline = data.get("hard_deadline") or (now + self._reconnect_hard_timeout)
        self._cooldowns[key] = {"next_retry": next_retry, "attempts": attempts, "hard_deadline": hard_deadline}

        if now >= hard_deadline:
            # 硬超时立即建议断开
            return ServiceConnectionState.DISCONNECTED
        return suggested

    # 被动反馈入口：供调用链将真实调用结果写入窗口
    def record_passive_feedback(self, agent_id: str, service_name: str, success: bool, response_time: Optional[float]) -> None:
        self._update_window_and_suggest(agent_id, service_name, success, response_time, None)

    def _reset_half_open_stats(self, key: Tuple[str, str]) -> None:
        self._half_open_stats.pop(key, None)
        self._leases.pop(key, None)

    def _get_retry_in(self, key: Tuple[str, str]) -> Optional[float]:
        cooldown = self._cooldowns.get(key)
        if not cooldown:
            return None
        next_retry = cooldown.get("next_retry")
        if not next_retry:
            return None
        delta = next_retry - time.time()
        return max(delta, 0.0)

    def _get_hard_timeout_in(self, key: Tuple[str, str]) -> Optional[float]:
        cooldown = self._cooldowns.get(key)
        if not cooldown:
            return None
        hard_deadline = cooldown.get("hard_deadline")
        if not hard_deadline:
            return None
        delta = hard_deadline - time.time()
        return max(delta, 0.0)

    def _next_retry_time(self, key: Tuple[str, str]) -> Optional[float]:
        cooldown = self._cooldowns.get(key)
        if not cooldown:
            return None
        return cooldown.get("next_retry")

    def _hard_deadline(self, key: Tuple[str, str]) -> Optional[float]:
        cooldown = self._cooldowns.get(key)
        if cooldown and cooldown.get("hard_deadline"):
            return cooldown.get("hard_deadline")
        # 如果已记录硬超时标记，直接返回
        return self._hard_timeouts.get(key)

    def _renew_lease(self, key: Tuple[str, str]) -> None:
        if self._lease_ttl <= 0:
            return
        self._leases[key] = time.time() + self._lease_ttl

    def _lease_remaining(self, key: Tuple[str, str]) -> Optional[float]:
        ttl = self._leases.get(key)
        if not ttl:
            return None
        return max(ttl - time.time(), 0.0)

    def _lease_deadline(self, key: Tuple[str, str]) -> Optional[float]:
        ttl = self._leases.get(key)
        if not ttl:
            return None
        return ttl

    @staticmethod
    def _empty_metrics(win: Deque[Tuple[float, bool, Optional[float]]]) -> Dict[str, Any]:
        return {
            "error_rate": None,
            "latency_p95": None,
            "latency_p99": None,
            "sample_size": len(win),
        }
