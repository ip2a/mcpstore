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
            logger.info("🔄 [INIT_CACHE] 开始从持久化文件初始化缓存...")

            # 1. 从 ClientManager 同步基础数据
            logger.info("🔄 [INIT_CACHE] 步骤1: 从ClientManager同步基础数据...")
            self.cache_manager.sync_from_client_manager(self.client_manager)
            logger.info("✅ [INIT_CACHE] 步骤1完成: ClientManager数据同步完成")

            # 2. 从 mcp.json 解析所有服务（包括 Agent 服务）
            import os
            config_path = getattr(self.config, 'config_path', None) or getattr(self.config, 'json_path', None)
            if config_path and os.path.exists(config_path):
                await self._initialize_services_from_mcp_config()

            # 3. 标记缓存已初始化
            from datetime import datetime
            self.registry.cache_sync_status["initialized"] = datetime.now()

            logger.info("✅ Cache initialization completed")

        except Exception as e:
            logger.error(f"❌ Cache initialization failed: {e}")
            raise

    async def _initialize_services_from_mcp_config(self):
        """
        从 mcp.json 初始化服务，解析 Agent 服务并建立映射关系
        """
        try:
            logger.info("🔄 [INIT_MCP] 开始从 mcp.json 解析服务...")

            # 读取 mcp.json 配置
            mcp_config = self.config.load_config()
            mcp_servers = mcp_config.get("mcpServers", {})

            if not mcp_servers:
                logger.info("🔄 [INIT_MCP] mcp.json 中没有服务配置")
                return

            logger.info(f"🔄 [INIT_MCP] 发现 {len(mcp_servers)} 个服务配置")

            # 解析服务并建立映射关系
            agents_discovered = set()
            global_agent_store_id = self.client_manager.global_agent_store_id

            for service_name, service_config in mcp_servers.items():
                try:
                    # 检查是否为 Agent 服务（包含 agent_id 字段）
                    agent_id = service_config.get("agent_id")
                    
                    if agent_id and agent_id != global_agent_store_id:
                        # Agent 服务：建立映射关系
                        logger.debug(f"🔄 [INIT_MCP] 发现 Agent 服务: {service_name} -> Agent {agent_id}")
                        
                        # 添加到发现的 Agent 集合
                        agents_discovered.add(agent_id)
                        
                        # 建立服务映射关系（Agent 服务名 -> 全局服务名）
                        agent_service_name = f"{service_name}_byagent_{agent_id}"
                        self.registry.add_agent_service_mapping(agent_id, agent_service_name, service_name)
                        
                        # 为 Agent 创建 Client 配置
                        client_id = f"client_{agent_id}_{service_name}_{hash(str(service_config)) % 10000}"
                        client_config = {"mcpServers": {service_name: service_config}}
                        
                        # 保存 Client 配置到缓存
                        self.registry.client_configs[client_id] = client_config
                        
                        # 建立 Agent -> Client 映射
                        self.registry.add_agent_client_mapping(agent_id, client_id)
                        
                        # 建立服务 -> Client 映射
                        if agent_id not in self.registry.service_to_client:
                            self.registry.service_to_client[agent_id] = {}
                        self.registry.service_to_client[agent_id][agent_service_name] = client_id
                        
                        logger.debug(f"✅ [INIT_MCP] Agent 服务映射完成: {agent_service_name} -> {client_id}")
                    
                    else:
                        # Store 服务：添加到 global_agent_store
                        logger.debug(f"🔄 [INIT_MCP] 发现 Store 服务: {service_name}")
                        
                        # 为 Store 服务创建 Client 配置
                        client_id = f"client_store_{service_name}_{hash(str(service_config)) % 10000}"
                        client_config = {"mcpServers": {service_name: service_config}}
                        
                        # 保存 Client 配置到缓存
                        self.registry.client_configs[client_id] = client_config
                        
                        # 建立 global_agent_store -> Client 映射
                        self.registry.add_agent_client_mapping(global_agent_store_id, client_id)
                        
                        # 建立服务 -> Client 映射
                        if global_agent_store_id not in self.registry.service_to_client:
                            self.registry.service_to_client[global_agent_store_id] = {}
                        self.registry.service_to_client[global_agent_store_id][service_name] = client_id
                        
                        logger.debug(f"✅ [INIT_MCP] Store 服务映射完成: {service_name} -> {client_id}")

                except Exception as e:
                    logger.error(f"❌ [INIT_MCP] 处理服务 {service_name} 失败: {e}")
                    continue

            # 同步发现的 Agent 到持久化文件
            if agents_discovered:
                logger.info(f"🔄 [INIT_MCP] 发现 {len(agents_discovered)} 个 Agent，开始同步到文件...")
                await self._sync_discovered_agents_to_files(agents_discovered)

            logger.info(f"✅ [INIT_MCP] mcp.json 解析完成，处理了 {len(mcp_servers)} 个服务")

        except Exception as e:
            logger.error(f"❌ [INIT_MCP] 从 mcp.json 初始化服务失败: {e}")
            raise
