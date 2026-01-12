import logging
from datetime import datetime
from typing import Dict, Any, List, Optional

from mcpstore.core.models.service import ServiceConnectionState

logger = logging.getLogger(__name__)


class SmartCacheQuery:
    """Smart cache query interface"""
    
    def __init__(self, registry):
        self.registry = registry
    
    def services(self, agent_id: str) -> 'ServiceQueryBuilder':
        """Create service query builder"""
        return ServiceQueryBuilder(self.registry, agent_id)
    
    def agents(self) -> 'AgentQueryBuilder':
        """Create Agent query builder"""
        return AgentQueryBuilder(self.registry)
    
    def clients(self, agent_id: str) -> 'ClientQueryBuilder':
        """Create Client query builder"""
        return ClientQueryBuilder(self.registry, agent_id)


class ServiceQueryBuilder:
    """Service query builder"""
    
    def __init__(self, registry, agent_id: str):
        self.registry = registry
        self.agent_id = agent_id
        self._filters = []
        self._sorts = []
        self._limit = None
    
    def healthy(self):
        """只查询健康的服务"""
        self._filters.append(('state', [ServiceConnectionState.HEALTHY, ServiceConnectionState.DEGRADED]))
        return self
    
    def failed(self):
        """只查询失败的服务"""
        self._filters.append(('state', [ServiceConnectionState.DISCONNECTED, ServiceConnectionState.DISCONNECTED]))
        return self
    
    def with_tools(self, min_count: int = 1):
        """查询有工具的服务"""
        self._filters.append(('tool_count', '>=', min_count))
        return self
    
    def name_like(self, pattern: str):
        """按名称模式查询"""
        self._filters.append(('name_pattern', pattern))
        return self
    
    def transport_type(self, transport: str):
        """按传输类型查询"""
        self._filters.append(('transport', transport))
        return self
    
    def sort_by_name(self, desc: bool = False):
        """按名称排序"""
        self._sorts.append(('name', desc))
        return self
    
    def sort_by_tool_count(self, desc: bool = True):
        """按工具数量排序"""
        self._sorts.append(('tool_count', desc))
        return self
    
    def sort_by_last_heartbeat(self, desc: bool = True):
        """按最后心跳时间排序"""
        self._sorts.append(('last_heartbeat', desc))
        return self
    
    def limit(self, count: int):
        """限制结果数量"""
        self._limit = count
        return self
    
    def execute(self) -> List[Dict[str, Any]]:
        """执行查询"""
        # 获取所有服务
        all_services = self.registry.get_all_services_complete_info(self.agent_id)
        
        # 应用过滤器
        filtered_services = []
        for service in all_services:
            if self._matches_filters(service):
                filtered_services.append(service)
        
        # 应用排序
        for sort_field, desc in reversed(self._sorts):
            filtered_services.sort(
                key=lambda s: self._get_sort_value(s, sort_field),
                reverse=desc
            )
        
        # 应用限制
        if self._limit:
            filtered_services = filtered_services[:self._limit]
        
        return filtered_services
    
    def count(self) -> int:
        """获取匹配的服务数量"""
        return len(self.execute())
    
    def first(self) -> Optional[Dict[str, Any]]:
        """获取第一个匹配的服务"""
        results = self.limit(1).execute()
        return results[0] if results else None
    
    def _matches_filters(self, service: Dict[str, Any]) -> bool:
        """检查服务是否匹配过滤条件"""
        for filter_type, *filter_args in self._filters:
            if filter_type == 'state':
                allowed_states = filter_args[0]
                service_state_str = service.get('state', 'unknown')
                # 将字符串状态转换为枚举进行比较
                try:
                    service_state = ServiceConnectionState(service_state_str)
                    if service_state not in allowed_states:
                        return False
                except ValueError:
                    return False
            elif filter_type == 'tool_count':
                operator, threshold = filter_args
                tool_count = service.get('tool_count', 0)
                if operator == '>=' and tool_count < threshold:
                    return False
                elif operator == '>' and tool_count <= threshold:
                    return False
                elif operator == '<=' and tool_count > threshold:
                    return False
                elif operator == '<' and tool_count >= threshold:
                    return False
                elif operator == '==' and tool_count != threshold:
                    return False
            elif filter_type == 'name_pattern':
                pattern = filter_args[0]
                if pattern.lower() not in service.get('name', '').lower():
                    return False
            elif filter_type == 'transport':
                transport = filter_args[0]
                service_transport = service.get('config', {}).get('transport', 'unknown')
                if transport.lower() != service_transport.lower():
                    return False
        
        return True
    
    def _get_sort_value(self, service: Dict[str, Any], field: str):
        """获取排序字段的值"""
        if field == 'name':
            return service.get('name', '')
        elif field == 'tool_count':
            return service.get('tool_count', 0)
        elif field == 'last_heartbeat':
            heartbeat = service.get('last_heartbeat')
            if heartbeat:
                if isinstance(heartbeat, str):
                    try:
                        return datetime.fromisoformat(heartbeat.replace('Z', '+00:00'))
                    except ValueError:
                        return datetime.min
                elif isinstance(heartbeat, datetime):
                    return heartbeat
            return datetime.min
        return ''


