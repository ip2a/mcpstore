"""
命名服务

负责处理服务和工具的命名转换，实现双重视角命名：
- Agent 视角：看到原始名称（如 "context7"）
- Store 视角：看到全局唯一名称（如 "context7_byagent_agent1"）
"""

import logging
from typing import Tuple

logger = logging.getLogger(__name__)


class NamingService:
    """
    命名服务
    
    提供服务和工具的全局命名生成和解析功能。
    """
    
    # 全局代理标识，用于 Store 视角的服务管理
    GLOBAL_AGENT_STORE = "global_agent_store"
    
    # 命名分隔符
    AGENT_SEPARATOR = "_byagent_"
    
    @staticmethod
    def generate_service_global_name(original_name: str, agent_id: str) -> str:
        """
        生成服务全局名称
        
        规则：
        - 如果 agent_id 是 "global_agent_store"，返回原始名称
        - 否则，返回 "{original_name}_byagent_{agent_id}"
        
        Args:
            original_name: 服务原始名称
            agent_id: Agent ID
            
        Returns:
            服务全局名称
            
        Examples:
            >>> NamingService.generate_service_global_name("context7", "agent1")
            "context7_byagent_agent1"
            
            >>> NamingService.generate_service_global_name("context7", "global_agent_store")
            "context7"
        """
        if not original_name:
            raise ValueError("Service original name cannot be empty")
        if not agent_id:
            raise ValueError("Agent ID cannot be empty")
        
        if agent_id == NamingService.GLOBAL_AGENT_STORE:
            global_name = original_name
        else:
            global_name = f"{original_name}{NamingService.AGENT_SEPARATOR}{agent_id}"
        
        logger.debug(
            f"[NAMING] Generated service global name: original_name={original_name}, "
            f"agent_id={agent_id}, global_name={global_name}"
        )
        
        return global_name
    
    @staticmethod
    def generate_tool_global_name(
        service_global_name: str,
        tool_original_name: str
    ) -> str:
        """
        生成工具全局名称
        
        规则：
        - 如果工具名已经以服务全局名称开头，直接返回工具名
        - 否则，格式为 "{service_global_name}_{tool_original_name}"
        
        Args:
            service_global_name: 服务全局名称
            tool_original_name: 工具原始名称
            
        Returns:
            工具全局名称
            
        Examples:
            >>> NamingService.generate_tool_global_name(
            ...     "context7_byagent_agent1",
            ...     "resolve-library-id"
            ... )
            "context7_byagent_agent1_resolve-library-id"
            
            >>> NamingService.generate_tool_global_name(
            ...     "context7",
            ...     "resolve-library-id"
            ... )
            "context7_resolve-library-id"
            
            >>> NamingService.generate_tool_global_name(
            ...     "mcpstore",
            ...     "mcpstore_get_current_weather"
            ... )
            "mcpstore_get_current_weather"  # 已包含服务前缀，不重复添加
        """
        if not service_global_name:
            raise ValueError("Service global name cannot be empty")
        if not tool_original_name:
            raise ValueError("Tool original name cannot be empty")
        
        # 检查工具名是否已经以服务全局名称开头
        # 避免重复添加前缀
        if tool_original_name.startswith(f"{service_global_name}_"):
            tool_global_name = tool_original_name
        else:
            tool_global_name = f"{service_global_name}_{tool_original_name}"
        
        logger.debug(
            f"[NAMING] Generated tool global name: service_global_name={service_global_name}, "
            f"tool_original_name={tool_original_name}, "
            f"tool_global_name={tool_global_name}"
        )
        
        return tool_global_name
    
    @staticmethod
    def parse_service_global_name(global_name: str) -> Tuple[str, str]:
        """
        解析服务全局名称
        
        规则：
        - 如果包含 "_byagent_"，拆分为 (original_name, agent_id)
        - 否则，认为是 global_agent_store 的服务，返回 (global_name, "global_agent_store")
        
        Args:
            global_name: 服务全局名称
            
        Returns:
            (original_name, agent_id) 元组
            
        Examples:
            >>> NamingService.parse_service_global_name("context7_byagent_agent1")
            ("context7", "agent1")
            
            >>> NamingService.parse_service_global_name("context7")
            ("context7", "global_agent_store")
        """
        if not global_name:
            raise ValueError("Service global name cannot be empty")
        
        if NamingService.AGENT_SEPARATOR not in global_name:
            # 没有分隔符，认为是 global_agent_store 的服务
            original_name = global_name
            agent_id = NamingService.GLOBAL_AGENT_STORE
        else:
            # 从右侧拆分，只拆分一次（防止服务名中包含分隔符）
            parts = global_name.rsplit(NamingService.AGENT_SEPARATOR, 1)
            if len(parts) != 2:
                raise ValueError(
                    f"Invalid service global name format: {global_name}. "
                    f"Expected format: 'name{NamingService.AGENT_SEPARATOR}agent_id' or 'name'"
                )
            original_name, agent_id = parts
        
        logger.debug(
            f"[NAMING] Parsed service global name: global_name={global_name}, "
            f"original_name={original_name}, agent_id={agent_id}"
        )
        
        return original_name, agent_id
    
    @staticmethod
    def is_global_agent_service(agent_id: str) -> bool:
        """
        判断是否为全局代理的服务
        
        Args:
            agent_id: Agent ID
            
        Returns:
            如果是 global_agent_store，返回 True
        """
        return agent_id == NamingService.GLOBAL_AGENT_STORE
