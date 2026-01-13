"""
MCPStore Service Query Module
服务查询相关功能实现，提供服务列表、详情查询、健康检查等核心功能
支持 Store 和 Agent 两种上下文模式，实现严格的服务隔离和透明代理
"""

import logging
from typing import Optional, List, Dict, Any

from mcpstore.core.models.service import ServiceInfo, ServiceConnectionState, TransportType, ServiceInfoResponse

logger = logging.getLogger(__name__)


class ServiceQueryMixin:
    """服务查询混入类，提供服务列表、详情查询、健康检查等功能"""
    
    def check_services(self, agent_id: Optional[str] = None) -> Dict[str, str]:
        """兼容性API，委托给上下文执行健康检查"""
        context = self.for_agent(agent_id) if agent_id else self.for_store()
        return context.check_services()

    def _infer_transport_type(self, service_config: Dict[str, Any]) -> TransportType:
        """Infer transport type of service"""
        if not service_config:
            return TransportType.STREAMABLE_HTTP

        # Prefer transport field first
        transport = service_config.get("transport")
        if transport:
            try:
                return TransportType(transport)
            except ValueError:
                pass
                
        # Then check based on url
        if service_config.get("url"):
            return TransportType.STREAMABLE_HTTP
            
        # Check based on command/args
        cmd = (service_config.get("command") or "").lower()
        args = " ".join(service_config.get("args", [])).lower()
        
        # Check if it's a Node.js package
        if "npx" in cmd or "node" in cmd or "npm" in cmd:
            return TransportType.STDIO
        
        # Check if it's a Python package
        if "python" in cmd or "pip" in cmd or ".py" in args:
            return TransportType.STDIO
            
        return TransportType.STREAMABLE_HTTP

    async def list_services(self, id: Optional[str] = None, agent_mode: bool = False) -> List[ServiceInfo]:
        """
        纯缓存模式的服务列表获取

         新特点：
        - 完全从缓存获取数据
        - 包含完整的 Agent-Client 信息
        - 高性能，无文件IO
        """
        services_info = []

        # 1. Store模式：直接从缓存层获取所有服务
        if not agent_mode and (not id or id == self.client_manager.global_agent_store_id):
            agent_id = self.client_manager.global_agent_store_id

            # 使用 _cache_layer_manager（CacheLayerManager）获取所有服务实体
            # 不再使用 _cache_layer，因为它在 Redis 模式下是 RedisStore，没有 get_all_entities_async 方法
            try:
                services = await self.registry._cache_layer_manager.get_all_entities_async("services")
                logger.debug(f"[QUERY] Retrieved service data: {services}")
            except Exception as e:
                logger.error(f"Failed to get services from cache: {e}")
                raise

            if not services:
                # 缓存为空，可能需要初始化
                logger.info("Cache is empty, you may need to add services first")
                return []

            for service_global_name, service_data in services.items():
                # 处理 ManagedEntry 对象
                if hasattr(service_data, 'value'):
                    actual_data = service_data.value
                    logger.debug(f"[QUERY] Extracting ManagedEntry.value: {actual_data}")
                else:
                    actual_data = service_data
                    logger.debug(f"[QUERY] Using data directly: {actual_data}")

                # 获取服务名称
                service_name = actual_data.get('service_original_name', service_global_name)
                logger.debug(f"[QUERY] Service name: {service_name}")

                # 从缓存获取完整信息 - 在异步上下文中调用异步版本
                logger.info(f"[QUERY] Getting complete service info: service_global_name={service_global_name}, service_name={service_name}")
                complete_info = await self.registry.get_complete_service_info_async(agent_id, service_global_name)

                logger.info(f"[QUERY] Got complete info: complete_info={complete_info}")

                # 安全检查，确保 complete_info 不为 None
                if complete_info is None:
                    logger.error(f"[QUERY] Service {service_global_name} complete info is NULL, using default values")
                    complete_info = {
                        "name": service_name,
                        "state": "disconnected",
                        "config": {},
                        "tool_count": 0,
                        "tools": []
                    }

                # 防御性编程：确保 config 不为 None
                if complete_info.get("config") is None:
                    complete_info["config"] = {}

                # 从 pykv 缓存层直接获取服务状态（唯一真相数据源）
                # 使用 cache/state_manager.py 的 get_service_status 方法
                cache_state_manager = getattr(self.registry, '_cache_state_manager', None)
                if cache_state_manager is not None:
                    status_data = await cache_state_manager.get_service_status(service_global_name)
                    if status_data is not None:
                        if hasattr(status_data, 'health_status'):
                            state = status_data.health_status
                        elif isinstance(status_data, dict):
                            state = status_data.get('health_status', 'disconnected')
                        else:
                            state = str(status_data)
                        logger.debug(f"[QUERY] Getting state from pykv: {service_global_name} -> {state}")
                    else:
                        state = complete_info.get("state") or "disconnected"
                        logger.debug(f"[QUERY] No state in pykv, using default value: {state}")
                else:
                    state = complete_info.get("state") or "disconnected"
                    logger.warning(f"[QUERY] Cache layer state manager unavailable, using state from complete_info: {state}")

                # 确保状态是ServiceConnectionState枚举
                if isinstance(state, str):
                    try:
                        state = ServiceConnectionState(state)
                    except ValueError:
                        state = ServiceConnectionState.DISCONNECTED

                # 读取时按需触发异步健康检查（非阻塞）
                try:
                    health_monitor = getattr(self, "container", None).health_monitor if getattr(self, "container", None) else None
                    if health_monitor:
                        await health_monitor.maybe_schedule_health_check(agent_id, service_global_name, current_state=state)
                except Exception as e:
                    logger.debug(f"[QUERY] schedule health check failed: {e}")

                service_info = ServiceInfo(
                    url=complete_info.get("config", {}).get("url", ""),
                    name=service_name,
                    transport_type=self._infer_transport_type(complete_info.get("config", {})),
                    status=state,
                    tool_count=complete_info.get("tool_count", 0),
                    keep_alive=complete_info.get("config", {}).get("keep_alive", False),
                    working_dir=complete_info.get("config", {}).get("working_dir"),
                    env=complete_info.get("config", {}).get("env"),
                    last_heartbeat=complete_info.get("last_heartbeat"),
                    command=complete_info.get("config", {}).get("command"),
                    args=complete_info.get("config", {}).get("args"),
                    package_name=complete_info.get("config", {}).get("package_name"),
                    state_metadata=complete_info.get("state_metadata"),
                    last_state_change=complete_info.get("state_entered_time"),
                    client_id=complete_info.get("client_id"),  #  新增：Client ID 信息
                    config=complete_info.get("config", {})  #  [REFACTOR] 添加完整的config字段
                )
                services_info.append(service_info)

        # 2. Agent模式：作为“视图”，从 Store 命名空间派生服务列表
        elif agent_mode and id:
            try:
                agent_id = id
                global_agent_id = self.client_manager.global_agent_store_id

                # 通过映射获取该 Agent 的全局服务名集合
                global_service_names = self.registry.get_agent_services(agent_id)
                if not global_service_names:
                    logger.debug(f"[STORE.LIST_SERVICES] Agent {agent_id} has no mapped global services, returning empty list")
                    return services_info

                for global_name in global_service_names:
                    # 解析出本地名（显示用）并校验归属
                    parsed = self.registry.get_agent_service_from_global_name(global_name)
                    if not parsed:
                        continue
                    mapped_agent, local_name = parsed
                    if mapped_agent != agent_id:
                        continue

                    # 从全局命名空间读取该服务的完整信息
                    complete_info = await self.registry.get_complete_service_info_async(global_agent_id, global_name)
                    if not complete_info:
                        logger.debug(f"[STORE.LIST_SERVICES] Service not found in global cache: {global_name}")
                        continue

                    # 状态枚举转换
                    state = complete_info.get("state", "disconnected")
                    if isinstance(state, str):
                        try:
                            state = ServiceConnectionState(state)
                        except ValueError:
                            state = ServiceConnectionState.DISCONNECTED

                    # 读取时按需触发异步健康检查（非阻塞）
                    try:
                        health_monitor = getattr(self, "container", None).health_monitor if getattr(self, "container", None) else None
                        if health_monitor:
                            await health_monitor.maybe_schedule_health_check(agent_id, global_name, current_state=state)
                    except Exception as e:
                        logger.debug(f"[STORE.LIST_SERVICES] schedule health check failed: {e}")

                    # 构建以本地名展示的 ServiceInfo（数据来源于全局）
                    cfg = complete_info.get("config", {})
                    service_info = ServiceInfo(
                        url=cfg.get("url", ""),
                        name=local_name or global_name,
                        transport_type=self._infer_transport_type(cfg),
                        status=state,
                        tool_count=complete_info.get("tool_count", 0),
                        keep_alive=cfg.get("keep_alive", False),
                        working_dir=cfg.get("working_dir"),
                        env=cfg.get("env"),
                        last_heartbeat=complete_info.get("last_heartbeat"),
                        command=cfg.get("command"),
                        args=cfg.get("args"),
                        package_name=cfg.get("package_name"),
                        state_metadata=complete_info.get("state_metadata"),
                        last_state_change=complete_info.get("state_entered_time"),
                        # 透明代理：client_id 使用全局命名空间的client
                        client_id=complete_info.get("client_id"),
                        config=cfg
                    )
                    services_info.append(service_info)
            except Exception as e:
                logger.error(f"[STORE.LIST_SERVICES] Agent view derivation failed: {e}")
                return services_info

        return services_info

    async def get_service_info(self, name: str, agent_id: Optional[str] = None) -> ServiceInfoResponse:
        """
        获取服务详细信息（严格按上下文隔离）：
        - 未传 agent_id：仅在 global_agent_store 下所有 client_id 中查找服务
        - 传 agent_id：仅在该 agent_id 下所有 client_id 中查找服务

        优先级：按client_id顺序返回第一个匹配的服务
        """
        from mcpstore.core.store.client_manager import ClientManager
        client_manager: ClientManager = self.client_manager

        # 统一从 pykv 完整信息接口读取，避免内存或状态缺失导致的误报
        if not agent_id:
            effective_agent_id = self.client_manager.global_agent_store_id
            context_type = "store"
        else:
            effective_agent_id = agent_id
            context_type = f"agent({agent_id})"

        complete_info = await self.registry.get_complete_service_info_async(effective_agent_id, name)
        if not complete_info:
            return ServiceInfoResponse(
                success=False,
                message=f"Service '{name}' not found in {context_type} context",
                service=None,
                tools=[],
                connected=False
            )

        config = complete_info.get("config", {}) or {}
        state = complete_info.get("state")
        if isinstance(state, str):
            try:
                state = ServiceConnectionState(state)
            except ValueError:
                state = None
        tools_info = complete_info.get("tools") or []
        tool_count = len(tools_info)
        client_id = complete_info.get("client_id")
        local_name = complete_info.get("service_original_name") or name

        service_info = ServiceInfo(
            url=config.get("url", ""),
            name=local_name,
            transport_type=self._infer_transport_type(config),
            status=state or ServiceConnectionState.DISCONNECTED,
            tool_count=tool_count,
            keep_alive=config.get("keep_alive", False),
            working_dir=config.get("working_dir"),
            env=config.get("env"),
            last_heartbeat=complete_info.get("last_heartbeat"),
            command=config.get("command"),
            args=config.get("args"),
            package_name=config.get("package_name"),
            state_metadata=complete_info.get("state_metadata"),
            last_state_change=complete_info.get("state_entered_time"),
            client_id=client_id,
            config=config
        )

        connected = service_info.status in [
            ServiceConnectionState.READY,
            ServiceConnectionState.HEALTHY,
            ServiceConnectionState.DEGRADED,
        ]

        return ServiceInfoResponse(
            success=True,
            message=f"Service found in {context_type} context",
            service=service_info,
            tools=tools_info,
            connected=connected
        )

    async def get_health_status(self, id: Optional[str] = None, agent_mode: bool = False) -> Dict[str, Any]:
        # NOTE:
        # 统一采用“按 Agent 命名空间存储服务”的约定：
        # - store 视角：使用 global_agent_store 作为命名空间
        # - agent 视角：使用指定 agent_id 作为命名空间
        # client_id 仅用于标注归属与过滤，不作为生命周期与配置的读写命名空间
        """
        获取服务健康状态：
        - store未传id 或 id==global_agent_store：聚合 global_agent_store 下所有 client_id 的服务健康状态
        - store传普通 client_id：只查该 client_id 下的服务健康状态
        - agent级别：聚合 agent_id 下所有 client_id 的服务健康状态；如果 id 不是 agent_id，尝试作为 client_id 查
        """
        from mcpstore.core.store.client_manager import ClientManager
        client_manager: ClientManager = self.client_manager
        services = []
        # 1. store未传id 或 id==global_agent_store，聚合 global_agent_store 下所有 client_id 的服务健康状态
        if not agent_mode and (not id or id == self.client_manager.global_agent_store_id):
            agent_ns = self.client_manager.global_agent_store_id
            # [pykv 唯一真相源] 在 async 上下文中必须使用 async 方法从 pykv 读取
            # 修复：将同步调用改为异步调用，避免在 FastAPI 事件循环中触发 AOB 冲突
            service_names = await self.registry._service_state_service.get_all_service_names_async(agent_ns)
            for name in service_names:
                config = self.config.get_service_config(name) or {}
                # 生命周期与元数据：按 Agent 命名空间读取（使用异步版本）
                service_state = await self.registry._service_state_service.get_service_state_async(agent_ns, name)
                state_metadata = await self.registry._service_state_service.get_service_metadata_async(agent_ns, name)
                # 标注该服务当前映射到哪个 client_id（使用异步版本）
                client_id = await self.registry._agent_client_service.get_service_client_id_async(agent_ns, name)

                def _remaining_seconds(dt):
                    import datetime
                    if dt is None:
                        return None
                    if isinstance(dt, (int, float)):
                        return max(dt - time.time(), 0.0)
                    if isinstance(dt, datetime.datetime):
                        return max((dt - datetime.datetime.now()).total_seconds(), 0.0)
                    return None

                def _remaining_seconds(dt):
                    import datetime
                    if dt is None:
                        return None
                    if isinstance(dt, (int, float)):
                        return max(dt - time.time(), 0.0)
                    if isinstance(dt, datetime.datetime):
                        return max((dt - datetime.datetime.now()).total_seconds(), 0.0)
                    return None

                service_status = {
                    "name": name,
                    "url": config.get("url", ""),
                    "transport_type": config.get("transport", ""),
                    "status": service_state.value if hasattr(service_state, "value") else str(service_state),
                    "command": config.get("command"),
                    "args": config.get("args"),
                    "package_name": config.get("package_name"),
                    "client_id": client_id,
                    # 生命周期元数据（新健康模型）
                    "response_time": getattr(state_metadata, "response_time", None) if state_metadata else None,
                    "consecutive_failures": getattr(state_metadata, "consecutive_failures", 0) if state_metadata else 0,
                    "last_state_change": (state_metadata.state_entered_time.isoformat() if state_metadata and state_metadata.state_entered_time else None),
                    "window_error_rate": getattr(state_metadata, "window_error_rate", None) if state_metadata else None,
                    "latency_p95": getattr(state_metadata, "latency_p95", None) if state_metadata else None,
                    "latency_p99": getattr(state_metadata, "latency_p99", None) if state_metadata else None,
                    "sample_size": getattr(state_metadata, "sample_size", None) if state_metadata else None,
                    "next_retry_time": (state_metadata.next_retry_time.isoformat() if state_metadata and state_metadata.next_retry_time else None),
                    "retry_in": _remaining_seconds(getattr(state_metadata, "next_retry_time", None)) if state_metadata else None,
                    "hard_timeout_in": _remaining_seconds(getattr(state_metadata, "hard_deadline", None)) if state_metadata else None,
                    "lease_remaining": _remaining_seconds(getattr(state_metadata, "lease_deadline", None)) if state_metadata else None,
                }
                services.append(service_status)
            return {
                "orchestrator_status": "running",
                "active_services": len(services),
                "services": services
            }
        # 2. store传普通 client_id，只查该 client_id 下的服务健康状态
        if not agent_mode and id:
            if id == self.client_manager.global_agent_store_id:
                return {
                    "orchestrator_status": "running",
                    "active_services": 0,
                    "services": []
                }
            # 仅返回当前 client_id 映射到的服务（仍按 Agent 命名空间读状态）
            # [pykv 唯一真相源] 使用异步方法从 pykv 读取
            agent_ns = self.client_manager.global_agent_store_id
            all_names = await self.registry._service_state_service.get_all_service_names_async(agent_ns)
            for name in all_names:
                mapped = await self.registry._agent_client_service.get_service_client_id_async(agent_ns, name)
                if mapped != id:
                    continue
                config = self.config.get_service_config(name) or {}
                service_state = await self.registry._service_state_service.get_service_state_async(agent_ns, name)
                state_metadata = await self.registry._service_state_service.get_service_metadata_async(agent_ns, name)
                service_status = {
                    "name": name,
                    "url": config.get("url", ""),
                    "transport_type": config.get("transport", ""),
                    "status": service_state.value if hasattr(service_state, "value") else str(service_state),
                    "command": config.get("command"),
                    "args": config.get("args"),
                    "package_name": config.get("package_name"),
                    "client_id": mapped,
                    "response_time": getattr(state_metadata, "response_time", None) if state_metadata else None,
                    "consecutive_failures": getattr(state_metadata, "consecutive_failures", 0) if state_metadata else 0,
                    "last_state_change": (state_metadata.state_entered_time.isoformat() if state_metadata and state_metadata.state_entered_time else None),
                    "window_error_rate": getattr(state_metadata, "window_error_rate", None) if state_metadata else None,
                    "latency_p95": getattr(state_metadata, "latency_p95", None) if state_metadata else None,
                    "latency_p99": getattr(state_metadata, "latency_p99", None) if state_metadata else None,
                    "sample_size": getattr(state_metadata, "sample_size", None) if state_metadata else None,
                    "next_retry_time": (state_metadata.next_retry_time.isoformat() if state_metadata and state_metadata.next_retry_time else None),
                    "retry_in": _remaining_seconds(getattr(state_metadata, "next_retry_time", None)) if state_metadata else None,
                    "hard_timeout_in": _remaining_seconds(getattr(state_metadata, "hard_deadline", None)) if state_metadata else None,
                    "lease_remaining": _remaining_seconds(getattr(state_metadata, "lease_deadline", None)) if state_metadata else None,
                }
                services.append(service_status)
            return {
                "orchestrator_status": "running",
                "active_services": len(services),
                "services": services
            }
        # 3. agent级别，聚合 agent_id 下所有 client_id 的服务健康状态；如果 id 不是 agent_id，尝试作为 client_id 查
        if agent_mode and id:
            # [pykv 唯一真相源] 从关系层获取
            agent_services_for_id = await self.registry._relation_manager.get_agent_services(id)
            client_ids = list(set(svc.get("client_id") for svc in agent_services_for_id if svc.get("client_id")))
            if client_ids:
                agent_ns = id
                # [pykv 唯一真相源] 使用异步方法从 pykv 读取
                names = await self.registry._service_state_service.get_all_service_names_async(agent_ns)
                for name in names:
                    config = self.config.get_service_config(name) or {}
                    service_state = await self.registry._service_state_service.get_service_state_async(agent_ns, name)
                    state_metadata = await self.registry._service_state_service.get_service_metadata_async(agent_ns, name)
                    mapped_client = await self.registry._agent_client_service.get_service_client_id_async(agent_ns, name)
                    if mapped_client not in (client_ids or []):
                        continue
                    service_status = {
                        "name": name,
                        "url": config.get("url", ""),
                        "transport_type": config.get("transport", ""),
                    "status": service_state.value if hasattr(service_state, "value") else str(service_state),
                    "command": config.get("command"),
                    "args": config.get("args"),
                    "package_name": config.get("package_name"),
                    "client_id": mapped_client,
                    "response_time": getattr(state_metadata, "response_time", None) if state_metadata else None,
                    "consecutive_failures": getattr(state_metadata, "consecutive_failures", 0) if state_metadata else 0,
                    "last_state_change": (state_metadata.state_entered_time.isoformat() if state_metadata and state_metadata.state_entered_time else None),
                    "window_error_rate": getattr(state_metadata, "window_error_rate", None) if state_metadata else None,
                    "latency_p95": getattr(state_metadata, "latency_p95", None) if state_metadata else None,
                    "latency_p99": getattr(state_metadata, "latency_p99", None) if state_metadata else None,
                    "sample_size": getattr(state_metadata, "sample_size", None) if state_metadata else None,
                    "next_retry_time": (state_metadata.next_retry_time.isoformat() if state_metadata and state_metadata.next_retry_time else None),
                }
                services.append(service_status)
                return {
                    "orchestrator_status": "running",
                    "active_services": len(services),
                    "services": services
                }
            else:
                # id 不是 agent_id，则视为 client_id：过滤 agent 命名空间下映射到该 client 的服务
                # [pykv 唯一真相源] 使用异步方法从 pykv 读取
                agent_ns = self.client_manager.global_agent_store_id
                names = await self.registry._service_state_service.get_all_service_names_async(agent_ns)
                for name in names:
                    mapped_client = await self.registry._agent_client_service.get_service_client_id_async(agent_ns, name)
                    if mapped_client != id:
                        continue
                    config = self.config.get_service_config(name) or {}
                    service_state = await self.registry._service_state_service.get_service_state_async(agent_ns, name)
                    state_metadata = await self.registry._service_state_service.get_service_metadata_async(agent_ns, name)
                    service_status = {
                        "name": name,
                        "url": config.get("url", ""),
                        "transport_type": config.get("transport", ""),
                        "status": service_state.value if hasattr(service_state, "value") else str(service_state),
                        "command": config.get("command"),
                        "args": config.get("args"),
                        "package_name": config.get("package_name"),
                        "client_id": mapped_client,
                        "response_time": getattr(state_metadata, "response_time", None) if state_metadata else None,
                        "consecutive_failures": getattr(state_metadata, "consecutive_failures", 0) if state_metadata else 0,
                        "last_state_change": (state_metadata.state_entered_time.isoformat() if state_metadata and state_metadata.state_entered_time else None)
                    }
                    services.append(service_status)
                return {
                    "orchestrator_status": "running",
                    "active_services": len(services),
                    "services": services
                }
        return {
            "orchestrator_status": "running",
            "active_services": 0,
            "services": []
        }
