"""
工具操作的纯逻辑核心

严格约束：
- 必须是纯同步函数
- 不包含任何 IO 操作（no pykv, no file IO, no network IO）
- 不调用任何异步方法
- 不使用 await/asyncio.run()
- 只做计算，不执行实际操作
"""

import logging
from dataclasses import dataclass
from typing import Any, Dict, List, Optional

logger = logging.getLogger(__name__)


@dataclass
class ToolInfo:
    """工具信息（纯数据结构）"""
    name: str  # 工具全局名（L3）
    tool_original_name: str  # FastMCP 标准格式（L2）
    description: str
    service_name: str  # 服务原始名（L0，保持与 service_original_name 一致）
    service_original_name: str  # 服务原始名（L0/FastMCP 视角）
    service_global_name: str  # 服务全局名（L3）
    client_id: Optional[str]
    inputSchema: Dict[str, Any]
    
    @classmethod
    def from_entity(
        cls,
        entity_data: Dict[str, Any],
        service_original_name: str,
        service_global_name: str,
        client_id: Optional[str] = None
    ) -> "ToolInfo":
        """从实体数据创建 ToolInfo"""
        if not entity_data.get("tool_original_name"):
            raise ValueError("tool_original_name is missing, cannot build ToolInfo")
        if not service_global_name:
            raise ValueError("service_global_name is missing, cannot build ToolInfo")
        return cls(
            name=entity_data.get("tool_global_name", ""),
            tool_original_name=entity_data.get("tool_original_name", ""),
            description=entity_data.get("description", ""),
            service_name=service_original_name,
            service_original_name=service_original_name,
            service_global_name=service_global_name,
            client_id=client_id,
            inputSchema=entity_data.get("input_schema", {})
        )
    
    def to_dict(self) -> Dict[str, Any]:
        """转换为字典"""
        return {
            "name": self.name,
            "description": self.description,
            "service_name": self.service_name,
            "service_original_name": self.service_original_name,
            "service_global_name": self.service_global_name,
            "tool_original_name": self.tool_original_name,
            "client_id": self.client_id,
            "inputSchema": self.inputSchema
        }


@dataclass
class ToolStatusItem:
    """工具状态项（纯数据结构）"""
    tool_global_name: str
    tool_original_name: str
    status: str  # "available" | "unavailable"


@dataclass
class ServiceStatus:
    """服务状态（纯数据结构）"""
    service_global_name: str
    health_status: str
    tools: List[ToolStatusItem]
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> "ServiceStatus":
        """从字典创建 ServiceStatus"""
        tools = []
        for tool_data in data.get("tools", []):
            tools.append(ToolStatusItem(
                tool_global_name=tool_data.get("tool_global_name", ""),
                tool_original_name=tool_data.get("tool_original_name", ""),
                status=tool_data.get("status", "unavailable")
            ))
        
        return cls(
            service_global_name=data.get("service_global_name", ""),
            health_status=data.get("health_status", "unknown"),
            tools=tools
        )


