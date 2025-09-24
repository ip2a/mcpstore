#!/usr/bin/env python3
"""
Component Control and Filtering
Tag-based dynamic filtering, supports enabling/disabling components, creating environment configuration files
"""

import json
import logging
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path
from typing import Dict, List, Set, Any, Optional

logger = logging.getLogger(__name__)

class ComponentType(Enum):
    """Component types"""
    TOOL = "tool"
    RESOURCE = "resource"
    PROMPT = "prompt"
    SERVICE = "service"

class EnvironmentType(Enum):
    """Environment types"""
    DEVELOPMENT = "development"
    TESTING = "testing"
    STAGING = "staging"
    PRODUCTION = "production"
    CUSTOM = "custom"

@dataclass
class ComponentInfo:
    """Component information"""
    name: str
    component_type: ComponentType
    tags: Set[str] = field(default_factory=set)
    enabled: bool = True
    service_name: Optional[str] = None
    description: Optional[str] = None
    metadata: Dict[str, Any] = field(default_factory=dict)

@dataclass
class EnvironmentProfile:
    """Environment configuration file"""
    name: str
    environment_type: EnvironmentType
    allowed_tags: Set[str] = field(default_factory=set)
    blocked_tags: Set[str] = field(default_factory=set)
    allowed_components: Set[str] = field(default_factory=set)
    blocked_components: Set[str] = field(default_factory=set)
    component_overrides: Dict[str, bool] = field(default_factory=dict)  # 组件启用/禁用覆盖
    description: Optional[str] = None

class ComponentFilter:
    """组件过滤器"""
    
    def __init__(self):
        self._components: Dict[str, ComponentInfo] = {}
        self._tag_index: Dict[str, Set[str]] = {}  # tag -> component_names
        self._type_index: Dict[ComponentType, Set[str]] = {}  # type -> component_names
    
    def register_component(self, component: ComponentInfo):
        """注册组件"""
        self._components[component.name] = component
        
        # 更新标签索引
        for tag in component.tags:
            if tag not in self._tag_index:
                self._tag_index[tag] = set()
            self._tag_index[tag].add(component.name)
        
        # 更新类型索引
        if component.component_type not in self._type_index:
            self._type_index[component.component_type] = set()
        self._type_index[component.component_type].add(component.name)
        
        logger.debug(f"Registered component: {component.name} ({component.component_type.value})")
    
    def filter_by_tags(self, include_tags: List[str] = None, exclude_tags: List[str] = None) -> List[ComponentInfo]:
        """基于标签过滤组件"""
        include_tags = set(include_tags or [])
        exclude_tags = set(exclude_tags or [])
        
        result = []
        for component in self._components.values():
            # 检查包含标签
            if include_tags and not include_tags.intersection(component.tags):
                continue
            
            # 检查排除标签
            if exclude_tags and exclude_tags.intersection(component.tags):
                continue
            
            # 检查是否启用
            if not component.enabled:
                continue
            
            result.append(component)
        
        return result
    
    def filter_by_type(self, component_type: ComponentType) -> List[ComponentInfo]:
        """按类型过滤组件"""
        component_names = self._type_index.get(component_type, set())
        return [self._components[name] for name in component_names if self._components[name].enabled]
    
    def get_components_by_service(self, service_name: str) -> List[ComponentInfo]:
        """获取指定服务的组件"""
        return [
            component for component in self._components.values()
            if component.service_name == service_name and component.enabled
        ]
    
    def enable_component(self, component_name: str, enabled: bool = True):
        """启用/禁用组件"""
        if component_name in self._components:
            self._components[component_name].enabled = enabled
            logger.info(f"Component {component_name} {'enabled' if enabled else 'disabled'}")
        else:
            logger.warning(f"Component {component_name} not found")
    
    def bulk_enable_components(self, component_names: List[str], enabled: bool = True):
        """批量启用/禁用组件"""
        for name in component_names:
            self.enable_component(name, enabled)
    
    def get_component_info(self, component_name: str) -> Optional[ComponentInfo]:
        """获取组件信息"""
        return self._components.get(component_name)
    
    def list_all_tags(self) -> List[str]:
        """列出所有标签"""
        return list(self._tag_index.keys())
    
    def get_components_with_tag(self, tag: str) -> List[ComponentInfo]:
        """获取具有指定标签的组件"""
        component_names = self._tag_index.get(tag, set())
        return [self._components[name] for name in component_names]

