"""
缓存架构模块

提供三层缓存架构的实现：
- 实体层 (Entity Layer)
- 关系层 (Relationship Layer)
- 状态层 (State Layer)
"""

from .cache_layer_manager import CacheLayerManager
from .naming_service import NamingService
from .scalar_value_wrapper import ScalarValueWrapper
from .service_entity_manager import ServiceEntityManager
from .tool_entity_manager import ToolEntityManager
from .relationship_manager import RelationshipManager
from .state_manager import StateManager
from .models import (
    ServiceEntity,
    ToolEntity,
    AgentEntity,
    StoreConfig,
    ServiceRelationItem,
    AgentServiceRelation,
    ToolRelationItem,
    ServiceToolRelation,
    ToolStatusItem,
    ServiceStatus,
)

__all__ = [
    # 管理器
    "CacheLayerManager",
    "NamingService",
    "ScalarValueWrapper",
    "ServiceEntityManager",
    "ToolEntityManager",
    "RelationshipManager",
    "StateManager",
    # 实体层模型
    "ServiceEntity",
    "ToolEntity",
    "AgentEntity",
    "StoreConfig",
    # 关系层模型
    "ServiceRelationItem",
    "AgentServiceRelation",
    "ToolRelationItem",
    "ServiceToolRelation",
    # 状态层模型
    "ToolStatusItem",
    "ServiceStatus",
]