class AgentQueryBuilder:
    """Agent查询构建器"""
    
    def __init__(self, registry):
        self.registry = registry
    
    async def with_services_async(self, min_count: int = 1):
        """查询有服务的Agent（异步版本）"""
        agents_with_services = []
        # Use get_all_agent_ids() instead of agent_clients.keys()
        for agent_id in self.registry.get_all_agent_ids():
            service_count = len(self.registry.get_all_service_names(agent_id))
            if service_count >= min_count:
                # 从 pykv 获取 client_ids
                client_ids = await self.registry.get_agent_clients_async(agent_id)
                agents_with_services.append({
                    'agent_id': agent_id,
                    'service_count': service_count,
                    'client_count': len(client_ids)
                })
        return agents_with_services

    def with_services(self, min_count: int = 1):
        """查询有服务的Agent（同步版本 - 使用 asyncio.run）"""
        import asyncio
        try:
            loop = asyncio.get_running_loop()
            raise RuntimeError("with_services cannot be called in async context, please use with_services_async")
        except RuntimeError:
            return asyncio.run(self.with_services_async(min_count))

    async def get_all_async(self) -> List[Dict[str, Any]]:
        """获取所有Agent信息（异步版本）"""
        agents = []
        # Use get_all_agent_ids() instead of agent_clients.keys()
        for agent_id in self.registry.get_all_agent_ids():
            # 从 pykv 获取 client_ids
            client_ids = await self.registry.get_agent_clients_async(agent_id)
            agents.append({
                'agent_id': agent_id,
                'service_count': len(self.registry.get_all_service_names(agent_id)),
                'client_count': len(client_ids),
                'healthy_services': len(self.registry.get_healthy_services(agent_id)),
                'failed_services': len(self.registry.get_failed_services(agent_id))
            })
        return agents

    def get_all(self) -> List[Dict[str, Any]]:
        """获取所有Agent信息（同步版本 - 使用 asyncio.run）"""
        import asyncio
        try:
            loop = asyncio.get_running_loop()
            raise RuntimeError("get_all cannot be called in async context, please use get_all_async")
        except RuntimeError:
            return asyncio.run(self.get_all_async())


class ClientQueryBuilder:
    """Client查询构建器"""
    
    def __init__(self, registry, agent_id: str):
        self.registry = registry
        self.agent_id = agent_id
    
    async def with_services_async(self, min_count: int = 1):
        """查询有服务的Client（异步版本）"""
        clients_with_services = []
        # 从 pykv 获取 client_ids
        client_ids = await self.registry.get_agent_clients_async(self.agent_id)
        
        for client_id in client_ids:
            client_config = self.registry.get_client_config_from_cache(client_id)
            if client_config:
                service_count = len(client_config.get('mcpServers', {}))
                if service_count >= min_count:
                    clients_with_services.append({
                        'client_id': client_id,
                        'service_count': service_count,
                        'services': list(client_config.get('mcpServers', {}).keys())
                    })
        
        return clients_with_services

    def with_services(self, min_count: int = 1):
        """查询有服务的Client（同步版本 - 使用 asyncio.run）"""
        import asyncio
        try:
            loop = asyncio.get_running_loop()
            raise RuntimeError("with_services cannot be called in async context, please use with_services_async")
        except RuntimeError:
            return asyncio.run(self.with_services_async(min_count))

    async def get_all_async(self) -> List[Dict[str, Any]]:
        """获取Agent下所有Client信息（异步版本）"""
        clients = []
        # 从 pykv 获取 client_ids
        client_ids = await self.registry.get_agent_clients_async(self.agent_id)
        
        for client_id in client_ids:
            client_config = self.registry.get_client_config_from_cache(client_id)
            clients.append({
                'client_id': client_id,
                'service_count': len(client_config.get('mcpServers', {})) if client_config else 0,
                'services': list(client_config.get('mcpServers', {}).keys()) if client_config else [],
                'config': client_config
            })
        
        return clients

    def get_all(self) -> List[Dict[str, Any]]:
        """获取Agent下所有Client信息（同步版本 - 使用 asyncio.run）"""
        import asyncio
        try:
            loop = asyncio.get_running_loop()
            raise RuntimeError("get_all cannot be called in async context, please use get_all_async")
        except RuntimeError:
            return asyncio.run(self.get_all_async())
