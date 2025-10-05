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
from typing import Dict, Any, Optional

from mcpstore.core.events.event_bus import EventBus
from mcpstore.core.events.service_events import ServiceAddRequested
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
        global_agent_store_id: str
    ):
        self._event_bus = event_bus
        self._registry = registry
        self._global_agent_store_id = global_agent_store_id
        
        logger.info("ServiceApplicationService initialized")
    
    async def add_service(
        self,
        agent_id: str,
        service_name: str,
        service_config: Dict[str, Any],
        wait_timeout: float = 0.0,
        source: str = "user"
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
            client_id = self._generate_client_id(agent_id, service_name, service_config)
            
            logger.info(
                f"[ADD_SERVICE] Starting: service={service_name}, "
                f"agent={agent_id}, client_id={client_id}"
            )
            
            # 3. 发布服务添加请求事件
            event = ServiceAddRequested(
                agent_id=agent_id,
                service_name=service_name,
                service_config=service_config,
                client_id=client_id,
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
    
    def _validate_params(self, service_name: str, service_config: Dict[str, Any]):
        """验证参数"""
        if not service_name:
            raise ValueError("service_name cannot be empty")
        
        if not service_config:
            raise ValueError("service_config cannot be empty")
        
        # 验证必要字段
        if "command" not in service_config and "url" not in service_config:
            raise ValueError("service_config must contain 'command' or 'url'")
    
    def _generate_client_id(
        self, 
        agent_id: str, 
        service_name: str, 
        service_config: Dict[str, Any]
    ) -> str:
        """生成 client_id"""
        # 检查是否已存在
        existing_client_id = self._registry.get_service_client_id(agent_id, service_name)
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
        
        状态收敛定义: 状态不再是 INITIALIZING
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
            state = self._registry.get_service_state(agent_id, service_name)
            if state and state != ServiceConnectionState.INITIALIZING:
                logger.debug(f"[WAIT_STATE] Converged: {service_name} -> {state.value}")
                return state.value
            
            # 等待一段时间再检查
            await asyncio.sleep(check_interval)
        
        # 超时，返回当前状态
        state = self._registry.get_service_state(agent_id, service_name)
        return state.value if state else "unknown"

