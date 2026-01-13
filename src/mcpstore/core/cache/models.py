"""
缓存架构数据模型

定义三层缓存架构中使用的所有数据模型。
"""

from dataclasses import dataclass, field, asdict
from typing import Any, Dict, List, Optional


# ==================== 实体层数据模型 ====================


@dataclass
class ServiceEntity:
    """
    服务实体
    
    存储在实体层的服务配置和元数据。
    """
    service_global_name: str
    service_original_name: str
    source_agent: str
    config: Dict[str, Any]
    added_time: int
    
    def to_dict(self) -> Dict[str, Any]:
        """转换为字典"""
        return asdict(self)
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> 'ServiceEntity':
        """从字典创建"""
        if not isinstance(data, dict):
            raise ValueError(f"Data must be a dictionary type, actual type: {type(data).__name__}")
        
        required_fields = [
            "service_global_name",
            "service_original_name",
            "source_agent",
            "config",
            "added_time"
        ]
        
        for field_name in required_fields:
            if field_name not in data:
                raise ValueError(f"Missing required field: {field_name}")
        
        return cls(
            service_global_name=data["service_global_name"],
            service_original_name=data["service_original_name"],
            source_agent=data["source_agent"],
            config=data["config"],
            added_time=data["added_time"]
        )


@dataclass
class ToolEntity:
    """
    工具实体
    
    存储在实体层的工具定义和 schema。
    """
    tool_global_name: str
    tool_original_name: str
    service_global_name: str
    service_original_name: str
    source_agent: str
    description: str
    input_schema: Dict[str, Any]
    created_time: int
    tool_hash: str
    
    def to_dict(self) -> Dict[str, Any]:
        """转换为字典"""
        return asdict(self)
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> 'ToolEntity':
        """从字典创建"""
        if not isinstance(data, dict):
            raise ValueError(f"Data must be a dictionary type, actual type: {type(data).__name__}")
        
        required_fields = [
            "tool_global_name",
            "tool_original_name",
            "service_global_name",
            "service_original_name",
            "source_agent",
            "description",
            "input_schema",
            "created_time",
            "tool_hash"
        ]
        
        for field_name in required_fields:
            if field_name not in data:
                raise ValueError(f"Missing required field: {field_name}")
        
        return cls(
            tool_global_name=data["tool_global_name"],
            tool_original_name=data["tool_original_name"],
            service_global_name=data["service_global_name"],
            service_original_name=data["service_original_name"],
            source_agent=data["source_agent"],
            description=data["description"],
            input_schema=data["input_schema"],
            created_time=data["created_time"],
            tool_hash=data["tool_hash"]
        )


@dataclass
class AgentEntity:
    """
    Agent 实体
    
    存储在实体层的 Agent 基础信息。
    """
    agent_id: str
    created_time: int
    last_active: int
    is_global: bool = False
    
    def to_dict(self) -> Dict[str, Any]:
        """转换为字典"""
        return asdict(self)
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> 'AgentEntity':
        """从字典创建"""
        if not isinstance(data, dict):
            raise ValueError(f"Data must be a dictionary type, actual type: {type(data).__name__}")
        
        required_fields = ["agent_id", "created_time", "last_active"]
        
        for field_name in required_fields:
            if field_name not in data:
                raise ValueError(f"Missing required field: {field_name}")
        
        return cls(
            agent_id=data["agent_id"],
            created_time=data["created_time"],
            last_active=data["last_active"],
            is_global=data.get("is_global", False)
        )


@dataclass
class StoreConfig:
    """
    Store 配置
    
    存储在实体层的全局配置信息。
    """
    mcp_version: str
    setup_time: int
    config_version: str
    mcp_json_path: str
    static_main_agent: str = "global_agent_store"
    
    def to_dict(self) -> Dict[str, Any]:
        """转换为字典"""
        return asdict(self)
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> 'StoreConfig':
        """从字典创建"""
        if not isinstance(data, dict):
            raise ValueError(f"Data must be a dictionary type, actual type: {type(data).__name__}")
        
        required_fields = [
            "mcp_version",
            "setup_time",
            "config_version",
            "mcp_json_path"
        ]
        
        for field_name in required_fields:
            if field_name not in data:
                raise ValueError(f"Missing required field: {field_name}")
        
        return cls(
            mcp_version=data["mcp_version"],
            setup_time=data["setup_time"],
            config_version=data["config_version"],
            mcp_json_path=data["mcp_json_path"],
            static_main_agent=data.get("static_main_agent", "global_agent_store")
        )