class ToolLogicCore:
    """
    工具操作的纯逻辑核心
    
    严格遵循 Functional Core 原则：
    - 所有方法都是纯同步函数
    - 不包含任何 IO 操作
    - 只做计算和数据转换
    - 遇到错误必须抛出，不做静默处理
    """
    
    @staticmethod
    def extract_original_tool_name(
        tool_name: str,
        service_name: str,
        alt_service_name: Optional[str] = None
    ) -> str:
        """
        提取工具的原始名称（去除服务前缀）
        优先使用提供的服务名/备用服务名做前缀匹配，匹配不上则原样返回。
        """
        for prefix in (service_name, alt_service_name):
            if prefix:
                full_prefix = f"{prefix}_"
                if tool_name.startswith(full_prefix):
                    return tool_name[len(full_prefix):]
        return tool_name
    
    @staticmethod
    def build_tools_from_entities(
        tool_entities: List[Dict[str, Any]],
        service_relations: List[Dict[str, Any]],
        client_id_map: Dict[str, str]
    ) -> List[ToolInfo]:
        """
        从实体数据构建工具列表
        
        纯同步计算，无 IO。
        
        Args:
            tool_entities: 工具实体列表（从 pykv 实体层读取）
            service_relations: 服务关系列表（从 pykv 关系层读取）
            client_id_map: 服务名到 client_id 的映射
        
        Returns:
            ToolInfo 列表
        """
        # 构建服务全局名到原始名的映射
        service_name_map: Dict[str, str] = {}
        for rel in service_relations:
            global_name = rel.get("service_global_name")
            original_name = rel.get("service_original_name")
            if global_name and original_name:
                service_name_map[global_name] = original_name
        
        tools = []
        for entity in tool_entities:
            if entity is None:
                continue
            
            service_global_name = entity.get("service_global_name", "")
            service_original_name = service_name_map.get(
                service_global_name,
                entity.get("service_original_name", "")
            )
            client_id = client_id_map.get(service_global_name)
            
            tool_info = ToolInfo.from_entity(
                entity,
                service_original_name,
                service_global_name,
                client_id
            )
            tools.append(tool_info)
        
        return tools
    
    @staticmethod
    def filter_tools_by_service(
        tools: List[ToolInfo],
        service_name: Optional[str]
    ) -> List[ToolInfo]:
        """
        按服务名筛选工具
        
        纯同步计算，无 IO。
        
        Args:
            tools: 工具列表
            service_name: 服务名称（None 表示不筛选）
        
        Returns:
            筛选后的工具列表
        """
        if service_name is None:
            return tools
        
        return [t for t in tools if t.service_name == service_name]
    
    @staticmethod
    def filter_tools_by_availability(
        tools: List[ToolInfo],
        service_status_map: Dict[str, ServiceStatus]
    ) -> List[ToolInfo]:
        """
        按可用性筛选工具
        
        纯同步计算，无 IO。
        遇到错误必须抛出，不做静默处理。
        
        Args:
            tools: 工具列表
            service_status_map: 服务状态映射（service_global_name -> ServiceStatus）
        
        Returns:
            可用的工具列表
            
        Raises:
            RuntimeError: 如果服务状态不存在或工具状态不存在
        """
        result = []
        
        for tool in tools:
            service_global_name = tool.service_global_name
            
            # 获取服务状态
            status = service_status_map.get(service_global_name)
            if status is None:
                raise RuntimeError(
                    f"Service state does not exist, cannot check tool availability: "
                    f"service_global_name={service_global_name}, tool={tool.name}"
                )
            
            original_tool_name = tool.tool_original_name or ToolLogicCore.extract_original_tool_name(
                tool.name,
                service_global_name,
                tool.service_name
            )
            
            # 查找工具状态
            tool_status = None
            for ts in status.tools:
                if ts.tool_original_name == original_tool_name:
                    tool_status = ts
                    break
            
            if tool_status is None:
                raise RuntimeError(
                    f"Tool does not exist in service state: "
                    f"service_global_name={service_global_name}, "
                    f"tool={tool.name}, original_name={original_tool_name}"
                )
            
            # 只返回可用的工具
            if tool_status.status == "available":
                result.append(tool)
        
        return result
    
    @staticmethod
    def _extract_service_global_name(tool_global_name: str) -> str:
        """
        从工具全局名称中提取服务全局名称
        
        工具全局名称格式：{service_global_name}_{tool_original_name}
        
        Args:
            tool_global_name: 工具全局名称
        
        Returns:
            服务全局名称
        """
        # 找到最后一个下划线的位置
        last_underscore = tool_global_name.rfind("_")
        if last_underscore == -1:
            return tool_global_name
        
        return tool_global_name[:last_underscore]
    
    @staticmethod
    def map_to_agent_view(
        tools: List[ToolInfo],
        global_to_local_map: Dict[str, str]
    ) -> List[ToolInfo]:
        """
        将全局工具名映射到 Agent 本地视图
        
        纯同步计算，无 IO。
        
        Args:
            tools: 工具列表（全局名称）
            global_to_local_map: 全局服务名到本地服务名的映射
        
        Returns:
            本地视图的工具列表
        """
        result = []
        
        for tool in tools:
            service_global_name = tool.service_global_name
            local_service_name = global_to_local_map.get(service_global_name)
            if local_service_name is None:
                # 没有映射，跳过
                continue
            
            # 创建本地视图的工具
            local_tool_name = tool.name.replace(
                f"{service_global_name}_",
                f"{local_service_name}_",
                1
            )
            
            local_tool = ToolInfo(
                name=local_tool_name,
                tool_original_name=tool.tool_original_name,
                description=tool.description,
                service_name=local_service_name,
                service_original_name=local_service_name,
                service_global_name=service_global_name,
                client_id=tool.client_id,
                inputSchema=tool.inputSchema
            )
            result.append(local_tool)
        
        return result
    
    @staticmethod
    def check_tool_availability(
        service_global_name: str,
        tool_name: str,
        service_status: Optional[Dict[str, Any]],
        tool_original_name_override: Optional[str] = None,
        service_original_name: Optional[str] = None,
    ) -> bool:
        """
        检查单个工具的可用性
        
        纯同步计算，无 IO。
        遇到错误必须抛出，不做静默处理。
        
        Args:
            service_global_name: 服务全局名称
            tool_name: 工具名称
            service_status: 服务状态数据（从 pykv 状态层读取）
        
        Returns:
            True 如果工具可用，否则 False
            
        Raises:
            RuntimeError: 如果服务状态不存在或工具状态不存在
        """
        if service_status is None:
            raise RuntimeError(
                f"Service state does not exist, cannot check tool availability: "
                f"service_global_name={service_global_name}, tool={tool_name}"
            )
        
        # 提取工具原始名称
        original_tool_name = (
            tool_original_name_override
            or ToolLogicCore.extract_original_tool_name(
                tool_name,
                service_global_name,
                service_original_name
            )
        )
        
        # 查找工具状态
        tools = service_status.get("tools", [])

        tool_status = None
        for ts in tools:
            if ts.get("tool_original_name") == original_tool_name:
                tool_status = ts
                break
        
        if tool_status is None:
            raise RuntimeError(
                f"Tool does not exist in service state: "
                f"service_global_name={service_global_name}, "
                f"tool={tool_name}, original_name={original_tool_name}"
            )
        
        return tool_status.get("status") == "available"