class EnvironmentManager:
    """环境管理器"""
    
    def __init__(self, config_dir: Optional[Path] = None):
        self.config_dir = config_dir or Path.home() / ".mcpstore" / "environments"
        self.config_dir.mkdir(parents=True, exist_ok=True)
        self._profiles: Dict[str, EnvironmentProfile] = {}
        self._current_profile: Optional[str] = None
        self._load_default_profiles()
    
    def _load_default_profiles(self):
        """加载默认环境配置"""
        # 开发环境：允许所有工具
        dev_profile = EnvironmentProfile(
            name="development",
            environment_type=EnvironmentType.DEVELOPMENT,
            allowed_tags={"development", "testing", "debug", "experimental"},
            description="Development environment with all tools enabled"
        )
        self._profiles["development"] = dev_profile
        
        # 生产环境：只允许安全的工具
        prod_profile = EnvironmentProfile(
            name="production",
            environment_type=EnvironmentType.PRODUCTION,
            allowed_tags={"production", "safe", "stable"},
            blocked_tags={"experimental", "debug", "dangerous"},
            description="Production environment with only safe, stable tools"
        )
        self._profiles["production"] = prod_profile
        
        # 测试环境
        test_profile = EnvironmentProfile(
            name="testing",
            environment_type=EnvironmentType.TESTING,
            allowed_tags={"testing", "safe", "mock"},
            blocked_tags={"production-only", "dangerous"},
            description="Testing environment with mock and safe tools"
        )
        self._profiles["testing"] = test_profile
    
    def create_profile(self, profile: EnvironmentProfile):
        """创建环境配置文件"""
        self._profiles[profile.name] = profile
        self._save_profile(profile)
        logger.info(f"Created environment profile: {profile.name}")
    
    def load_profile(self, profile_name: str) -> Optional[EnvironmentProfile]:
        """加载环境配置文件"""
        if profile_name in self._profiles:
            return self._profiles[profile_name]
        
        # 尝试从文件加载
        profile_file = self.config_dir / f"{profile_name}.json"
        if profile_file.exists():
            try:
                with open(profile_file, 'r', encoding='utf-8') as f:
                    data = json.load(f)
                    profile = self._dict_to_profile(data)
                    self._profiles[profile_name] = profile
                    return profile
            except Exception as e:
                logger.error(f"Failed to load profile {profile_name}: {e}")
        
        return None
    
    def activate_profile(self, profile_name: str) -> bool:
        """激活环境配置文件"""
        profile = self.load_profile(profile_name)
        if profile:
            self._current_profile = profile_name
            logger.info(f"Activated environment profile: {profile_name}")
            return True
        else:
            logger.error(f"Profile {profile_name} not found")
            return False
    
    def get_current_profile(self) -> Optional[EnvironmentProfile]:
        """获取当前环境配置"""
        if self._current_profile:
            return self._profiles.get(self._current_profile)
        return None
    
    def apply_profile_to_filter(self, component_filter: ComponentFilter, profile_name: Optional[str] = None):
        """将环境配置应用到组件过滤器"""
        profile = self._profiles.get(profile_name or self._current_profile)
        if not profile:
            logger.warning("No profile to apply")
            return
        
        # 应用组件启用/禁用覆盖
        for component_name, enabled in profile.component_overrides.items():
            component_filter.enable_component(component_name, enabled)
        
        # 根据标签禁用组件
        if profile.blocked_tags:
            for tag in profile.blocked_tags:
                components = component_filter.get_components_with_tag(tag)
                for component in components:
                    component_filter.enable_component(component.name, False)
        
        logger.info(f"Applied profile {profile.name} to component filter")
    
    def list_profiles(self) -> List[str]:
        """列出所有环境配置文件"""
        return list(self._profiles.keys())
    
    def _save_profile(self, profile: EnvironmentProfile):
        """保存环境配置文件"""
        profile_file = self.config_dir / f"{profile.name}.json"
        try:
            with open(profile_file, 'w', encoding='utf-8') as f:
                json.dump(self._profile_to_dict(profile), f, indent=2, ensure_ascii=False)
        except Exception as e:
            logger.error(f"Failed to save profile {profile.name}: {e}")
    
    def _profile_to_dict(self, profile: EnvironmentProfile) -> Dict[str, Any]:
        """将配置文件转换为字典"""
        return {
            "name": profile.name,
            "environment_type": profile.environment_type.value,
            "allowed_tags": list(profile.allowed_tags),
            "blocked_tags": list(profile.blocked_tags),
            "allowed_components": list(profile.allowed_components),
            "blocked_components": list(profile.blocked_components),
            "component_overrides": profile.component_overrides,
            "description": profile.description
        }
    
    def _dict_to_profile(self, data: Dict[str, Any]) -> EnvironmentProfile:
        """将字典转换为配置文件"""
        return EnvironmentProfile(
            name=data["name"],
            environment_type=EnvironmentType(data["environment_type"]),
            allowed_tags=set(data.get("allowed_tags", [])),
            blocked_tags=set(data.get("blocked_tags", [])),
            allowed_components=set(data.get("allowed_components", [])),
            blocked_components=set(data.get("blocked_components", [])),
            component_overrides=data.get("component_overrides", {}),
            description=data.get("description")
        )

class ComponentControlManager:
    """组件控制管理器"""
    
    def __init__(self):
        self.filter = ComponentFilter()
        self.environment_manager = EnvironmentManager()
    
    def register_tool(self, name: str, service_name: str, tags: List[str] = None, **metadata):
        """注册工具组件"""
        component = ComponentInfo(
            name=name,
            component_type=ComponentType.TOOL,
            tags=set(tags or []),
            service_name=service_name,
            metadata=metadata
        )
        self.filter.register_component(component)
    
    def get_available_tools(self, environment: Optional[str] = None, tags: List[str] = None) -> List[ComponentInfo]:
        """获取可用工具（考虑环境和标签过滤）"""
        if environment:
            self.environment_manager.activate_profile(environment)
            self.environment_manager.apply_profile_to_filter(self.filter, environment)
        
        if tags:
            return self.filter.filter_by_tags(include_tags=tags)
        else:
            return self.filter.filter_by_type(ComponentType.TOOL)
    
    def create_custom_environment(self, name: str, allowed_tags: List[str], blocked_tags: List[str] = None):
        """创建自定义环境"""
        profile = EnvironmentProfile(
            name=name,
            environment_type=EnvironmentType.CUSTOM,
            allowed_tags=set(allowed_tags),
            blocked_tags=set(blocked_tags or []),
            description=f"Custom environment: {name}"
        )
        self.environment_manager.create_profile(profile)
    
    def switch_environment(self, environment_name: str) -> bool:
        """切换环境"""
        return self.environment_manager.activate_profile(environment_name)

# 全局实例
_global_component_manager = None

def get_component_manager() -> ComponentControlManager:
    """获取全局组件控制管理器"""
    global _global_component_manager
    if _global_component_manager is None:
        _global_component_manager = ComponentControlManager()
    return _global_component_manager
