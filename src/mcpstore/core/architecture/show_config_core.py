"""
ShowConfigLogicCore - show_config 的纯逻辑核心

遵循 "Functional Core, Imperative Shell" 架构原则：
- 纯同步函数
- 不包含任何 IO 操作（no pykv, no file IO, no network IO）
- 不调用任何异步方法
- 不使用 await/asyncio.run()
- 只做数据组装和计算，不执行实际操作

返回格式说明：
show_config 返回类似 mcp.json 的格式：
{
    "mcpServers": {
        "context7": {"url": "https://mcp.context7.com/mcp"},
        "grep": {"url": "https://mcp.grep.app"},
        "spec-workflow-mcp": {"command": "npx", "args": ["-y", "spec-workflow-mcp@latest"]}
    }
}
"""

import logging
from typing import Dict, Any, Optional

logger = logging.getLogger(__name__)


class ShowConfigLogicCore:
    """
    show_config 的纯逻辑核心
    
    职责：
    - 组装配置数据结构（类似 mcp.json 格式）
    - 数据格式转换
    
    严格约束：
    - 所有方法必须是纯同步函数
    - 输入：从 pykv 预读取的纯数据（字典、列表等）
    - 输出：组装好的配置数据结构（mcpServers 格式）
    """
    
    def build_store_config(
        self,
        services_data: Dict[str, Dict[str, Any]]
    ) -> Dict[str, Any]:
        """
        构建 Store 级别配置（类似 mcp.json 格式）
        
        纯同步计算，组装 Store 级别配置数据结构。
        
        Args:
            services_data: 从 pykv 预读取的服务数据
                格式: {
                    service_original_name: {
                        "config": {"url": "..."} 或 {"command": "...", "args": [...]}
                    }
                }
        
        Returns:
            类似 mcp.json 的配置结构:
            {
                "mcpServers": {
                    "service_name": {"url": "..."} 或 {"command": "...", "args": [...]}
                }
            }
        """
        mcp_servers = {}
        
        for service_name, service_info in services_data.items():
            # 提取服务配置（url/command/args 等）
            config = service_info.get("config", {})
            if config:
                mcp_servers[service_name] = config
        
        return {
            "mcpServers": mcp_servers
        }
    
    def build_agent_config(
        self,
        agent_id: str,
        services_data: Dict[str, Dict[str, Any]]
    ) -> Dict[str, Any]:
        """
        构建 Agent 级别配置（类似 mcp.json 格式）
        
        纯同步计算，组装 Agent 级别配置数据结构。
        
        Args:
            agent_id: Agent ID
            services_data: 从 pykv 预读取的服务数据
                格式: {
                    service_original_name: {
                        "config": {"url": "..."} 或 {"command": "...", "args": [...]}
                    }
                }
        
        Returns:
            类似 mcp.json 的配置结构（带 agent_id）:
            {
                "agent_id": "...",
                "mcpServers": {
                    "service_name": {"url": "..."} 或 {"command": "...", "args": [...]}
                }
            }
        """
        mcp_servers = {}
        
        for service_name, service_info in services_data.items():
            # 提取服务配置（url/command/args 等）
            config = service_info.get("config", {})
            if config:
                mcp_servers[service_name] = config
        
        return {
            "agent_id": agent_id,
            "mcpServers": mcp_servers
        }
    
    def build_error_response(
        self,
        error_message: str,
        agent_id: Optional[str] = None
    ) -> Dict[str, Any]:
        """
        构建错误响应
        
        纯同步计算，构建标准化的错误响应结构。
        
        Args:
            error_message: 错误信息
            agent_id: 可选的 Agent ID
        
        Returns:
            标准化的错误响应结构
        """
        response = {
            "error": error_message,
            "mcpServers": {}
        }
        
        if agent_id:
            response["agent_id"] = agent_id
        
        return response
    
    def extract_service_config(
        self,
        service_entity: Dict[str, Any]
    ) -> Dict[str, Any]:
        """
        从服务实体中提取配置
        
        纯同步计算，从 ServiceEntity 中提取 mcp.json 格式的配置。
        
        ServiceEntity 结构:
        {
            "service_global_name": "context7@global_agent_store",
            "service_original_name": "context7",
            "source_agent": "global_agent_store",
            "config": {"url": "https://mcp.context7.com/mcp"},
            "added_time": 1234567890
        }
        
        Args:
            service_entity: 从 pykv 获取的服务实体
        
        Returns:
            服务配置（url/command/args 等）
        """
        return service_entity.get("config", {})
