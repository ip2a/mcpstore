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

from mcpstore.core.events.event_bus import EventBus
from mcpstore.core.events.service_events import (
    ServiceAddRequested,
    ServiceInitialized,
    HealthCheckRequested,
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
        global_agent_store_id: str
    ):
        self._event_bus = event_bus
        self._registry = registry
        self._lifecycle_manager = lifecycle_manager
        self._global_agent_store_id = global_agent_store_id
        
        logger.info("ServiceApplicationService initialized")
    
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
                client_id=client_id,
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
    ) -> bool:
        """重启服务（应用层 API）

        - 通过 LifecycleManager 将状态迁移到 STARTUP；
        - 重置基础元数据计数器；
        - 发布 ServiceInitialized + HealthCheckRequested 事件；
        - 可选：等待状态从 STARTUP 收敛到其他状态。
        """
        start_time = asyncio.get_event_loop().time()
        agent_key = agent_id or self._global_agent_store_id

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
    ) -> bool:
        start_time = asyncio.get_event_loop().time()

        try:
            # 使用异步 API 检查服务是否存在
            if not await self._registry.has_service_async(agent_id, service_name):
                logger.warning(
                    f"[RESET_SERVICE_APP] Service '{service_name}' not found for agent {agent_id}"
                )
                return False

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
