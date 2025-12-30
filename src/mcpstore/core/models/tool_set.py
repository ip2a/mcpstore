"""
工具集状态数据模型

本模块定义了工具集管理系统的核心数据模型。
"""

import time
from dataclasses import dataclass, field
from typing import Set, Dict, Any, List


@dataclass
class ToolSetState:
    """
    Agent 服务的工具集状态
    
    表示某个 Agent 对某个服务的工具集管理状态，包括当前可用的工具集合、
    操作历史等信息。
    
    Attributes:
        agent_id: Agent 的唯一标识符
        service_name: 服务名称（Agent 本地名称）
        available_tools: 当前可用的工具名称集合
        created_at: 创建时间戳
        updated_at: 最后更新时间戳
        version: 版本号，用于并发控制
        operation_history: 操作历史记录列表（最多保留10条）
    """
    
    agent_id: str
    service_name: str
    available_tools: Set[str] = field(default_factory=set)
    
    # 元数据
    created_at: float = field(default_factory=time.time)
    updated_at: float = field(default_factory=time.time)
    version: int = 1
    
    # 操作历史（可选）
    operation_history: List[Dict[str, Any]] = field(default_factory=list)
    
    def to_dict(self) -> Dict[str, Any]:
        """
        序列化为字典
        
        将 ToolSetState 对象转换为可以存储到缓存的字典格式。
        
        Returns:
            包含所有状态信息的字典
        """
        return {
            "agent_id": self.agent_id,
            "service_name": self.service_name,
            "available_tools": list(self.available_tools),
            "created_at": self.created_at,
            "updated_at": self.updated_at,
            "version": self.version,
            "operation_history": self.operation_history[-10:]  # 只保留最近10条
        }
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> 'ToolSetState':
        """
        从字典反序列化
        
        从缓存中读取的字典数据创建 ToolSetState 对象。
        
        Args:
            data: 包含状态信息的字典
            
        Returns:
            ToolSetState 对象实例
        """
        return cls(
            agent_id=data["agent_id"],
            service_name=data["service_name"],
            available_tools=set(data.get("available_tools", [])),
            created_at=data.get("created_at", time.time()),
            updated_at=data.get("updated_at", time.time()),
            version=data.get("version", 1),
            operation_history=data.get("operation_history", [])
        )
    
    def add_tools(self, tool_names: Set[str]) -> None:
        """
        添加工具到可用集合
        
        将指定的工具添加到当前可用工具集合中。这是一个增量操作，
        不会影响已存在的工具。
        
        Args:
            tool_names: 要添加的工具名称集合
        """
        self.available_tools.update(tool_names)
        self.updated_at = time.time()
        self.version += 1
        self._record_operation("add", list(tool_names))
    
    def remove_tools(self, tool_names: Set[str]) -> None:
        """
        从可用集合移除工具
        
        将指定的工具从当前可用工具集合中移除。如果工具不存在，
        不会产生错误。
        
        Args:
            tool_names: 要移除的工具名称集合
        """
        self.available_tools.difference_update(tool_names)
        self.updated_at = time.time()
        self.version += 1
        self._record_operation("remove", list(tool_names))
    
    def reset(self, all_tools: Set[str]) -> None:
        """
        重置为所有工具
        
        将可用工具集合重置为指定的完整工具集。通常用于恢复到
        服务的默认状态（所有工具可用）。
        
        Args:
            all_tools: 完整的工具名称集合
        """
        self.available_tools = all_tools.copy()
        self.updated_at = time.time()
        self.version += 1
        self._record_operation("reset", [])
    
    def _record_operation(self, op_type: str, tools: List[str]) -> None:
        """
        记录操作历史
        
        将操作记录添加到历史列表中，用于审计和调试。
        
        Args:
            op_type: 操作类型（"add", "remove", "reset"）
            tools: 涉及的工具列表
        """
        self.operation_history.append({
            "type": op_type,
            "tools": tools,
            "timestamp": time.time()
        })
