
import logging
from datetime import datetime
from typing import Dict, Any, Optional, List, Set

from fastmcp import Client

logger = logging.getLogger(__name__)

class ServiceRegistry:
    """
    Service Registry - Core Value of MCPStore

    Core Design: Complete Agent-level Isolation

    This is MCPStore's unique value compared to FastMCP:
    - Each Agent has independent service space
    - Store level uses global_agent_store_id as special Agent
    - Completely isolated multi-tenant architecture

    Simplified design after refactoring:
    - Directly use FastMCP Client, remove complex session abstraction
    - Retain core value of Agent-level isolation
    - Simplify data structures, improve performance
    - Remove duplicate connection management, rely on FastMCP's connection lifecycle
    """
    
    def __init__(self):
        # === Core Data Structures: Agent-level Isolation ===
        
        # agent_id -> {service_name: FastMCP Client}
        self.agent_clients: Dict[str, Dict[str, Client]] = {}
        
        # agent_id -> {service_name: last_heartbeat_time}
        self.service_health: Dict[str, Dict[str, datetime]] = {}
        
        # agent_id -> {tool_name: tool_definition}
        self.tool_cache: Dict[str, Dict[str, Dict[str, Any]]] = {}
        
        # agent_id -> {tool_name: service_name} - Tool to service mapping
        self.tool_to_service_map: Dict[str, Dict[str, str]] = {}

        # Long-lived connection service markers - agent_id:service_name
        self.long_lived_connections: Set[str] = set()

        logger.info("ServiceRegistry initialized with Agent-level isolation")

    def clear_agent(self, agent_id: str):
        """
        Clear all registered services and tools for specified Agent
        Only affects this Agent, does not affect other Agents
        """
        self.agent_clients.pop(agent_id, None)
        self.service_health.pop(agent_id, None)
        self.tool_cache.pop(agent_id, None)
        self.tool_to_service_map.pop(agent_id, None)
        logger.info(f"Cleared all services for agent: {agent_id}")

    def add_service(self, agent_id: str, service_name: str, client: Client, tools: List[Dict[str, Any]]) -> List[str]:
        """
        为指定Agent添加服务
        
        Args:
            agent_id: Agent ID
            service_name: 服务名称
            client: FastMCP Client实例
            tools: 工具定义列表
            
        Returns:
            添加的工具名称列表
        """
        # 确保Agent存在
        if agent_id not in self.agent_clients:
            self.agent_clients[agent_id] = {}
            self.service_health[agent_id] = {}
            self.tool_cache[agent_id] = {}
            self.tool_to_service_map[agent_id] = {}
        
        # 添加服务客户端
        self.agent_clients[agent_id][service_name] = client
        
        # 更新健康状态
        self.service_health[agent_id][service_name] = datetime.now()
        
        # 添加工具
        added_tools = []
        for tool in tools:
            tool_name = tool.get('name')
            if tool_name:
                self.tool_cache[agent_id][tool_name] = tool
                self.tool_to_service_map[agent_id][tool_name] = service_name
                added_tools.append(tool_name)
        
        logger.info(f"Added service '{service_name}' with {len(added_tools)} tools for agent '{agent_id}'")
        return added_tools

    def remove_service(self, agent_id: str, service_name: str) -> bool:
        """
        移除指定Agent的服务
        
        Args:
            agent_id: Agent ID
            service_name: 服务名称
            
        Returns:
            是否成功移除
        """
        if agent_id not in self.agent_clients:
            return False
        
        # 移除服务客户端
        if service_name in self.agent_clients[agent_id]:
            del self.agent_clients[agent_id][service_name]
        
        # 移除健康状态
        if service_name in self.service_health[agent_id]:
            del self.service_health[agent_id][service_name]
        
        # 移除相关工具
        tools_to_remove = [
            tool_name for tool_name, svc_name in self.tool_to_service_map[agent_id].items()
            if svc_name == service_name
        ]
        
        for tool_name in tools_to_remove:
            self.tool_cache[agent_id].pop(tool_name, None)
            self.tool_to_service_map[agent_id].pop(tool_name, None)
        
        logger.info(f"Removed service '{service_name}' and {len(tools_to_remove)} tools for agent '{agent_id}'")
        return True

    def get_client(self, agent_id: str, service_name: str) -> Optional[Client]:
        """获取指定Agent的服务客户端"""
        return self.agent_clients.get(agent_id, {}).get(service_name)

    def get_client_for_tool(self, agent_id: str, tool_name: str) -> Optional[Client]:
        """获取指定Agent的工具对应的客户端"""
        service_name = self.tool_to_service_map.get(agent_id, {}).get(tool_name)
        if service_name:
            return self.get_client(agent_id, service_name)
        return None

    def get_tools(self, agent_id: str) -> Dict[str, Dict[str, Any]]:
        """获取指定Agent的所有工具"""
        return self.tool_cache.get(agent_id, {})

    def get_tool(self, agent_id: str, tool_name: str) -> Optional[Dict[str, Any]]:
        """获取指定Agent的特定工具"""
        return self.tool_cache.get(agent_id, {}).get(tool_name)

    def get_services(self, agent_id: str) -> List[str]:
        """获取指定Agent的所有服务名称"""
        return list(self.agent_clients.get(agent_id, {}).keys())

    def get_all_agents(self) -> List[str]:
        """获取所有Agent ID"""
        return list(self.agent_clients.keys())

    def update_service_health(self, agent_id: str, service_name: str, timestamp: datetime = None):
        """更新服务健康状态"""
        if timestamp is None:
            timestamp = datetime.now()
        
        if agent_id in self.service_health:
            self.service_health[agent_id][service_name] = timestamp

    def get_service_health(self, agent_id: str, service_name: str) -> Optional[datetime]:
        """获取服务健康状态"""
        return self.service_health.get(agent_id, {}).get(service_name)

    def is_service_healthy(self, agent_id: str, service_name: str, timeout_seconds: int = 300) -> bool:
        """检查服务是否健康（基于最后心跳时间）"""
        last_heartbeat = self.get_service_health(agent_id, service_name)
        if not last_heartbeat:
            return False
        
        time_diff = (datetime.now() - last_heartbeat).total_seconds()
        return time_diff <= timeout_seconds

    def get_unhealthy_services(self, agent_id: str, timeout_seconds: int = 300) -> List[str]:
        """获取指定Agent的不健康服务列表"""
        unhealthy = []
        for service_name in self.get_services(agent_id):
            if not self.is_service_healthy(agent_id, service_name, timeout_seconds):
                unhealthy.append(service_name)
        return unhealthy

    def get_stats(self, agent_id: str = None) -> Dict[str, Any]:
        """
        获取统计信息
        
        Args:
            agent_id: 如果指定，返回该Agent的统计；否则返回全局统计
            
        Returns:
            统计信息字典
        """
        if agent_id:
            # 单个Agent的统计
            services = self.get_services(agent_id)
            tools = self.get_tools(agent_id)
            healthy_services = [
                svc for svc in services 
                if self.is_service_healthy(agent_id, svc)
            ]
            
            return {
                "agent_id": agent_id,
                "services": {
                    "total": len(services),
                    "healthy": len(healthy_services),
                    "unhealthy": len(services) - len(healthy_services),
                    "names": services
                },
                "tools": {
                    "total": len(tools),
                    "names": list(tools.keys())
                }
            }
        else:
            # 全局统计
            all_agents = self.get_all_agents()
            total_services = sum(len(self.get_services(aid)) for aid in all_agents)
            total_tools = sum(len(self.get_tools(aid)) for aid in all_agents)
            
            return {
                "agents": {
                    "total": len(all_agents),
                    "ids": all_agents
                },
                "services": {
                    "total": total_services
                },
                "tools": {
                    "total": total_tools
                }
            }

    def mark_as_long_lived(self, agent_id: str, service_name: str):
        """标记服务为长连接服务"""
        service_key = f"{agent_id}:{service_name}"
        self.long_lived_connections.add(service_key)
        logger.debug(f"Marked service '{service_name}' as long-lived for agent '{agent_id}'")

    def is_long_lived_service(self, agent_id: str, service_name: str) -> bool:
        """检查服务是否为长连接服务"""
        service_key = f"{agent_id}:{service_name}"
        return service_key in self.long_lived_connections

    def get_long_lived_services(self, agent_id: str) -> List[str]:
        """获取指定Agent的所有长连接服务"""
        prefix = f"{agent_id}:"
        return [
            key[len(prefix):] for key in self.long_lived_connections
            if key.startswith(prefix)
        ]

    def should_cache_aggressively(self, agent_id: str, service_name: str) -> bool:
        """
        判断是否应该激进缓存
        长连接服务可以更激进地缓存，因为连接稳定
        """
        return self.is_long_lived_service(agent_id, service_name)

    def __repr__(self) -> str:
        stats = self.get_stats()
        return f"ServiceRegistry(agents={stats['agents']['total']}, services={stats['services']['total']}, tools={stats['tools']['total']})"