# ==================== 关系层数据模型 ====================


@dataclass
class ServiceRelationItem:
    """
    服务关系项
    
    Agent-Service 关系中的单个服务项。
    """
    service_original_name: str
    service_global_name: str
    client_id: str
    established_time: int
    last_access: Optional[int] = None
    
    def to_dict(self) -> Dict[str, Any]:
        """转换为字典"""
        return asdict(self)
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> 'ServiceRelationItem':
        """从字典创建"""
        if not isinstance(data, dict):
            raise ValueError(f"Data must be a dictionary type, actual type: {type(data).__name__}")
        
        required_fields = [
            "service_original_name",
            "service_global_name",
            "client_id",
            "established_time"
        ]
        
        for field_name in required_fields:
            if field_name not in data:
                raise ValueError(f"Missing required field: {field_name}")
        
        return cls(
            service_original_name=data["service_original_name"],
            service_global_name=data["service_global_name"],
            client_id=data["client_id"],
            established_time=data["established_time"],
            last_access=data.get("last_access")
        )


@dataclass
class AgentServiceRelation:
    """
    Agent-Service 关系
    
    存储在关系层的 Agent 与服务的映射关系。
    """
    services: List[ServiceRelationItem] = field(default_factory=list)
    
    def to_dict(self) -> Dict[str, Any]:
        """转换为字典"""
        return {
            "services": [item.to_dict() for item in self.services]
        }
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> 'AgentServiceRelation':
        """从字典创建"""
        if not isinstance(data, dict):
            raise ValueError(f"Data must be a dictionary type, actual type: {type(data).__name__}")
        
        services_data = data.get("services", [])
        if not isinstance(services_data, list):
            raise ValueError(f"services must be a list type, actual type: {type(services_data).__name__}")
        
        services = [
            ServiceRelationItem.from_dict(item)
            for item in services_data
        ]
        
        return cls(services=services)


@dataclass
class ToolRelationItem:
    """
    工具关系项
    
    Service-Tool 关系中的单个工具项。
    """
    tool_global_name: str
    tool_original_name: str
    
    def to_dict(self) -> Dict[str, Any]:
        """转换为字典"""
        return asdict(self)
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> 'ToolRelationItem':
        """从字典创建"""
        if not isinstance(data, dict):
            raise ValueError(f"Data must be a dictionary type, actual type: {type(data).__name__}")
        
        required_fields = ["tool_global_name", "tool_original_name"]
        
        for field_name in required_fields:
            if field_name not in data:
                raise ValueError(f"Missing required field: {field_name}")
        
        return cls(
            tool_global_name=data["tool_global_name"],
            tool_original_name=data["tool_original_name"]
        )


@dataclass
class ServiceToolRelation:
    """
    Service-Tool 关系
    
    存储在关系层的服务与工具的映射关系。
    """
    service_global_name: str
    service_original_name: str
    source_agent: str
    tools: List[ToolRelationItem] = field(default_factory=list)
    
    def to_dict(self) -> Dict[str, Any]:
        """转换为字典"""
        return {
            "service_global_name": self.service_global_name,
            "service_original_name": self.service_original_name,
            "source_agent": self.source_agent,
            "tools": [item.to_dict() for item in self.tools]
        }
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> 'ServiceToolRelation':
        """从字典创建"""
        if not isinstance(data, dict):
            raise ValueError(f"Data must be a dictionary type, actual type: {type(data).__name__}")
        
        required_fields = [
            "service_global_name",
            "service_original_name",
            "source_agent"
        ]
        
        for field_name in required_fields:
            if field_name not in data:
                raise ValueError(f"Missing required field: {field_name}")
        
        tools_data = data.get("tools", [])
        if not isinstance(tools_data, list):
            raise ValueError(f"tools must be a list type, actual type: {type(tools_data).__name__}")
        
        tools = [
            ToolRelationItem.from_dict(item)
            for item in tools_data
        ]
        
        return cls(
            service_global_name=data["service_global_name"],
            service_original_name=data["service_original_name"],
            source_agent=data["source_agent"],
            tools=tools
        )


