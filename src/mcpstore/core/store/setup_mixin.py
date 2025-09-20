"""
设置 Mixin 模块
负责处理 MCPStore 的实例级别初始化方法
"""

import logging

logger = logging.getLogger(__name__)


class SetupMixin:
    """设置 Mixin - 包含实例级别的初始化方法"""
    
    async def initialize_cache_from_files(self):
        """启动时从文件初始化缓存"""
        try:
            logger.info(" [INIT_CACHE] 开始从持久化文件初始化缓存...")

            # 单源模式：不再从 ClientManager 分片文件初始化
            logger.info(" [INIT_CACHE] 单源模式：跳过从分片文件初始化基础数据")

            # 2. 从 mcp.json 解析所有服务（包括 Agent 服务）
            import os
            config_path = getattr(self.config, 'config_path', None) or getattr(self.config, 'json_path', None)
            if config_path and os.path.exists(config_path):
                await self._initialize_services_from_mcp_config()

            # 3. 标记缓存已初始化
            from datetime import datetime
            self.registry.cache_sync_status["initialized"] = datetime.now()

            logger.info(" Cache initialization completed")

        except Exception as e:
            logger.error(f"❌ Cache initialization failed: {e}")
            raise

    def _find_existing_client_id_for_agent_service(self, agent_id: str, service_name: str) -> str:
        """
        查找Agent服务是否已有对应的client_id

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            现有的client_id，如果不存在则返回None
        """
        try:
            # 检查service_to_client映射
            if agent_id in self.registry.service_to_client:
                # Agent 空间中Service-Client映射以本地名为键
                if service_name in self.registry.service_to_client[agent_id]:
                    existing_client_id = self.registry.service_to_client[agent_id][service_name]
                    logger.debug(f" [INIT_MCP] 找到现有Agent client_id: {service_name} -> {existing_client_id}")
                    return existing_client_id

            # 检查agent_clients中是否有匹配的client_id
            client_ids = self.registry.agent_clients.get(agent_id, [])
            for client_id in client_ids:
                # 优先解析确定性ID
                try:
                    from mcpstore.core.utils.id_generator import ClientIDGenerator
                    if ClientIDGenerator.is_deterministic_format(client_id):
                        parsed = ClientIDGenerator.parse_client_id(client_id)
                        if parsed.get("type") == "agent" \
                           and parsed.get("agent_id") == agent_id \
                           and parsed.get("service_name") == service_name:
                            logger.debug(f" [INIT_MCP] 通过解析确定性ID找到Agent client_id: {client_id}")
                            return client_id
                except Exception:
                    pass
                # 兼容旧格式：保留模式匹配
                if f"_{agent_id}_{service_name}_" in client_id:
                    logger.debug(f" [INIT_MCP] 通过旧格式匹配找到Agent client_id: {client_id}")
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
            # 检查service_to_client映射
            if agent_id in self.registry.service_to_client:
                if service_name in self.registry.service_to_client[agent_id]:
                    existing_client_id = self.registry.service_to_client[agent_id][service_name]
                    logger.debug(f" [INIT_MCP] 找到现有Store client_id: {service_name} -> {existing_client_id}")
                    return existing_client_id

            # 检查agent_clients中是否有匹配的client_id
            client_ids = self.registry.agent_clients.get(agent_id, [])
            for client_id in client_ids:
                # 统一的确定性ID格式匹配：优先尝试解析
                try:
                    from mcpstore.core.utils.id_generator import ClientIDGenerator
                    if ClientIDGenerator.is_deterministic_format(client_id):
                        parsed = ClientIDGenerator.parse_client_id(client_id)
                        if parsed.get("type") == "store" and parsed.get("service_name") == service_name:
                            logger.debug(f" [INIT_MCP] 通过解析确定性ID找到Store client_id: {client_id}")
                            return client_id
                except Exception:
                    pass
                # 兼容旧格式：保留模式匹配
                if f"client_store_{service_name}_" in client_id:
                    logger.debug(f" [INIT_MCP] 通过旧格式匹配找到Store client_id: {client_id}")
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
            logger.info(" [INIT_MCP] 开始从 mcp.json 解析服务...")

            # 读取 mcp.json 配置
            mcp_config = self.config.load_config()
            mcp_servers = mcp_config.get("mcpServers", {})

            if not mcp_servers:
                logger.info(" [INIT_MCP] mcp.json 中没有服务配置")
                return

            logger.info(f" [INIT_MCP] 发现 {len(mcp_servers)} 个服务配置")

            # 解析服务并建立映射关系
            agents_discovered = set()
            global_agent_store_id = self.client_manager.global_agent_store_id

            for service_name, service_config in mcp_servers.items():
                try:
                    # 通过名称后缀解析是否为 Agent 服务
                    from mcpstore.core.context.agent_service_mapper import AgentServiceMapper

                    if AgentServiceMapper.is_any_agent_service(service_name):
                        agent_id, local_name = AgentServiceMapper.parse_agent_service_name(service_name)
                        if agent_id == global_agent_store_id:
                            # 防御：不应把全局ID当作Agent服务
                            agent_id = None
                    else:
                        agent_id = None
                        local_name = None

                    if agent_id:
                        global_name = service_name     # 带后缀的全局名

                        logger.debug(f" [INIT_MCP] 发现 Agent 服务: {global_name} -> Agent {agent_id} (local: {local_name})")
                        # 添加到发现的 Agent 集合
                        agents_discovered.add(agent_id)

                        # 建立服务映射关系（Agent 本地名 -> 全局服务名）
                        self.registry.add_agent_service_mapping(agent_id, local_name, global_name)

                        #  修复：检查是否已存在该服务的client_id，避免重复生成（按本地名查找）
                        existing_client_id = self._find_existing_client_id_for_agent_service(agent_id, local_name)

                        if existing_client_id:
                            # 使用现有的client_id
                            client_id = existing_client_id
                            logger.debug(f" [INIT_MCP] 使用现有Agent client_id: {global_name} -> {client_id}")
                        else:
                            #  使用统一的ClientIDGenerator生成确定性client_id
                            from mcpstore.core.utils.id_generator import ClientIDGenerator
                            
                            client_id = ClientIDGenerator.generate_deterministic_id(
                                agent_id=agent_id,
                                service_name=local_name,
                                service_config=service_config,
                                global_agent_store_id=global_agent_store_id
                            )
                            logger.debug(f" [INIT_MCP] 生成新Agent client_id: {global_name} -> {client_id}")

                        client_config = {"mcpServers": {local_name: service_config}}

                        # 保存 Client 配置到缓存
                        self.registry.client_configs[client_id] = client_config

                        # 建立 Agent -> Client 映射
                        self.registry.add_agent_client_mapping(agent_id, client_id)

                        # 建立服务 -> Client 映射
                        if agent_id not in self.registry.service_to_client:
                            self.registry.service_to_client[agent_id] = {}
                        # Agent 空间的服务键应使用本地名
                        self.registry.service_to_client[agent_id][local_name] = client_id

                        logger.debug(f" [INIT_MCP] Agent 服务映射完成: {agent_id}:{local_name} -> {client_id}")
                    
                    else:
                        # Store 服务：添加到 global_agent_store
                        logger.debug(f" [INIT_MCP] 发现 Store 服务: {service_name}")
                        
                        #  修复：检查是否已存在该服务的client_id，避免重复生成
                        existing_client_id = self._find_existing_client_id_for_store_service(global_agent_store_id, service_name)

                        if existing_client_id:
                            # 使用现有的client_id
                            client_id = existing_client_id
                            logger.debug(f" [INIT_MCP] 使用现有Store client_id: {service_name} -> {client_id}")
                        else:
                            # 生成新的client_id（统一使用确定性算法）
                            from mcpstore.core.utils.id_generator import ClientIDGenerator
                            client_id = ClientIDGenerator.generate_deterministic_id(
                                agent_id=global_agent_store_id,
                                service_name=service_name,
                                service_config=service_config,
                                global_agent_store_id=global_agent_store_id
                            )
                            logger.debug(f" [INIT_MCP] 生成新Store client_id: {service_name} -> {client_id}")

                        client_config = {"mcpServers": {service_name: service_config}}

                        # 保存 Client 配置到缓存
                        self.registry.client_configs[client_id] = client_config

                        # 建立 global_agent_store -> Client 映射
                        self.registry.add_agent_client_mapping(global_agent_store_id, client_id)

                        # 建立服务 -> Client 映射
                        if global_agent_store_id not in self.registry.service_to_client:
                            self.registry.service_to_client[global_agent_store_id] = {}
                        self.registry.service_to_client[global_agent_store_id][service_name] = client_id

                        logger.debug(f" [INIT_MCP] Store 服务映射完成: {service_name} -> {client_id}")

                except Exception as e:
                    logger.error(f"❌ [INIT_MCP] 处理服务 {service_name} 失败: {e}")
                    continue

            # 同步发现的 Agent 到持久化文件
            if agents_discovered:
                logger.info(f" [INIT_MCP] 发现 {len(agents_discovered)} 个 Agent，开始同步到文件...")
                await self._sync_discovered_agents_to_files(agents_discovered)

            logger.info(f" [INIT_MCP] mcp.json 解析完成，处理了 {len(mcp_servers)} 个服务")

        except Exception as e:
            logger.error(f"❌ [INIT_MCP] 从 mcp.json 初始化服务失败: {e}")
            raise
