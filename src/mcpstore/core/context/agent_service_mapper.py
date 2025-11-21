"""
Agent Service Name Mapper

Responsible for converting between Agent's local names and global names:
- Local names: Original service names seen by Agent (e.g., "demo")
- Global names: Internal storage names with suffix (e.g., "demobyagent1")

Design principles:
1. Agent only sees original names in its own space
2. Internal storage and synchronization use global names with suffix
3. Provide bidirectional conversion and filtering functions
"""

import logging
from typing import Dict, Any, List, Optional

logger = logging.getLogger(__name__)


class AgentServiceMapper:
    """Agent service name mapper"""
    
    def __init__(self, agent_id: str):
        """
        Initialize mapper

        Args:
            agent_id: Agent ID
        """
        self.agent_id = agent_id
        self.suffix = f"_byagent_{agent_id}"
        
    def to_global_name(self, local_name: str) -> str:
        """
        Convert local name to global name

        Args:
            local_name: Original service name seen by Agent

        Returns:
            Global storage service name with suffix (format: service_byagent_agentid)
        """
        return f"{local_name}{self.suffix}"
    
    def to_local_name(self, global_name: str) -> str:
        """
        Convert global name to local name

        Args:
            global_name: Global storage service name with suffix

        Returns:
            Original service name seen by Agent
        """
        if global_name.endswith(self.suffix):
            return global_name[:-len(self.suffix)]
        return global_name
    
    def is_agent_service(self, global_name: str) -> bool:
        """
        Determine if service belongs to current Agent

        Args:
            global_name: Global service name

        Returns:
            Whether it belongs to current Agent
        """
        return global_name.endswith(self.suffix)

    @staticmethod
    def is_any_agent_service(service_name: str) -> bool:
        """
        Determine if service belongs to any Agent (static method)

        Args:
            service_name: Service name to check

        Returns:
            Whether it's an Agent service (contains _byagent_ pattern)
        """
        return "_byagent_" in service_name

    @staticmethod
    def parse_agent_service_name(global_name: str) -> tuple[str, str]:
        """
        Parse Agent service name to extract agent_id and local_name

        Args:
            global_name: Global service name (format: service_byagent_agentid)

        Returns:
            Tuple of (agent_id, local_name)

        Raises:
            ValueError: If the service name format is invalid
        """
        if not AgentServiceMapper.is_any_agent_service(global_name):
            raise ValueError(f"Not an Agent service: {global_name}")

        # 允许 agent_id 含有下划线等字符；只要包含分隔符即可
        if "_byagent_" not in global_name:
            raise ValueError(f"Invalid Agent service name format: {global_name}")

        local_name, agent_id = global_name.split("_byagent_", 1)
        if not local_name or not agent_id:
            raise ValueError(f"Invalid Agent service name format: {global_name}")

        # 放宽校验：不再限制 agent_id 中的下划线，保持单一分隔符规则
        return agent_id.strip(), local_name.strip()

    def filter_agent_services(self, global_services: Dict[str, Any]) -> Dict[str, Any]:
        """
        从全局服务中过滤出属于当前Agent的服务，并转换为本地名称
        
        Args:
            global_services: 全局服务配置字典
            
        Returns:
            本地服务配置字典（使用原始名称）
        """
        local_services = {}
        
        for global_name, config in global_services.items():
            if self.is_agent_service(global_name):
                local_name = self.to_local_name(global_name)
                local_services[local_name] = config
                logger.debug(f"Mapped service: {global_name} -> {local_name}")
        
        return local_services
    
    def convert_service_list_to_local(self, global_service_infos: List[Any]) -> List[Any]:
        """
        将全局服务信息列表转换为本地服务信息列表
        
        Args:
            global_service_infos: 全局服务信息列表
            
        Returns:
            本地服务信息列表（使用原始名称）
        """
        local_service_infos = []
        
        for service_info in global_service_infos:
            if self.is_agent_service(service_info.name):
                # 创建新的服务信息对象，使用本地名称
                local_name = self.to_local_name(service_info.name)
                
                # 复制服务信息，但使用本地名称
                # 注意：ServiceInfo没有tools属性，工具信息需要单独获取
                local_service_info = type(service_info)(
                    name=local_name,
                    transport_type=service_info.transport_type,
                    status=service_info.status,
                    tool_count=service_info.tool_count,
                    keep_alive=service_info.keep_alive,
                    url=getattr(service_info, 'url', ''),
                    working_dir=getattr(service_info, 'working_dir', None),
                    env=getattr(service_info, 'env', None),
                    last_heartbeat=getattr(service_info, 'last_heartbeat', None),
                    command=getattr(service_info, 'command', None),
                    args=getattr(service_info, 'args', None),
                    package_name=getattr(service_info, 'package_name', None),
                    state_metadata=getattr(service_info, 'state_metadata', None),
                    last_state_change=getattr(service_info, 'last_state_change', None),
                    client_id=getattr(service_info, 'client_id', None),
                    config=getattr(service_info, 'config', {})  #  [REFACTOR] 复制config字段
                )
                
                local_service_infos.append(local_service_info)
                logger.debug(f"Converted service info: {service_info.name} -> {local_name}")
        
        return local_service_infos
    

    
    def find_global_tool_name(self, local_tool_name: str, available_tools: List[str]) -> Optional[str]:
        """
        根据本地工具名称查找对应的全局工具名称
        
        Args:
            local_tool_name: 本地工具名称（如 "demo_get_weather"）
            available_tools: 可用的全局工具名称列表
            
        Returns:
            对应的全局工具名称，如果找不到则返回None
        """
        # 解析本地工具名称
        if "_" not in local_tool_name:
            # 如果没有下划线，可能是直接的工具名
            return None
        
        local_service_name, tool_suffix = local_tool_name.split("_", 1)
        global_service_name = self.to_global_name(local_service_name)
        expected_global_tool_name = f"{global_service_name}_{tool_suffix}"
        
        # 在可用工具中查找
        if expected_global_tool_name in available_tools:
            logger.debug(f"Found global tool: {local_tool_name} -> {expected_global_tool_name}")
            return expected_global_tool_name
        
        # 如果找不到精确匹配，尝试模糊匹配
        for global_tool_name in available_tools:
            if global_tool_name.startswith(f"{global_service_name}_"):
                tool_part = global_tool_name[len(f"{global_service_name}_"):]
                if tool_part == tool_suffix:
                    logger.debug(f"Found global tool (fuzzy): {local_tool_name} -> {global_tool_name}")
                    return global_tool_name
        
        logger.warning(f"Could not find global tool for local tool: {local_tool_name}")
        return None
    
    def convert_config_to_local(self, global_config: Dict[str, Any]) -> Dict[str, Any]:
        """
        将全局配置转换为本地配置（Agent视角）
        
        Args:
            global_config: 全局配置（包含所有服务）
            
        Returns:
            本地配置（只包含当前Agent的服务，使用原始名称）
        """
        if "mcpServers" not in global_config:
            return {"mcpServers": {}}
        
        local_servers = self.filter_agent_services(global_config["mcpServers"])
        
        return {
            "mcpServers": local_servers,
            # 保留其他配置项
            **{k: v for k, v in global_config.items() if k != "mcpServers"}
        }
    
    def convert_config_to_global(self, local_config: Dict[str, Any]) -> Dict[str, Any]:
        """
        将本地配置转换为全局配置（用于存储）
        
        Args:
            local_config: 本地配置（使用原始名称）
            
        Returns:
            全局配置（使用带后缀名称）
        """
        if "mcpServers" not in local_config:
            return local_config
        
        global_servers = {}
        for local_name, config in local_config["mcpServers"].items():
            global_name = self.to_global_name(local_name)
            global_servers[global_name] = config
            logger.debug(f"Converted config: {local_name} -> {global_name}")
        
        return {
            "mcpServers": global_servers,
            # 保留其他配置项
            **{k: v for k, v in local_config.items() if k != "mcpServers"}
        }
