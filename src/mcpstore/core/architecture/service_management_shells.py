"""
Service Management Shells - 双路外壳实现

异步外壳和同步外壳的完整实现，严格遵循Functional Core, Imperative Shell架构原则。
"""

import asyncio
import logging
from typing import Dict, Any, Optional

from .service_management_core import ServiceManagementCore
from ..bridge import get_async_bridge

logger = logging.getLogger(__name__)


class ServiceManagementAsyncShell:
    """
    异步外壳：执行所有IO操作

    特点：
    - 只在入口处调用一次核心逻辑
    - 之后纯异步执行，不再有任何同步/异步混用
    - 直接调用pykv异步方法，避免_sync_to_kv
    """

    def __init__(self, core: ServiceManagementCore, registry, orchestrator):
        """
        初始化异步外壳

        Args:
            core: 纯同步核心逻辑实例
            registry: ServiceRegistry实例
            orchestrator: MCPOrchestrator实例
        """
        self.core = core
        self.registry = registry
        self.orchestrator = orchestrator
        try:
            # 优先复用当前事件循环
            self._loop = asyncio.get_event_loop()
        except RuntimeError:
            # 主线程未创建事件循环时主动创建并设置，避免 RuntimeError
            self._loop = asyncio.new_event_loop()
            asyncio.set_event_loop(self._loop)
            logger.debug("[ASYNC_SHELL] [INIT] Created new event loop for main thread")
        logger.debug("[ASYNC_SHELL] [INIT] Initializing ServiceManagementAsyncShell")

    async def add_service_async(self, config: Dict[str, Any]) -> Dict[str, Any]:
        """
        异步外壳：执行服务添加

        严格按照新架构原则：
        1. 调用纯同步核心（无IO，无锁，无异步）
        2. 纯异步执行所有pykv操作
        3. 避免任何_sync_to_kv调用

        修复：使用正确的缓存层管理器方法
        """
        logger.debug("[ASYNC_SHELL] [START] Starting to add service")

        try:
            # 1. 调用纯同步核心（这是唯一可能调用同步逻辑的地方）
            operation_plan = self.core.add_service(config)
            logger.debug(f"[ASYNC_SHELL] [PLAN] Got operation plan: {len(operation_plan.operations)} operations")

            # 2. 获取正确的缓存层管理器
            # 优先使用 cache/ 目录下的管理器（直接操作 pykv）
            # 这些管理器是数据的唯一真相源
            service_manager = getattr(self.registry, '_cache_service_manager', None)
            relation_manager = getattr(self.registry, '_relation_manager', None)
            state_manager = getattr(self.registry, '_cache_state_manager', None)

            # 如果缓存层管理器不存在，抛出错误（不做降级处理）
            if service_manager is None:
                raise RuntimeError(
                    "Cache layer ServiceEntityManager not initialized. "
                    "Please ensure ServiceRegistry correctly initializes the _cache_service_manager attribute."
                )

            # 3. 纯异步执行所有操作
            results = []
            successful_operations = []

            for i, operation in enumerate(operation_plan.operations):
                logger.debug(f"[ASYNC_SHELL] [EXEC] Executing operation {i+1}/{len(operation_plan.operations)}: {operation.type}")

                try:
                    if operation.type == "put_entity":
                        # 使用 cache/ServiceEntityManager 创建服务实体
                        await service_manager.create_service(
                            agent_id=operation.data.get("agent_id", "global_agent_store"),
                            original_name=operation.data.get("original_name", operation.data["key"]),
                            config=operation.data.get("config", operation.data.get("value", {}))
                        )
                        logger.debug(f"[ASYNC_SHELL] [SUCCESS] create_service successful, key={operation.data['key']}")
                        successful_operations.append(operation)
                        results.append({"operation": operation.key, "status": "success"})

                    elif operation.type == "put_relation":
                        # 使用 cache/RelationshipManager 创建关系
                        if relation_manager is None:
                            raise RuntimeError(
                                "Cache layer RelationshipManager not initialized. "
                                "Please ensure ServiceRegistry correctly initializes the _relation_manager attribute."
                            )
                        await relation_manager.add_agent_service(
                            agent_id=operation.data.get("agent_id", "global_agent_store"),
                            service_original_name=operation.data.get("service_original_name", ""),
                            service_global_name=operation.data.get("service_global_name", operation.data["key"]),
                            client_id=operation.data.get("client_id", f"client_{operation.data['key']}")
                        )
                        logger.debug(f"[ASYNC_SHELL] [SUCCESS] add_agent_service successful, key={operation.data['key']}")
                        successful_operations.append(operation)
                        results.append({"operation": operation.key, "status": "success"})

                    elif operation.type == "update_state":
                        # 使用 cache/StateManager 更新状态
                        if state_manager is None:
                            raise RuntimeError(
                                "Cache layer StateManager not initialized. "
                                "Please ensure ServiceRegistry correctly initializes the _cache_state_manager attribute."
                            )
                        await state_manager.update_service_status(
                            service_global_name=operation.data["key"],
                            health_status=operation.data.get("health_status", "startup"),
                            tools_status=operation.data.get("tools_status", [])
                        )
                        logger.debug(f"[ASYNC_SHELL] [SUCCESS] update_state successful, key={operation.data['key']}")
                        successful_operations.append(operation)
                        results.append({"operation": operation.key, "status": "success"})

                    elif operation.type == "put_metadata":
                        cache_layer = getattr(self.registry, "_cache_layer_manager", None)
                        if cache_layer is None:
                            raise RuntimeError("Cache layer CacheLayerManager is not initialized.")
                        await cache_layer.put_state(
                            "service_metadata",
                            operation.data["key"],
                            operation.data.get("value", {})
                        )
                        logger.debug(f"[ASYNC_SHELL] [SUCCESS] put_metadata successful, key={operation.data['key']}")
                        successful_operations.append(operation)
                        results.append({"operation": operation.key, "status": "success"})

                    else:
                        raise ValueError(f"Unknown operation type: {operation.type}")

                except Exception as e:
                    logger.error(f"[ASYNC_SHELL] [ERROR] Operation failed {operation.key}: {e}")
                    results.append({"operation": operation.key, "status": "failed", "error": str(e)})
                    # 按要求抛出错误，不做静默处理
                    raise

            # 服务的实际连接交由事件驱动流程（ServiceAddRequested → ServiceCached → ServiceInitialized → ConnectionManager）
            logger.info(f"[ASYNC_SHELL] [COMPLETE] Service addition completed: {len(operation_plan.service_names)} services, {len([r for r in results if r['status'] == 'success'])} successful")

            # 4. 发布 ServiceAddRequested 事件，触发事件驱动的连接流程
            # 这是关键修复：确保连接流程被触发
            try:
                # 获取 event_bus（优先从 orchestrator.container 获取，否则从 orchestrator 获取）
                event_bus = None
                if self.orchestrator:
                    event_bus = getattr(getattr(self.orchestrator, 'container', None), '_event_bus', None)
                    if event_bus is None:
                        event_bus = getattr(self.orchestrator, 'event_bus', None)
                
                if event_bus is None:
                    logger.warning("[ASYNC_SHELL] [WARN] EventBus unavailable, cannot publish ServiceAddRequested event. Connection flow may not start.")
                else:
                    # 为每个成功添加的服务发布 ServiceAddRequested 事件
                    from mcpstore.core.events.service_events import ServiceAddRequested
                    
                    for service_name in operation_plan.service_names:
                        # 从 operations 中提取服务信息
                        service_info = None
                        client_id = None
                        agent_id = None
                        service_config = None
                        
                        # 查找 put_entity 操作获取服务配置
                        for op in operation_plan.operations:
                            if op.type == "put_entity" and op.data.get("original_name") == service_name:
                                agent_id = op.data.get("agent_id", "global_agent_store")
                                service_config = op.data.get("config", {})
                                break
                        
                        # 查找 put_relation 操作获取 client_id
                        for op in operation_plan.operations:
                            if op.type == "put_relation" and op.data.get("service_original_name") == service_name:
                                client_id = op.data.get("client_id")
                                if agent_id is None:
                                    agent_id = op.data.get("agent_id", "global_agent_store")
                                break
                        
                        # 如果找不到 service_config，尝试从 config 参数中获取
                        if not service_config:
                            # 从原始 config 中提取
                            if isinstance(config, dict):
                                if "mcpServers" in config:
                                    service_config = config["mcpServers"].get(service_name, {})
                                elif "name" in config and config.get("name") == service_name:
                                    service_config = {k: v for k, v in config.items() if k != "name"}
                                else:
                                    service_config = config
                        
                        # 确保有 service_config 才能生成 client_id
                        if not service_config:
                            raise RuntimeError(f"Unable to get service configuration, cannot generate client_id: {service_name}")
                        
                        # 如果找不到 client_id，使用 ClientIDGenerator 生成一个
                        if client_id is None:
                            from mcpstore.core.utils.id_generator import ClientIDGenerator
                            global_agent_store_id = getattr(getattr(self.orchestrator, 'client_manager', None), 'global_agent_store_id', 'global_agent_store')
                            client_id = ClientIDGenerator.generate_deterministic_id(
                                agent_id=agent_id or "global_agent_store",
                                service_name=service_name,
                                service_config=service_config,
                                global_agent_store_id=global_agent_store_id
                            )
                        
                        if service_config:
                            # 发布 ServiceAddRequested 事件
                            add_event = ServiceAddRequested(
                                agent_id=agent_id or "global_agent_store",
                                service_name=service_name,
                                service_config=service_config,
                                client_id=client_id,
                                source="service_management_shell",
                                wait_timeout=0.0
                            )
                            await event_bus.publish(add_event, wait=True)
                            
                            logger.info(f"[ASYNC_SHELL] [EVENT] ServiceAddRequested event published: {service_name} (agent={agent_id or 'global_agent_store'})")
                        else:
                            logger.warning(f"[ASYNC_SHELL] [WARN] Cannot publish ServiceAddRequested event for service {service_name}: service configuration not found")
            except Exception as event_error:
                logger.error(f"[ASYNC_SHELL] [ERROR] Failed to publish ServiceAddRequested event: {event_error}", exc_info=True)
                # 不抛出异常，允许服务添加成功返回，但记录错误

            return {
                "success": True,
                "added_services": operation_plan.service_names,
                "operations": results,
                "total_operations": len(operation_plan.operations),
                "successful_operations": len([r for r in results if r['status'] == 'success'])
            }

        except Exception as e:
            logger.error(f"[ASYNC_SHELL] [ERROR] add_service_async failed: {e}")
            return {
                "success": False,
                "error": str(e),
                "added_services": [],
                "operations": [],
                "total_operations": 0,
                "successful_operations": 0
            }

    async def wait_service_async(self, service_name: str, timeout: float | None = None):
        """
        异步外壳：等待服务就绪

        严格按照新架构原则：
        1. 调用纯同步核心生成等待计划
        2. 纯异步执行状态检查循环
        3. 直接从pykv读取状态，使用缓存层管理器
        """
        logger.debug(f"[ASYNC_SHELL] [WAIT] Starting to wait for service: {service_name}, timeout={timeout}")

        try:
            # 1. 如未显式传入，按传输类型选择默认超时
            effective_timeout = timeout

            # 先生成计划以获取全局名，再读取配置推断传输
            draft_plan = self.core.wait_service_plan(service_name, timeout or 0)
            agent_id = getattr(self.core, "agent_id", "global_agent_store") or "global_agent_store"
            if effective_timeout is None:
                try:
                    info = await self.registry.get_complete_service_info_async(agent_id, draft_plan.global_name)
                    cfg = info.get("config", {}) if isinstance(info, dict) else {}
                    transport = str(cfg.get("transport", "")).lower()
                    if not transport and cfg.get("url"):
                        transport = "http"
                    if not transport and (cfg.get("command") or cfg.get("args")):
                        transport = "stdio"
                    # 从生命周期配置获取对应超时
                    lm = getattr(self.orchestrator, "lifecycle_manager", None)
                    lc = getattr(lm, "_config", None)
                    def _get(name, default):
                        return getattr(lc, name, default) if lc else default
                    if "sse" in transport:
                        effective_timeout = _get("ping_timeout_sse", 20.0)
                    elif "stdio" in transport:
                        effective_timeout = _get("ping_timeout_stdio", 40.0)
                    else:
                        effective_timeout = _get("ping_timeout_http", 20.0)
                except Exception:
                    effective_timeout = timeout or 20.0

            # 2. 调用纯同步核心（使用计算后的超时）
            wait_plan = self.core.wait_service_plan(service_name, effective_timeout)
            logger.debug(f"[ASYNC_SHELL] [PLAN] Wait plan: {wait_plan}")

            # 2. 获取缓存层状态管理器
            # cache/state_manager.py 的方法签名是 get_service_status(service_global_name)
            state_manager = getattr(self.registry, '_cache_state_manager', None)
            if state_manager is None:
                raise RuntimeError(
                    "Cache layer StateManager not initialized. "
                    "Please ensure ServiceRegistry correctly initializes the _cache_state_manager attribute."
                )

            # 3. 纯异步等待检查
            start_time = self._loop.time()

            while True:
                try:
                    state_data = await state_manager.get_service_status(wait_plan.global_name)

                    health_status = "unknown"
                    window_metrics = None
                    retry_in = None
                    hard_timeout_in = None
                    if state_data:
                        if hasattr(state_data, 'health_status'):
                            health_status = state_data.health_status
                            window_metrics = {
                                "error_rate": getattr(state_data, "window_error_rate", None),
                                "latency_p95": getattr(state_data, "latency_p95", None),
                                "latency_p99": getattr(state_data, "latency_p99", None),
                                "sample_size": getattr(state_data, "sample_size", None),
                            }
                            retry_in = self._remaining_seconds_timestamp(getattr(state_data, "next_retry_time", None))
                            hard_timeout_in = self._remaining_seconds_timestamp(getattr(state_data, "hard_deadline", None))
                            lease_remaining = self._remaining_seconds_timestamp(getattr(state_data, "lease_deadline", None))
                        elif hasattr(state_data, 'get'):
                            health_status = state_data.get("health_status", "unknown")
                            window_metrics = {
                                "error_rate": state_data.get("window_error_rate"),
                                "latency_p95": state_data.get("latency_p95"),
                                "latency_p99": state_data.get("latency_p99"),
                                "sample_size": state_data.get("sample_size"),
                            }
                            retry_in = self._remaining_seconds_timestamp(state_data.get("next_retry_time"))
                            hard_timeout_in = self._remaining_seconds_timestamp(state_data.get("hard_deadline"))
                            lease_remaining = self._remaining_seconds_timestamp(state_data.get("lease_deadline"))
                        elif hasattr(state_data, 'value'):
                            health_status = state_data.value
                            lease_remaining = None
                        else:
                            health_status = str(state_data)
                            lease_remaining = None

                        if health_status == wait_plan.target_status:
                            elapsed = self._loop.time() - start_time
                            logger.debug(f"[ASYNC_SHELL] [READY] Service {service_name} is ready")
                            return {
                                "success": True,
                                "status": health_status,
                                "window_metrics": window_metrics,
                                "retry_in": retry_in,
                                "hard_timeout_in": hard_timeout_in,
                                "lease_remaining": lease_remaining,
                            }

                except Exception as e:
                    logger.debug(f"[ASYNC_SHELL] [ERROR] Status check failed: {e}")

                elapsed = self._loop.time() - start_time
                if elapsed > wait_plan.timeout:
                    logger.warning(f"[ASYNC_SHELL] [TIMEOUT] Waiting for service {service_name} timed out ({elapsed:.1f}s)")
                    return {
                        "success": False,
                        "status": health_status,
                        "window_metrics": window_metrics,
                        "retry_in": retry_in,
                        "hard_timeout_in": hard_timeout_in,
                        "hard_timeout_remaining": max(wait_plan.timeout - elapsed, 0.0),
                        "lease_remaining": lease_remaining if 'lease_remaining' in locals() else None,
                    }

                await asyncio.sleep(wait_plan.check_interval)

        except Exception as e:
            logger.error(f"[ASYNC_SHELL] [ERROR] wait_service_async failed: {e}")
            return {"success": False, "error": str(e)}

    @staticmethod
    def _remaining_seconds_timestamp(ts: Optional[float]) -> Optional[float]:
        """从时间戳计算剩余秒数"""
        try:
            if ts is None:
                return None
            loop = asyncio.get_event_loop()
            now = loop.time()
            return max(ts - now, 0.0)
        except Exception:
            return None

    async def _start_services_async(self, service_names: list) -> None:
        """
        异步启动服务列表

        使用缓存层管理器直接从 pykv 获取服务配置
        """
        logger.info(f"[CONNECTION_START] [START] Starting service connection flow, service list: {service_names}")
        logger.info(f"[CONNECTION_START] [INFO] Orchestrator type: {type(self.orchestrator)}")

        if not self.orchestrator:
            logger.warning("[CONNECTION_START] [WARN] No orchestrator, skipping service startup")
            return

        logger.info(f"[CONNECTION_START] [INFO] Orchestrator exists, checking startup methods...")

        # 获取缓存层服务管理器
        service_manager = getattr(self.registry, '_cache_service_manager', None)
        if service_manager is None:
            raise RuntimeError(
                "Cache layer ServiceEntityManager is not initialized. "
                "Please ensure ServiceRegistry correctly initializes the _cache_service_manager attribute."
            )

        for service_name in service_names:
            try:
                logger.info(f"[CONNECTION_START] [TRY] Attempting to start service: {service_name}")

                # 检查orchestrator是否有连接方法
                if hasattr(self.orchestrator, 'connect_service'):
                    logger.info(f"[CONNECTION_START] [FOUND] Found connect_service method, connecting service...")

                    # 计算全局名称，并从缓存层直接获取服务配置
                    from ..cache.naming_service import NamingService
                    naming = NamingService()
                    global_name = naming.generate_service_global_name(service_name, self.core.agent_id or "global_agent_store")

                    service_config = {}
                    try:
                        # 使用缓存层 ServiceEntityManager 获取服务实体（全局名）
                        service_entity = await service_manager.get_service(global_name)

                        logger.info(f"[CONNECTION_START] [GET] Retrieved service_entity: {service_entity}")

                        if service_entity:
                            # ServiceEntity 对象有 config 属性
                            if hasattr(service_entity, 'config'):
                                inner_config = service_entity.config
                            elif hasattr(service_entity, 'get'):
                                inner_config = service_entity.get("config", {})
                            else:
                                inner_config = {}

                            if inner_config and ('url' in inner_config or 'command' in inner_config):
                                service_config = inner_config
                                logger.info(f"[CONNECTION_START] [CONFIG] Service configuration passed to orchestrator: {service_config}")
                            else:
                                raise RuntimeError(f"[CONNECTION_START] Service configuration is invalid or empty: {inner_config}")
                        else:
                            raise RuntimeError(f"[CONNECTION_START] service_entity is empty: global_name={global_name}")

                    except Exception as e:
                        logger.error(f"[CONNECTION_START] [ERROR] Failed to get service configuration: {e}")

                    # 检查是否有异步版本
                    connect_method = getattr(self.orchestrator, 'connect_service')
                    import inspect
                    if inspect.iscoroutinefunction(connect_method):
                        logger.info(f"[CONNECTION_START] [ASYNC] connect_service is async method, calling directly...")
                        success, message = await self.orchestrator.connect_service(service_name, service_config)
                        logger.info(f"[CONNECTION_START] [RESULT] Connection result: success={success}, message={message}")
                    else:
                        logger.info(f"[CONNECTION_START] [SYNC] connect_service is sync method, calling in thread...")
                        loop = asyncio.get_running_loop()
                        success, message = await loop.run_in_executor(None, lambda: self.orchestrator.connect_service(service_name, service_config))
                        logger.info(f"[CONNECTION_START] [RESULT] Connection result: success={success}, message={message}")

                    logger.info(f"[CONNECTION_START] [SENT] Service {service_name} connection command sent")
                elif hasattr(self.orchestrator, 'start_service'):
                    logger.warning(f"[CONNECTION_START] [WARN] Only sync method start_service available, may cause deadlock, skipping startup {service_name}")
                elif hasattr(self.orchestrator, 'start_service_async'):
                    logger.info(f"[CONNECTION_START] [FOUND] Found start_service_async method, starting service...")
                    await self.orchestrator.start_service_async(service_name)
                    logger.info(f"[CONNECTION_START] [SENT] Service {service_name} startup command sent")
                else:
                    logger.warning(f"[CONNECTION_START] [WARN] Orchestrator has no startup/connection methods, skipping {service_name}")
                    logger.info(f"[CONNECTION_START] [INFO] Orchestrator available methods: {[m for m in dir(self.orchestrator) if not m.startswith('_') and any(kw in m for kw in ['start', 'connect', 'service'])]}")

            except Exception as e:
                logger.error(f"[CONNECTION_START] [ERROR] Failed to start service {service_name}: {e}", exc_info=True)


