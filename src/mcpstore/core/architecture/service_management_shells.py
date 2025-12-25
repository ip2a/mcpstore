"""
Service Management Shells - 双路外壳实现

异步外壳和同步外壳的完整实现，严格遵循Functional Core, Imperative Shell架构原则。
"""

import asyncio
import logging
from typing import Dict, Any, Optional
from .service_management_core import ServiceManagementCore, ServiceOperationPlan, WaitOperationPlan, ServiceOperation

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
        logger.debug("[ASYNC_SHELL] 初始化 ServiceManagementAsyncShell")

    async def add_service_async(self, config: Dict[str, Any]) -> Dict[str, Any]:
        """
        异步外壳：执行服务添加

        严格按照新架构原则：
        1. 调用纯同步核心（无IO，无锁，无异步）
        2. 纯异步执行所有pykv操作
        3. 避免任何_sync_to_kv调用

        修复：使用正确的缓存层管理器方法
        """
        logger.debug("[ASYNC_SHELL] 开始添加服务")

        try:
            # 1. 调用纯同步核心（这是唯一可能调用同步逻辑的地方）
            operation_plan = self.core.add_service(config)
            logger.debug(f"[ASYNC_SHELL] 获得操作计划: {len(operation_plan.operations)}个操作")

            # 2. 获取正确的缓存层管理器
            # 优先使用 cache/ 目录下的管理器（直接操作 pykv）
            # 这些管理器是数据的唯一真相源
            service_manager = getattr(self.registry, '_cache_service_manager', None)
            relation_manager = getattr(self.registry, '_relation_manager', None)
            state_manager = getattr(self.registry, '_cache_state_manager', None)

            # 如果缓存层管理器不存在，抛出错误（不做降级处理）
            if service_manager is None:
                raise RuntimeError(
                    "缓存层 ServiceEntityManager 未初始化。"
                    "请确保 ServiceRegistry 正确初始化了 _cache_service_manager 属性。"
                )

            # 3. 纯异步执行所有操作
            results = []
            successful_operations = []

            for i, operation in enumerate(operation_plan.operations):
                logger.debug(f"[ASYNC_SHELL] 执行操作 {i+1}/{len(operation_plan.operations)}: {operation.type}")

                try:
                    if operation.type == "put_entity":
                        # 使用 cache/ServiceEntityManager 创建服务实体
                        await service_manager.create_service(
                            agent_id=operation.data.get("agent_id", "global_agent_store"),
                            original_name=operation.data.get("original_name", operation.data["key"]),
                            config=operation.data.get("config", operation.data.get("value", {}))
                        )
                        logger.debug(f"[ASYNC_SHELL] create_service 成功，key={operation.data['key']}")
                        successful_operations.append(operation)
                        results.append({"operation": operation.key, "status": "success"})

                    elif operation.type == "put_relation":
                        # 使用 cache/RelationshipManager 创建关系
                        if relation_manager is None:
                            raise RuntimeError(
                                "缓存层 RelationshipManager 未初始化。"
                                "请确保 ServiceRegistry 正确初始化了 _relation_manager 属性。"
                            )
                        await relation_manager.add_agent_service(
                            agent_id=operation.data.get("agent_id", "global_agent_store"),
                            service_original_name=operation.data.get("service_original_name", ""),
                            service_global_name=operation.data.get("service_global_name", operation.data["key"]),
                            client_id=operation.data.get("client_id", f"client_{operation.data['key']}")
                        )
                        logger.debug(f"[ASYNC_SHELL] add_agent_service 成功，key={operation.data['key']}")
                        successful_operations.append(operation)
                        results.append({"operation": operation.key, "status": "success"})

                    elif operation.type == "update_state":
                        # 使用 cache/StateManager 更新状态
                        if state_manager is None:
                            raise RuntimeError(
                                "缓存层 StateManager 未初始化。"
                                "请确保 ServiceRegistry 正确初始化了 _cache_state_manager 属性。"
                            )
                        await state_manager.update_service_status(
                            service_global_name=operation.data["key"],
                            health_status=operation.data.get("health_status", "initializing"),
                            tools_status=operation.data.get("tools_status", [])
                        )
                        logger.debug(f"[ASYNC_SHELL] update_state 成功，key={operation.data['key']}")
                        successful_operations.append(operation)
                        results.append({"operation": operation.key, "status": "success"})

                    elif operation.type == "put_metadata":
                        cache_layer = getattr(self.registry, "_cache_layer_manager", None)
                        if cache_layer is None:
                            raise RuntimeError("缓存层 CacheLayerManager 未初始化。")
                        await cache_layer.put_state(
                            "service_metadata",
                            operation.data["key"],
                            operation.data.get("value", {})
                        )
                        logger.debug(f"[ASYNC_SHELL] put_metadata 成功，key={operation.data['key']}")
                        successful_operations.append(operation)
                        results.append({"operation": operation.key, "status": "success"})

                    else:
                        raise ValueError(f"未知操作类型: {operation.type}")

                except Exception as e:
                    logger.error(f"[ASYNC_SHELL] 操作失败 {operation.key}: {e}")
                    results.append({"operation": operation.key, "status": "failed", "error": str(e)})
                    # 按要求抛出错误，不做静默处理
                    raise

            # 服务的实际连接交由事件驱动流程（ServiceAddRequested → ServiceCached → ServiceInitialized → ConnectionManager）
            logger.info(f"[ASYNC_SHELL] 服务添加完成: {len(operation_plan.service_names)}个服务, {len([r for r in results if r['status'] == 'success'])}个成功")

            return {
                "success": True,
                "added_services": operation_plan.service_names,
                "operations": results,
                "total_operations": len(operation_plan.operations),
                "successful_operations": len([r for r in results if r['status'] == 'success'])
            }

        except Exception as e:
            logger.error(f"[ASYNC_SHELL] add_service_async 失败: {e}")
            return {
                "success": False,
                "error": str(e),
                "added_services": [],
                "operations": [],
                "total_operations": 0,
                "successful_operations": 0
            }

    async def wait_service_async(self, service_name: str, timeout: float = 10.0) -> bool:
        """
        异步外壳：等待服务就绪

        严格按照新架构原则：
        1. 调用纯同步核心生成等待计划
        2. 纯异步执行状态检查循环
        3. 直接从pykv读取状态，使用缓存层管理器
        """
        logger.debug(f"[ASYNC_SHELL] 开始等待服务: {service_name}, timeout={timeout}")

        try:
            # 1. 调用纯同步核心
            wait_plan = self.core.wait_service_plan(service_name, timeout)
            logger.debug(f"[ASYNC_SHELL] 等待计划: {wait_plan}")

            # 2. 获取缓存层状态管理器
            # cache/state_manager.py 的方法签名是 get_service_status(service_global_name)
            state_manager = getattr(self.registry, '_cache_state_manager', None)
            if state_manager is None:
                raise RuntimeError(
                    "缓存层 StateManager 未初始化。"
                    "请确保 ServiceRegistry 正确初始化了 _cache_state_manager 属性。"
                )

            # 3. 纯异步等待检查
            start_time = asyncio.get_event_loop().time()

            while True:
                try:
                    # 使用 cache/state_manager.py: get_service_status(service_global_name)
                    # 注意：方法签名是 (service_global_name)，不是 (agent_id, service_name)
                    state_data = await state_manager.get_service_status(wait_plan.global_name)

                    # 处理 ServiceStatus 对象
                    if state_data:
                        if hasattr(state_data, 'health_status'):
                            # ServiceStatus 对象
                            health_status = state_data.health_status
                        elif hasattr(state_data, 'get'):
                            # 字典对象
                            health_status = state_data.get("health_status")
                        elif hasattr(state_data, 'value'):
                            # ServiceConnectionState 枚举
                            health_status = state_data.value
                        else:
                            # 其他类型，转换为字符串
                            health_status = str(state_data)

                        if health_status == wait_plan.target_status:
                            logger.debug(f"[ASYNC_SHELL] 服务 {service_name} 已就绪")
                            return True

                except Exception as e:
                    logger.debug(f"[ASYNC_SHELL] 状态检查失败: {e}")

                # 检查超时
                elapsed = asyncio.get_event_loop().time() - start_time
                if elapsed > wait_plan.timeout:
                    logger.warning(f"[ASYNC_SHELL] 等待服务 {service_name} 超时 ({elapsed:.1f}s)")
                    return False

                # 异步等待
                await asyncio.sleep(wait_plan.check_interval)

        except Exception as e:
            logger.error(f"[ASYNC_SHELL] wait_service_async 失败: {e}")
            return False

    async def _start_services_async(self, service_names: list) -> None:
        """
        异步启动服务列表

        使用缓存层管理器直接从 pykv 获取服务配置
        """
        logger.info(f"[CONNECTION_START] 开始启动服务流程，服务列表: {service_names}")
        logger.info(f"[CONNECTION_START] orchestrator类型: {type(self.orchestrator)}")

        if not self.orchestrator:
            logger.warning("[CONNECTION_START] 没有orchestrator，跳过服务启动")
            return

        logger.info(f"[CONNECTION_START] orchestrator存在，检查启动方法...")

        # 获取缓存层服务管理器
        service_manager = getattr(self.registry, '_cache_service_manager', None)
        if service_manager is None:
            raise RuntimeError(
                "缓存层 ServiceEntityManager 未初始化。"
                "请确保 ServiceRegistry 正确初始化了 _cache_service_manager 属性。"
            )

        for service_name in service_names:
            try:
                logger.info(f"[CONNECTION_START] 尝试启动服务: {service_name}")

                # 检查orchestrator是否有连接方法
                if hasattr(self.orchestrator, 'connect_service'):
                    logger.info(f"[CONNECTION_START] 找到 connect_service 方法，连接服务...")

                    # 计算全局名称，并从缓存层直接获取服务配置
                    from ..cache.naming_service import NamingService
                    naming = NamingService()
                    global_name = naming.generate_service_global_name(service_name, self.core.agent_id or "global_agent_store")

                    service_config = {}
                    try:
                        # 使用缓存层 ServiceEntityManager 获取服务实体（全局名）
                        service_entity = await service_manager.get_service(global_name)

                        logger.info(f"[CONNECTION_START] 获取的service_entity: {service_entity}")

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
                                logger.info(f"[CONNECTION_START] 传递给orchestrator的服务配置: {service_config}")
                            else:
                                raise RuntimeError(f"[CONNECTION_START] 服务配置无效或为空: {inner_config}")
                        else:
                            raise RuntimeError(f"[CONNECTION_START] service_entity 为空: global_name={global_name}")

                    except Exception as e:
                        logger.error(f"[CONNECTION_START] 获取服务配置失败: {e}")

                    # 检查是否有异步版本
                    connect_method = getattr(self.orchestrator, 'connect_service')
                    import inspect
                    if inspect.iscoroutinefunction(connect_method):
                        logger.info(f"[CONNECTION_START] connect_service 是异步方法，直接调用...")
                        success, message = await self.orchestrator.connect_service(service_name, service_config)
                        logger.info(f"[CONNECTION_START] 连接结果: success={success}, message={message}")
                    else:
                        logger.info(f"[CONNECTION_START] connect_service 是同步方法，在线程中调用...")
                        loop = asyncio.get_running_loop()
                        success, message = await loop.run_in_executor(None, lambda: self.orchestrator.connect_service(service_name, service_config))
                        logger.info(f"[CONNECTION_START] 连接结果: success={success}, message={message}")

                    logger.info(f"[CONNECTION_START] 服务 {service_name} 连接命令已发送")
                elif hasattr(self.orchestrator, 'start_service'):
                    logger.warning(f"[CONNECTION_START] 只有同步方法 start_service，可能导致死锁，跳过启动 {service_name}")
                elif hasattr(self.orchestrator, 'start_service_async'):
                    logger.info(f"[CONNECTION_START] 找到 start_service_async 方法，启动服务...")
                    await self.orchestrator.start_service_async(service_name)
                    logger.info(f"[CONNECTION_START] 服务 {service_name} 启动命令已发送")
                else:
                    logger.warning(f"[CONNECTION_START] orchestrator 没有任何启动/连接方法，跳过 {service_name}")
                    logger.info(f"[CONNECTION_START] orchestrator可用方法: {[m for m in dir(self.orchestrator) if not m.startswith('_') and any(kw in m for kw in ['start', 'connect', 'service'])]}")

            except Exception as e:
                logger.error(f"[CONNECTION_START] 启动服务 {service_name} 失败: {e}", exc_info=True)


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
        logger.debug("[SYNC_SHELL] 初始化 ServiceManagementSyncShell")

    def add_service(self, config: Dict[str, Any]) -> Dict[str, Any]:
        """
        同步外壳：添加服务

        通过 AOB 在后台事件循环中执行异步壳，避免每次创建新循环。
        """
        logger.debug("[SYNC_SHELL] 开始同步添加服务")

        try:
            result = self._bridge.run(
                self.async_shell.add_service_async(config),
                op_name="service_management.add_service",
            )

            logger.debug(f"[SYNC_SHELL] 同步添加服务完成: {result.get('success', False)}")
            return result

        except Exception as e:
            logger.error(f"[SYNC_SHELL] 同步添加服务失败: {e}")
            return {
                "success": False,
                "error": str(e),
                "added_services": [],
                "operations": [],
                "total_operations": 0,
                "successful_operations": 0,
            }

    def wait_service(self, service_name: str, timeout: float = 10.0) -> bool:
        """
        同步外壳：等待服务就绪

        通过 AOB 在后台事件循环中执行异步壳。
        """
        logger.debug(f"[SYNC_SHELL] 开始同步等待服务: {service_name}")

        try:
            result = self._bridge.run(
                self.async_shell.wait_service_async(service_name, timeout),
                op_name="service_management.wait_service",
            )

            logger.debug(f"[SYNC_SHELL] 同步等待服务完成: {result}")
            return result

        except Exception as e:
            logger.error(f"[SYNC_SHELL] 同步等待服务失败: {e}")
            return False


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

        logger.info("[FACTORY] 创建服务管理实例完成")

        return sync_shell, async_shell, core
