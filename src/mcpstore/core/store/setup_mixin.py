"""
Setup Mixin Module
Handles instance-level initialization methods for MCPStore
"""

import logging

logger = logging.getLogger(__name__)


class SetupMixin:
    """Setup Mixin - contains instance-level initialization methods"""
    
    async def initialize_cache_from_files(self):
        """Initialize cache from files on startup"""
        try:
            logger.info(" [INIT_CACHE] Starting cache initialization from persistent files...")

            # Single source mode: no longer initialize from ClientManager shard files
            logger.info(" [INIT_CACHE] Single source mode: skipping basic data initialization from shard files")

            # 2. Parse all services from mcp.json (including Agent services)
            import os
            config_path = getattr(self.config, 'config_path', None) or getattr(self.config, 'json_path', None)
            if config_path and os.path.exists(config_path):
                await self._initialize_services_from_mcp_config()

            # 3. Mark cache as initialized
            from datetime import datetime
            self.registry.cache_sync_status["initialized"] = datetime.now()

            logger.info(" Cache initialization completed")

        except Exception as e:
            logger.error(f" Cache initialization failed: {e}")
            raise

    def _find_existing_client_id_for_agent_service(self, agent_id: str, service_name: str) -> str:
        """
        Find if Agent service already has corresponding client_id

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            现有的client_id，如果不存在则返回None
        """
        try:
            # 检查service_to_client映射（统一通过Registry API）
            existing_client_id = self.registry._agent_client_service.get_service_client_id(agent_id, service_name)
            if existing_client_id:
                logger.debug(f"[INIT_MCP] [FOUND] Found existing Agent client_id: {service_name} -> {existing_client_id}")
                return existing_client_id

            # 检查agent_clients中是否有匹配的client_id（统一通过Registry API）
            # 注意：这是同步方法，需要使用 asyncio.run
            import asyncio
            try:
                loop = asyncio.get_running_loop()
                # 在异步上下文中，使用 run_coroutine_threadsafe
                import concurrent.futures
                with concurrent.futures.ThreadPoolExecutor() as executor:
                    future = executor.submit(lambda: asyncio.run(self.registry.get_agent_clients_async(agent_id)))
                    client_ids = future.result(timeout=10.0)
            except RuntimeError:
                client_ids = asyncio.run(self.registry.get_agent_clients_async(agent_id))
            for client_id in client_ids:
                # 优先解析确定性ID
                try:
                    from mcpstore.core.utils.id_generator import ClientIDGenerator
                    if ClientIDGenerator.is_deterministic_format(client_id):
                        parsed = ClientIDGenerator.parse_client_id(client_id)
                        if parsed.get("type") == "agent" \
                           and parsed.get("agent_id") == agent_id \
                           and parsed.get("service_name") == service_name:
                            logger.debug(f"[INIT_MCP] [FOUND] Found Agent client_id by parsing deterministic ID: {client_id}")
                            return client_id
                except Exception:
                    pass
                # 兼容旧格式：保留模式匹配
                if f"_{agent_id}_{service_name}_" in client_id:
                    logger.debug(f"[INIT_MCP] [FOUND] Found Agent client_id by old format matching: {client_id}")
                    return client_id

            return None

        except Exception as e:
            logger.error(f"Error finding existing Agent client_id for service {service_name}: {e}")
            return None

    def _find_existing_client_id_for_store_service(self, agent_id: str, service_name: str) -> str:
        """
        查找Store服务是否已有对应的client_id

        Args:
            agent_id: Agent ID (通常是global_agent_store)
            service_name: 服务名称

        Returns:
            现有的client_id，如果不存在则返回None
        """
        try:
            # 优先：通过 Registry 提供的映射API 获取
            existing_client_id = self.registry._agent_client_service.get_service_client_id(agent_id, service_name)
            if existing_client_id:
                logger.debug(f"[INIT_MCP] [FOUND] Found existing Store client_id: {service_name} -> {existing_client_id}")
                return existing_client_id

            # 其次：检查 agent 的所有 client_ids（通过 Registry API）
            # 注意：这是同步方法，需要使用 asyncio.run
            import asyncio
            try:
                loop = asyncio.get_running_loop()
                # 在异步上下文中，使用 run_coroutine_threadsafe
                import concurrent.futures
                with concurrent.futures.ThreadPoolExecutor() as executor:
                    future = executor.submit(lambda: asyncio.run(self.registry.get_agent_clients_async(agent_id)))
                    client_ids = future.result(timeout=10.0)
            except RuntimeError:
                client_ids = asyncio.run(self.registry.get_agent_clients_async(agent_id))
            for client_id in client_ids:
                # 统一的确定性ID格式匹配：优先尝试解析
                try:
                    from mcpstore.core.utils.id_generator import ClientIDGenerator
                    if ClientIDGenerator.is_deterministic_format(client_id):
                        parsed = ClientIDGenerator.parse_client_id(client_id)
                        if parsed.get("type") == "store" and parsed.get("service_name") == service_name:
                            logger.debug(f"[INIT_MCP] [FOUND] Found Store client_id by parsing deterministic ID: {client_id}")
                            return client_id
                except Exception:
                    pass
                # 兼容旧格式：保留模式匹配
                if f"client_store_{service_name}_" in client_id:
                    logger.debug(f"[INIT_MCP] [FOUND] Found Store client_id by old format matching: {client_id}")
                    return client_id

            return None

        except Exception as e:
            logger.error(f"Error finding existing Store client_id for service {service_name}: {e}")
            return None

    async def _initialize_services_from_mcp_config(self):
        """
        从 mcp.json 初始化服务，解析 Agent 服务并建立映射关系
        """
        try:
            logger.info("[INIT_MCP] [START] Starting to parse services from mcp.json...")

            # 读取 mcp.json 配置（优化：使用缓存）
            mcp_config = self._unified_config.get_mcp_config()
            mcp_servers = mcp_config.get("mcpServers", {})

            if not mcp_servers:
                logger.info("[INIT_MCP] [INFO] No service configuration in mcp.json")
                return

            logger.info(f"[INIT_MCP] [FOUND] Found {len(mcp_servers)} service configurations")

            # 解析服务并建立映射关系
            global_agent_store_id = self.client_manager.global_agent_store_id
            for service_name, service_config in mcp_servers.items():
                try:
                    # 通过名称后缀解析是否为 Agent 服务
                    from mcpstore.core.context.agent_service_mapper import AgentServiceMapper

                    if AgentServiceMapper.is_any_agent_service(service_name):
                        agent_id, local_name = AgentServiceMapper.parse_agent_service_name(service_name)
                        global_name = service_name
                    else:
                        agent_id = global_agent_store_id
                        local_name = service_name
                        global_name = service_name

                    logger.info(f"[INIT_MCP] [REPLAY] service={service_name} agent={agent_id} local={local_name}")

                    # 已存在则跳过
                    if await self.registry.has_service_async(agent_id, local_name):
                        logger.info(f"[INIT_MCP] [SKIP] Service already exists in cache: {agent_id}:{local_name}")
                        continue

                    # 发布 bootstrap 事件，构建缓存后后台连接
                    from mcpstore.core.events.service_events import ServiceBootstrapRequested
                    from mcpstore.core.utils.id_generator import ClientIDGenerator

                    client_id = ClientIDGenerator.generate_deterministic_id(
                        agent_id=agent_id,
                        service_name=local_name,
                        service_config=service_config,
                        global_agent_store_id=global_agent_store_id
                    )

                    event_bus = getattr(getattr(self, "container", None), "_event_bus", None) or getattr(self, "event_bus", None) or getattr(getattr(self, "orchestrator", None), "event_bus", None)
                    if not event_bus:
                        raise RuntimeError("EventBus is not available during setup bootstrap")

                    bootstrap_event = ServiceBootstrapRequested(
                        agent_id=agent_id,
                        service_name=local_name,
                        service_config=service_config,
                        client_id=client_id,
                        global_name=global_name,
                        origin_agent_id=agent_id,
                        origin_local_name=local_name,
                        source="bootstrap_mcpjson"
                    )
                    await event_bus.publish(bootstrap_event, wait=False)
                    logger.info(f"[INIT_MCP] [OK] Published bootstrap event for service: {service_name} -> agent={agent_id}")

                except Exception as e:
                    logger.error(f"[INIT_MCP] [ERROR] Failed to process service {service_name}: {e}")
                    continue

            logger.info(f"[INIT_MCP] [COMPLETE] mcp.json parsing completed, processed {len(mcp_servers)} services")

        except Exception as e:
            logger.error(f"[INIT_MCP] [ERROR] Failed to initialize services from mcp.json: {e}")
            raise
