"""
MCPStore Agent Statistics Module
Implementation of Agent statistics functionality
"""

import logging

from mcpstore.core.models.agent import AgentsSummary, AgentStatistics, AgentServiceSummary

logger = logging.getLogger(__name__)

class AgentStatisticsMixin:
    """Agent statistics mixin class"""
    
    def get_agents_summary(self) -> AgentsSummary:
        """
        Get summary information for all Agents (synchronous version)

        Returns:
            AgentsSummary: Agent summary information
        """
        return self._run_async_via_bridge(
            self.get_agents_summary_async(),
            op_name="agent_statistics.get_agents_summary"
        )

    async def get_agents_summary_async(self) -> AgentsSummary:
        """
        Get summary information for all Agents (asynchronous version)

        Returns:
            AgentsSummary: Agent summary information
        """
        try:
            #  [REFACTOR] Get all Agent IDs from Registry cache
            logger.info(" [AGENT_STATS] Starting to get Agent statistics...")
            all_agent_ids = await self._store.registry.get_all_agent_ids_async()
            logger.info(f" [AGENT_STATS] Agent IDs retrieved from Registry cache: {all_agent_ids}")

            # Statistical information
            total_agents = len(all_agent_ids)
            active_agents = 0
            total_services = 0
            total_tools = 0
            
            agent_details = []
            
            for agent_id in all_agent_ids:
                try:
                    # Get Agent statistics information
                    logger.info(f" [AGENT_STATS] Starting to get detailed statistics for Agent {agent_id}...")
                    agent_stats = await self._get_agent_statistics(agent_id)
                    logger.info(f" [AGENT_STATS] Agent {agent_id} statistics completed: {agent_stats.service_count} services, {agent_stats.tool_count} tools")
                    
                    if agent_stats.is_active:
                        active_agents += 1
                    
                    total_services += agent_stats.service_count
                    total_tools += agent_stats.tool_count
                    
                    agent_details.append(agent_stats)
                    
                except Exception as e:
                    logger.warning(f"Failed to get statistics for agent {agent_id}: {e}")
                    # 创建一个错误状态的统计信息
                    error_stats = AgentStatistics(
                        agent_id=agent_id,
                        service_count=0,
                        tool_count=0,
                        healthy_services=0,
                        unhealthy_services=0,
                        total_tool_executions=0,
                        is_active=False,
                        last_activity=None,
                        services=[]
                    )
                    agent_details.append(error_stats)
            
            #  [REFACTOR] 获取Store级别的统计信息
            store_services = await self._store.list_services()
            store_tools = await self._store.list_tools()

            return AgentsSummary(
                total_agents=total_agents,
                active_agents=active_agents,
                total_services=total_services,
                total_tools=total_tools,
                store_services=len(store_services),
                store_tools=len(store_tools),
                agents=agent_details
            )
            
        except Exception as e:
            logger.error(f"Failed to get agents summary: {e}")
            return AgentsSummary(
                total_agents=0,
                active_agents=0,
                total_services=0,
                total_tools=0,
                store_services=0,
                store_tools=0,
                agents=[]
            )

    async def _get_agent_statistics(self, agent_id: str) -> AgentStatistics:
        """
        获取单个Agent的详细统计信息
        
        Args:
            agent_id: Agent ID
            
        Returns:
            AgentStatistics: Agent统计信息
        """
        try:
            # 获取Agent的所有client - 从 pykv 获取
            logger.info(f" [AGENT_STATS] Getting all clients for Agent {agent_id}...")
            client_ids = await self._store.registry.get_agent_clients_async(agent_id)
            logger.info(f" [AGENT_STATS] Agent {agent_id} client list: {client_ids}")

            # 统计服务和工具
            services = []
            total_tools = 0
            is_active = False
            last_activity = None
            
            for client_id in client_ids:
                try:
                    # 获取client配置
                    client_config = self._store.orchestrator.client_manager.get_client_config(client_id)
                    if not client_config:
                        continue
                    
                    #  [REFACTOR] 简化逻辑：直接检查服务状态来判断client是否活跃
                    # 不再调用不存在的get_client_status方法
                    
                    # 统计服务（新架构：从 client 实体的 services 列表获取服务名称）
                    services = client_config.get("services", []) if isinstance(client_config, dict) else []
                    for service_name in services:
                        try:
                            #  [REFACTOR] 使用正确的Registry方法获取服务工具（异步）
                            service_tools = await self._store.registry.get_tools_for_service_async(agent_id, service_name)
                            tool_count = len(service_tools) if service_tools else 0
                            total_tools += tool_count

                            #  [REFACTOR] 使用正确的Registry方法获取服务状态（异步）
                            service_state = await self._store.registry.get_service_state_async(agent_id, service_name)

                            # 检查服务是否活跃（有工具且状态不是DISCONNECTED）
                            from mcpstore.core.models.service import ServiceConnectionState
                            if service_state not in [ServiceConnectionState.DISCONNECTED, ServiceConnectionState.DISCONNECTED]:
                                is_active = True

                            service_summary = AgentServiceSummary(
                                service_name=service_name,
                                service_type="local" if service_config.get("command") else "remote",
                                status=service_state,
                                tool_count=tool_count,
                                client_id=client_id
                            )
                            services.append(service_summary)
                            
                        except Exception as e:
                            logger.warning(f"Failed to get service {service_name} stats for agent {agent_id}: {e}")
                            # 添加错误状态的服务
                            from mcpstore.core.models.service import ServiceConnectionState
                            error_service = AgentServiceSummary(
                                service_name=service_name,
                                service_type="unknown",
                                status=ServiceConnectionState.DISCONNECTED,
                                tool_count=0,
                                client_id=client_id
                            )
                            services.append(error_service)
                            
                except Exception as e:
                    logger.warning(f"Failed to process client {client_id} for agent {agent_id}: {e}")
            
            # 统计健康和不健康的服务
            healthy_services = len([s for s in services if s.status in ["healthy", "degraded"]])
            unhealthy_services = len(services) - healthy_services

            return AgentStatistics(
                agent_id=agent_id,
                service_count=len(services),
                tool_count=total_tools,
                healthy_services=healthy_services,
                unhealthy_services=unhealthy_services,
                total_tool_executions=0,  # TODO: 实现工具执行统计
                is_active=is_active,
                last_activity=last_activity,
                services=services
            )
            
        except Exception as e:
            logger.error(f"Failed to get statistics for agent {agent_id}: {e}")
            return AgentStatistics(
                agent_id=agent_id,
                service_count=0,
                tool_count=0,
                healthy_services=0,
                unhealthy_services=0,
                total_tool_executions=0,
                is_active=False,
                last_activity=None,
                services=[]
            )