# ==================== 状态层数据模型 ====================


@dataclass
class ToolStatusItem:
    """
    工具状态项
    
    服务状态中的单个工具状态。
    """
    tool_global_name: str
    tool_original_name: str
    status: str  # "available" | "unavailable"
    
    def to_dict(self) -> Dict[str, Any]:
        """转换为字典"""
        return asdict(self)
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> 'ToolStatusItem':
        """从字典创建"""
        if not isinstance(data, dict):
            raise ValueError(f"Data must be a dictionary type, actual type: {type(data).__name__}")
        
        required_fields = ["tool_global_name", "tool_original_name", "status"]
        
        for field_name in required_fields:
            if field_name not in data:
                raise ValueError(f"Missing required field: {field_name}")
        
        # 验证状态值
        valid_statuses = ["available", "unavailable"]
        if data["status"] not in valid_statuses:
            raise ValueError(
                f"无效的工具状态: {data['status']}. "
                f"有效值: {valid_statuses}"
            )
        
        return cls(
            tool_global_name=data["tool_global_name"],
            tool_original_name=data["tool_original_name"],
            status=data["status"]
        )


@dataclass
class ServiceStatus:
    """
    服务状态
    
    存储在状态层的服务运行时状态。
    """
    service_global_name: str
    health_status: str  # "healthy" | "unhealthy" | "unknown"
    last_health_check: int
    connection_attempts: int
    max_connection_attempts: int
    current_error: Optional[str]
    tools: List[ToolStatusItem] = field(default_factory=list)
    window_error_rate: Optional[float] = None
    latency_p95: Optional[float] = None
    latency_p99: Optional[float] = None
    sample_size: Optional[int] = None
    next_retry_time: Optional[float] = None
    hard_deadline: Optional[float] = None
    lease_deadline: Optional[float] = None
    
    def to_dict(self) -> Dict[str, Any]:
        """转换为字典"""
        return {
            "service_global_name": self.service_global_name,
            "health_status": self.health_status,
            "last_health_check": self.last_health_check,
            "connection_attempts": self.connection_attempts,
            "max_connection_attempts": self.max_connection_attempts,
            "current_error": self.current_error,
            "tools": [item.to_dict() for item in self.tools],
            "window_error_rate": self.window_error_rate,
            "latency_p95": self.latency_p95,
            "latency_p99": self.latency_p99,
            "sample_size": self.sample_size,
            "next_retry_time": self.next_retry_time,
            "hard_deadline": self.hard_deadline,
            "lease_deadline": self.lease_deadline,
        }
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> 'ServiceStatus':
        """从字典创建"""
        if not isinstance(data, dict):
            raise ValueError(f"Data must be a dictionary type, actual type: {type(data).__name__}")
        
        required_fields = [
            "service_global_name",
            "health_status",
            "last_health_check",
            "connection_attempts",
            "max_connection_attempts"
        ]
        
        for field_name in required_fields:
            if field_name not in data:
                raise ValueError(f"Missing required field: {field_name}")
        
        # 验证健康状态值
        valid_health_statuses = [
            "init", "startup", "ready", "healthy",
            "degraded", "circuit_open", "half_open", "disconnected"
        ]
        if data["health_status"] not in valid_health_statuses:
            raise ValueError(
                f"无效的健康状态: {data['health_status']}. "
                f"有效值: {valid_health_statuses}"
            )
        
        tools_data = data.get("tools", [])
        if not isinstance(tools_data, list):
            raise ValueError(f"tools must be a list type, actual type: {type(tools_data).__name__}")
        
        tools = [
            ToolStatusItem.from_dict(item)
            for item in tools_data
        ]
        
        return cls(
            service_global_name=data["service_global_name"],
            health_status=data["health_status"],
            last_health_check=data["last_health_check"],
            connection_attempts=data["connection_attempts"],
            max_connection_attempts=data["max_connection_attempts"],
            current_error=data.get("current_error"),
            tools=tools,
            window_error_rate=data.get("window_error_rate"),
            latency_p95=data.get("latency_p95"),
            latency_p99=data.get("latency_p99"),
            sample_size=data.get("sample_size"),
            next_retry_time=data.get("next_retry_time"),
            hard_deadline=data.get("hard_deadline"),
            lease_deadline=data.get("lease_deadline"),
        )