class ServiceManagementSyncShell:
    """
    同步外壳：一次性同步转异步

    特点：
    - 通过 Async Orchestrated Bridge 在稳定事件循环中执行
    - 内部调用异步外壳，不再有任何同步/异步混用
    - 完全避免_sync_to_kv的使用
    """

    def __init__(self, async_shell: ServiceManagementAsyncShell):
        """
        初始化同步外壳

        Args:
            async_shell: 异步外壳实例
        """
        self.async_shell = async_shell
        self._bridge = get_async_bridge()
        self._loop = asyncio.get_event_loop()
        logger.debug("[SYNC_SHELL] [INIT] Initializing ServiceManagementSyncShell")

    def add_service(self, config: Dict[str, Any]) -> Dict[str, Any]:
        """
        同步外壳：添加服务

        通过 AOB 在后台事件循环中执行异步壳，避免每次创建新循环。
        """
        logger.debug("[SYNC_SHELL] [START] Starting synchronous service addition")

        try:
            result = self._bridge.run(
                self.async_shell.add_service_async(config),
                op_name="service_management.add_service",
            )

            logger.debug(f"[SYNC_SHELL] [COMPLETE] Synchronous service addition completed: {result.get('success', False)}")
            return result

        except Exception as e:
            logger.error(f"[SYNC_SHELL] [ERROR] Synchronous service addition failed: {e}")
            return {
                "success": False,
                "error": str(e),
                "added_services": [],
                "operations": [],
                "total_operations": 0,
                "successful_operations": 0,
            }

    def wait_service(self, service_name: str, timeout: float | None = None):
        """
        同步外壳：等待服务就绪

        通过 AOB 在后台事件循环中执行异步壳。
        """
        logger.debug(f"[SYNC_SHELL] [START] Starting synchronous service wait: {service_name}")

        try:
            result = self._bridge.run(
                self.async_shell.wait_service_async(service_name, timeout),
                op_name="service_management.wait_service",
            )

            logger.debug(f"[SYNC_SHELL] [COMPLETE] Synchronous service wait completed: {result}")
            return result

        except Exception as e:
            logger.error(f"[SYNC_SHELL] [ERROR] Synchronous service wait failed: {e}")
            return {"success": False, "error": str(e)}


class ServiceManagementFactory:
    """
    服务管理工厂类

    用于创建完整的服务管理实例（核心 + 外壳）
    """

    @staticmethod
    def create_service_management(registry, orchestrator, agent_id: str = "global_agent_store") -> tuple:
        """
        创建完整的服务管理实例

        Returns:
            tuple: (sync_shell, async_shell, core)
        """
        # 1. 创建纯同步核心
        core = ServiceManagementCore(agent_id=agent_id)

        # 2. 创建异步外壳
        async_shell = ServiceManagementAsyncShell(core, registry, orchestrator)

        # 3. 创建同步外壳
        sync_shell = ServiceManagementSyncShell(async_shell)

        logger.info("[FACTORY] [COMPLETE] Service management instance creation completed")

        return sync_shell, async_shell, core
