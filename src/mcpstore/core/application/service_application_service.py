"""
服务应用服务 - 协调服务添加流程

职责:
1. 参数验证
2. 生成 client_id
3. 发布事件
4. 等待状态收敛（可选）
5. 返回结果给用户
"""

import asyncio
import logging
from dataclasses import dataclass
from datetime import datetime
from typing import Dict, Any, Optional

from mcpstore.core.eventlog.event_models import domain_event_to_record
from mcpstore.core.eventlog.event_syncer import EventSyncer
from mcpstore.core.events.event_bus import EventBus
from mcpstore.core.events.service_events import (
    ServiceAddRequested,
    ServiceInitialized,
    HealthCheckRequested,
    ServiceRestartRequested,
    ServiceResetRequested,
)
from mcpstore.core.models.service import ServiceConnectionState
from mcpstore.core.utils.id_generator import ClientIDGenerator

logger = logging.getLogger(__name__)


@dataclass
class AddServiceResult:
    """服务添加结果"""
    success: bool
    service_name: str
    client_id: str
    final_state: Optional[str] = None
    error_message: Optional[str] = None
    duration_ms: float = 0.0


class ServiceApplicationService:
    """
    服务应用服务 - 用户操作的协调器
    
    职责:
    1. 参数验证
    2. 生成 client_id
    3. 发布事件
    4. 等待状态收敛（可选）
    5. 返回结果给用户
    """
    
    def __init__(
        self,
        event_bus: EventBus,
        registry: 'CoreRegistry',
        lifecycle_manager: 'LifecycleManager',
        global_agent_store_id: str,
        event_store=None,
        enable_event_log: bool = True,
        is_only_db: bool = False,
    ):
        self._event_bus = event_bus
        self._registry = registry
        self._lifecycle_manager = lifecycle_manager
        self._global_agent_store_id = global_agent_store_id
        self._event_store = event_store
        self._enable_event_log = bool(event_store is not None and enable_event_log)
        self._is_only_db = is_only_db
        # 事件入队重试参数（可按需调整或配置注入）
        self._event_queue_retry_attempts = 3
        self._event_queue_retry_initial_delay = 0.2
        self._event_queue_retry_backoff = 2.0
        # 同步消费器（可选注入，local_db 模式下由容器 attach）
        self._event_syncer: EventSyncer | None = None
        
        logger.info("ServiceApplicationService initialized")

    async def _queue_event_with_retry(self, event, dedup_key: str) -> bool:
        """
        将事件写入事件队列，带有限重试；不回退本地。

        返回 True 表示已入队；返回 False 表示重试后仍失败（需调用方处理）。
        """
        if not (self._enable_event_log and self._event_store):
            return False
        last_error: Exception | None = None
        delay = self._event_queue_retry_initial_delay
        for attempt in range(1, self._event_queue_retry_attempts + 1):
            try:
                record = domain_event_to_record(
                    event,
                    source=getattr(event, "source", "user"),
                    dedup_key=dedup_key,
                )
                record = await self._event_store.append_event(record)  # type: ignore[union-attr]
                logger.info(
                    "[EVENT_QUEUE] queued id=%s type=%s dedup=%s attempt=%s",
                    getattr(record, "id", None),
                    getattr(record, "type", type(event).__name__),
                    dedup_key,
                    attempt,
                )
                return True
            except Exception as log_err:
                last_error = log_err
                logger.error(
                    "[EVENT_QUEUE] append failed (attempt %s/%s) type=%s dedup=%s error=%s",
                    attempt,
                    self._event_queue_retry_attempts,
                    type(event).__name__,
                    dedup_key,
                    log_err,
                    exc_info=True,
                )
                if attempt < self._event_queue_retry_attempts:
                    try:
                        await asyncio.sleep(delay)
                        delay *= self._event_queue_retry_backoff
                    except Exception:
                        pass
        logger.error(
            "[EVENT_QUEUE] append abandoned after %s attempts type=%s dedup=%s last_error=%s",
            self._event_queue_retry_attempts,
            type(event).__name__,
            dedup_key,
            last_error,
        )
        return False

    async def _sync_consume_if_available(self, target_id: Optional[str]) -> None:
        """
        在 local_db 模式下尝试同步消费一次事件队列，直至 offset 覆盖 target_id 或达到有限次数。
        - 仅在存在 event_syncer 时执行，不添加硬编码 sleep。
        """
        if not self._event_syncer or not target_id:
            return
        max_rounds = 3
        for _ in range(max_rounds):
            offset = await self._event_syncer.consume_once_now()
            try:
                if offset is not None and int(offset) >= int(target_id):
                    return
            except Exception:
                return
    
    async def add_service(
        self,
        agent_id: str,
        service_name: str,
        service_config: Dict[str, Any],
        wait_timeout: float = 0.0,
        source: str = "user",
        global_name: Optional[str] = None,
        client_id: Optional[str] = None,
        origin_agent_id: Optional[str] = None,
        origin_local_name: Optional[str] = None,
    ) -> AddServiceResult:
        """
        添加服务（用户API）
        
        Args:
            agent_id: Agent ID
            service_name: 服务名称
            service_config: 服务配置
            wait_timeout: 等待超时（0表示不等待）
            source: 调用来源
            
        Returns:
            AddServiceResult: 添加结果
        """
        start_time = asyncio.get_event_loop().time()
        
        try:
            # 1. 参数验证
            self._validate_params(service_name, service_config)
            
            # 2. 生成 client_id
            cid = client_id or await self._generate_client_id(agent_id, service_name, service_config)
            
            logger.info(
                f"[ADD_SERVICE] Starting: service={service_name}, "
                f"agent={agent_id}, client_id={cid}"
            )
            
            # 3. 发布服务添加请求事件
            event = ServiceAddRequested(
                agent_id=agent_id,
                service_name=service_name,
                service_config=service_config,
                client_id=cid,
                global_name=global_name or "",
                origin_agent_id=origin_agent_id,
                origin_local_name=origin_local_name,
                source=source,
                wait_timeout=wait_timeout
            )

            published = False
            if self._enable_event_log:
                try:
                    dedup_key = self._build_dedup_key(event)
                    record = domain_event_to_record(
                        event,
                        source=source,
                        dedup_key=dedup_key,
                    )
                    record = await self._event_store.append_event(record)  # type: ignore[union-attr]
                    published = True
                    logger.info(f"[ADD_SERVICE] Event logged to queue: id={record.id} dedup={dedup_key}")
                    # local_db 场景：写后尝试同步消费，提高可见性
                    await self._sync_consume_if_available(getattr(record, "id", None))
                except Exception as log_error:
                    logger.error(f"[ADD_SERVICE] Failed to append event to queue, fallback to local bus: {log_error}", exc_info=True)

            if not published:
                await self._event_bus.publish(event, wait=False)
            
            # 4. 等待状态收敛（可选）
            final_state = None
            if wait_timeout > 0:
                final_state = await self._wait_for_state_convergence(
                    agent_id, service_name, wait_timeout
                )
            
            duration_ms = (asyncio.get_event_loop().time() - start_time) * 1000
            
            logger.info(
                f"[ADD_SERVICE] Completed: service={service_name}, "
                f"state={final_state}, duration={duration_ms:.2f}ms"
            )
            
            return AddServiceResult(
                success=True,
                service_name=service_name,
                client_id=cid,
                final_state=final_state,
                duration_ms=duration_ms
            )
            
        except Exception as e:
            duration_ms = (asyncio.get_event_loop().time() - start_time) * 1000
            logger.error(f"[ADD_SERVICE] Failed: service={service_name}, error={e}", exc_info=True)
            
            return AddServiceResult(
                success=False,
                service_name=service_name,
                client_id="",
                error_message=str(e),
                duration_ms=duration_ms
            )

    async def restart_service(
        self,
        service_name: str,
        agent_id: Optional[str] = None,
        wait_timeout: float = 0.0,
        *,
        from_event: bool = False,
        source: str = "user",
    ) -> bool:
        """重启服务（应用层 API）

        - 通过 LifecycleManager 将状态迁移到 STARTUP；
        - 重置基础元数据计数器；
        - 发布 ServiceInitialized + HealthCheckRequested 事件；
        - 可选：等待状态从 STARTUP 收敛到其他状态。
        """
        start_time = asyncio.get_event_loop().time()
        agent_key = agent_id or self._global_agent_store_id

        # 用户入口（非事件消费）统一走事件层；不回退本地，入队失败抛出异常
        if self._enable_event_log and not from_event:
            event = ServiceRestartRequested(agent_id=agent_key, service_name=service_name, source=source)
            queued = await self._queue_event_with_retry(
                event,
                dedup_key=f"{event.__class__.__name__}:{service_name}",
            )
            if not queued:
                raise RuntimeError(f"Queue write failed: {event.__class__.__name__} {service_name}")
            return True

        try:
            # 1. 校验服务是否存在（使用异步 API）
            if not await self._registry.has_service_async(agent_key, service_name):
                logger.warning(
                    f"[RESTART_SERVICE_APP] Service '{service_name}' not found for agent {agent_key}"
                )
                return False

            # 2. 读取并校验元数据 - 从 pykv 异步获取
            metadata = await self._registry._service_state_service.get_service_metadata_async(agent_key, service_name)
            if not metadata:
                logger.error(
                    f"[RESTART_SERVICE_APP] No metadata found for service '{service_name}' (agent={agent_key})"
                )
                return False

            # 幂等短路：若已在 STARTUP/READY/HEALTHY 状态，可选择直接返回（避免重复重启）
            try:
                current_state = await self._registry.get_service_state_async(agent_key, service_name)
                if current_state in (
                    ServiceConnectionState.STARTUP,
                    ServiceConnectionState.READY,
                    ServiceConnectionState.HEALTHY,
                ):
                    logger.info(
                        f"[RESTART_SERVICE_APP] Skip restart (state={getattr(current_state, 'value', current_state)}) "
                        f"service='{service_name}' agent={agent_key}"
                    )
                    return True
            except Exception as state_err:
                logger.debug(f"[RESTART_SERVICE_APP] state check failed: {state_err}")

            # 3. 通过 LifecycleManager 统一入口迁移到 STARTUP
            await self._lifecycle_manager._transition_state(
                agent_id=agent_key,
                service_name=service_name,
                new_state=ServiceConnectionState.STARTUP,
                reason="restart_service",
                source="ServiceApplicationService",
            )

            # 4. 重置元数据计数器
            metadata.consecutive_failures = 0
            metadata.consecutive_successes = 0
            metadata.reconnect_attempts = 0
            metadata.error_message = None
            metadata.state_entered_time = datetime.now()
            metadata.next_retry_time = None
            self._registry.set_service_metadata(agent_key, service_name, metadata)

            # 5. 发布初始化完成 + 一次性健康检查请求事件
            initialized_event = ServiceInitialized(
                agent_id=agent_key,
                service_name=service_name,
                initial_state="startup",
            )
            await self._event_bus.publish(initialized_event, wait=True)

            health_check_event = HealthCheckRequested(
                agent_id=agent_key,
                service_name=service_name,
            )
            await self._event_bus.publish(health_check_event, wait=True)

            # 6. 可选：等待状态收敛
            if wait_timeout > 0:
                final_state = await self._wait_for_state_convergence(
                    agent_key, service_name, wait_timeout
                )
                logger.info(
                    f"[RESTART_SERVICE_APP] Completed restart for '{service_name}' "
                    f"state={final_state} agent={agent_key}"
                )
            else:
                logger.info(
                    f"[RESTART_SERVICE_APP] Restart triggered for '{service_name}' "
                    f"(no wait, agent={agent_key})"
                )

            duration_ms = (asyncio.get_event_loop().time() - start_time) * 1000
            try:
                logger.debug(
                    f"[RESTART_SERVICE_APP] duration={duration_ms:.2f}ms service='{service_name}' agent={agent_key}"
                )
            except Exception:
                pass

            return True

        except Exception as e:
            logger.error(
                f"[RESTART_SERVICE_APP] Failed to restart service '{service_name}' (agent={agent_key}): {e}",
                exc_info=True,
            )
            return False
    
    async def reset_service(
        self,
        agent_id: str,
        service_name: str,
        wait_timeout: float = 0.0,
        *,
        from_event: bool = False,
        source: str = "user",
    ) -> bool:
        start_time = asyncio.get_event_loop().time()

        # 用户入口（非事件消费）统一走事件层；不回退本地，入队失败抛出异常
        if self._enable_event_log and not from_event:
            event = ServiceResetRequested(agent_id=agent_id, service_name=service_name, source=source)
            queued = await self._queue_event_with_retry(
                event,
                dedup_key=f"{event.__class__.__name__}:{service_name}",
            )
            if not queued:
                raise RuntimeError(f"Queue write failed: {event.__class__.__name__} {service_name}")
            return True

        try:
            # 使用异步 API 检查服务是否存在
            if not await self._registry.has_service_async(agent_id, service_name):
                logger.warning(
                    f"[RESET_SERVICE_APP] Service '{service_name}' not found for agent {agent_id}"
                )
                return False

            # 幂等短路：如果已在初始化/健康态且配置存在，可直接视为成功
            try:
                current_state = await self._registry.get_service_state_async(agent_id, service_name)
                if current_state in (
                    ServiceConnectionState.STARTUP,
                    ServiceConnectionState.READY,
                    ServiceConnectionState.HEALTHY,
                ):
                    logger.info(
                        f"[RESET_SERVICE_APP] Skip reset (state={getattr(current_state, 'value', current_state)}) "
                        f"service='{service_name}' agent={agent_id}"
                    )
                    return True
            except Exception as state_err:
                logger.debug(f"[RESET_SERVICE_APP] state check failed: {state_err}")

            service_config = await self._registry.get_service_config_from_cache_async(agent_id, service_name)
            if not service_config:
                logger.error(
                    f"[RESET_SERVICE_APP] No service config found for '{service_name}' (agent={agent_id})"
                )
                return False

            success = await self._lifecycle_manager.initialize_service(
                agent_id=agent_id,
                service_name=service_name,
                service_config=service_config,
            )
            if not success:
                logger.error(
                    f"[RESET_SERVICE_APP] initialize_service returned False for '{service_name}' (agent={agent_id})"
                )
                return False

            if wait_timeout > 0:
                final_state = await self._wait_for_state_convergence(
                    agent_id, service_name, wait_timeout
                )
                logger.info(
                    f"[RESET_SERVICE_APP] Completed reset for '{service_name}' "
                    f"state={final_state} agent={agent_id}"
                )
            else:
                logger.info(
                    f"[RESET_SERVICE_APP] Reset triggered for '{service_name}' "
                    f"(no wait, agent={agent_id})"
                )

            duration_ms = (asyncio.get_event_loop().time() - start_time) * 1000
            try:
                logger.debug(
                    f"[RESET_SERVICE_APP] duration={duration_ms:.2f}ms service='{service_name}' agent={agent_id}"
                )
            except Exception:
                pass

            return True

        except Exception as e:
            logger.error(
                f"[RESET_SERVICE_APP] Failed to reset service '{service_name}' (agent={agent_id}): {e}",
                exc_info=True,
            )
            return False
    
    async def get_service_status_async(self, agent_id: str, service_name: str) -> Dict[str, Any]:
        """读取单个服务的状态信息（只读，从 pykv 异步获取）"""
        try:
            state = await self._registry._service_state_service.get_service_state_async(agent_id, service_name)
            metadata = await self._registry._service_state_service.get_service_metadata_async(agent_id, service_name)
            client_id = await self._registry.get_service_client_id_async(agent_id, service_name)

            status_response: Dict[str, Any] = {
                "service_name": service_name,
                "agent_id": agent_id,
                "client_id": client_id,
            }

            # 状态与健康度
            if state:
                status_response["status"] = getattr(state, "value", str(state))
                status_response["healthy"] = state in [
                    ServiceConnectionState.HEALTHY,
                    ServiceConnectionState.DEGRADED,
                ]
            else:
                status_response["status"] = "unknown"
                status_response["healthy"] = False

            # 元数据
            if metadata:
                status_response["last_check"] = (
                    metadata.last_health_check.timestamp()
                    if getattr(metadata, "last_health_check", None)
                    else None
                )
                status_response["response_time"] = getattr(
                    metadata, "last_response_time", None
                )
                status_response["error"] = getattr(metadata, "error_message", None)
                status_response["consecutive_failures"] = getattr(
                    metadata, "consecutive_failures", 0
                )
                status_response["state_entered_time"] = (
                    metadata.state_entered_time.timestamp()
                    if getattr(metadata, "state_entered_time", None)
                    else None
                )
            else:
                status_response.setdefault("last_check", None)
                status_response.setdefault("response_time", None)
                status_response.setdefault("error", None)
                status_response.setdefault("consecutive_failures", 0)
                status_response.setdefault("state_entered_time", None)

            logger.info(
                f"[GET_STATUS_APP] service='{service_name}' agent='{agent_id}' "
                f"status='{status_response.get('status')}' healthy={status_response.get('healthy')}"
            )
            return status_response

        except Exception as e:
            logger.error(
                f"[GET_STATUS_APP] Failed to get status for service '{service_name}' (agent={agent_id}): {e}",
                exc_info=True,
            )
            return {
                "service_name": service_name,
                "agent_id": agent_id,
                "client_id": None,
                "status": "error",
                "healthy": False,
                "last_check": None,
                "response_time": None,
                "error": str(e),
                "consecutive_failures": 0,
                "state_entered_time": None,
            }
    
    def _validate_params(self, service_name: str, service_config: Dict[str, Any]):
        """验证参数"""
        if not service_name:
            raise ValueError("service_name cannot be empty")
        
        if not service_config:
            raise ValueError("service_config cannot be empty")
        
        # 验证必要字段
        if "command" not in service_config and "url" not in service_config:
            raise ValueError("service_config must contain 'command' or 'url'")

    def _build_dedup_key(self, event: ServiceAddRequested) -> str:
        """构造去重键：事件类型 + 全局服务名。"""
        global_name = event.global_name or event.service_name
        return f"{event.__class__.__name__}:{global_name}"

    async def _generate_client_id(
        self,
        agent_id: str,
        service_name: str,
        service_config: Dict[str, Any]
    ) -> str:
        """生成 client_id（优先异步获取已有映射，避免事件循环冲突）"""
        # 优先使用异步 API，避免在运行事件循环中调用同步桥接
        existing_client_id = None
        try:
            existing_client_id = await self._registry.get_service_client_id_async(agent_id, service_name)
        except Exception as e:
            logger.warning(f"Failed to get existing client_id asynchronously: {e}")

        if existing_client_id:
            logger.debug(f"Using existing client_id: {existing_client_id}")
            return existing_client_id

        # 生成新的
        client_id = ClientIDGenerator.generate_deterministic_id(
            agent_id=agent_id,
            service_name=service_name,
            service_config=service_config,
            global_agent_store_id=self._global_agent_store_id
        )

        logger.debug(f"Generated new client_id: {client_id}")
        return client_id
    
    async def _wait_for_state_convergence(
        self,
        agent_id: str,
        service_name: str,
        timeout: float
    ) -> Optional[str]:
        """
        等待服务状态收敛
        
        状态收敛定义: 状态不再是 STARTUP
        """
        logger.debug(f"[WAIT_STATE] Waiting for {service_name} (timeout={timeout}s)")
        
        start_time = asyncio.get_event_loop().time()
        check_interval = 0.1  # 100ms
        
        while True:
            # 检查超时
            elapsed = asyncio.get_event_loop().time() - start_time
            if elapsed >= timeout:
                logger.warning(f"[WAIT_STATE] Timeout for {service_name}")
                break
            
            # 检查状态
            state = self._registry._service_state_service.get_service_state(agent_id, service_name)
            if state and state != ServiceConnectionState.STARTUP:
                logger.debug(f"[WAIT_STATE] Converged: {service_name} -> {state.value}")
                return state.value
            
            # 等待一段时间再检查
            await asyncio.sleep(check_interval)
        
        # 超时，返回当前状态
        state = self._registry._service_state_service.get_service_state(agent_id, service_name)
        return state.value if state else "unknown"
