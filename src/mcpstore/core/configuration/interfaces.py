"""
配置系统接口定义
通过接口抽象化解决循环导入，实现依赖反转
"""

from abc import ABC, abstractmethod
from dataclasses import dataclass
from enum import Enum
from typing import Dict, Any, Optional, List, Set


class ConfigSource(Enum):
    """配置源类型"""
    DEFAULT = "default"
    TOML_FILE = "toml_file"
    KV_STORAGE = "kv_storage"
    ENVIRONMENT = "environment"


@dataclass
class ConfigItem:
    """配置项"""
    key: str
    value: Any
    source: ConfigSource
    metadata: Optional[Dict[str, Any]] = None


@dataclass
class ConfigValidationRule:
    """配置验证规则"""
    required: bool = False
    min_value: Optional[float] = None
    max_value: Optional[float] = None
    allowed_values: Optional[List[Any]] = None
    data_type: Optional[type] = None


class IConfigProvider(ABC):
    """配置提供者接口"""

    @abstractmethod
    async def get_config(self, key: str, default: Any = None) -> Any:
        """获取配置值"""
        pass

    @abstractmethod
    async def set_config(self, key: str, value: Any, persist: bool = True) -> bool:
        """设置配置值"""
        pass

    @abstractmethod
    async def get_config_with_source(self, key: str) -> Optional[ConfigItem]:
        """获取配置项和源信息"""
        pass

    @abstractmethod
    async def list_configs(self, pattern: str = "*") -> List[ConfigItem]:
        """列出配置项"""
        pass

    @abstractmethod
    async def validate_config(self, key: str, value: Any) -> tuple[bool, str]:
        """验证配置值"""
        pass


class IConfigValidator(ABC):
    """配置验证器接口"""

    @abstractmethod
    def get_validation_rule(self, key: str) -> Optional[ConfigValidationRule]:
        """获取验证规则"""
        pass

    @abstractmethod
    def validate_value(self, key: str, value: Any) -> tuple[bool, str]:
        """验证值"""
        pass

    @abstractmethod
    def get_supported_keys(self) -> Set[str]:
        """获取支持的配置键"""
        pass


class IConfigPersistence(ABC):
    """配置持久化接口"""

    @abstractmethod
    async def load_config(self) -> Dict[str, Any]:
        """加载配置"""
        pass

    @abstractmethod
    async def save_config(self, config: Dict[str, Any]) -> bool:
        """保存配置"""
        pass

    @abstractmethod
    async def backup_config(self) -> bool:
        """备份配置"""
        pass

    @abstractmethod
    async def restore_config(self) -> bool:
        """恢复配置"""
        pass


class IConfigSnapshot(ABC):
    """配置快照接口"""

    @abstractmethod
    async def create_snapshot(self) -> Dict[str, Any]:
        """创建快照"""
        pass

    @abstractmethod
    async def get_snapshot(self, snapshot_id: str) -> Dict[str, Any]:
        """获取快照"""
        pass

    @abstractmethod
    async def list_snapshots(self) -> List[Dict[str, Any]]:
        """列出快照"""
        pass


class IConfigMetadata(ABC):
    """配置元数据接口"""

    @abstractmethod
    def get_metadata(self, key: str) -> Optional[Dict[str, Any]]:
        """获取配置元数据"""
        pass

    @abstractmethod
    def get_all_metadata(self) -> Dict[str, Dict[str, Any]]:
        """获取所有元数据"""
        pass

    @abstractmethod
    def get_dynamic_keys(self) -> Set[str]:
        """获取动态配置键"""
        pass


class IConfigFactory(ABC):
    """配置工厂接口"""

    @abstractmethod
    async def create_config_provider(self) -> IConfigProvider:
        """创建配置提供者"""
        pass

    @abstractmethod
    async def create_validator(self) -> IConfigValidator:
        """创建验证器"""
        pass

    @abstractmethod
    async def create_persistence(self) -> IConfigPersistence:
        """创建持久化"""
        pass


class IConfigService(IConfigProvider, IConfigMetadata):
    """配置服务组合接口"""

    @abstractmethod
    async def get_all_configs(self) -> Dict[str, Any]:
        """获取所有配置"""
        pass

    @abstractmethod
    async def update_configs(self, updates: Dict[str, Any]) -> Dict[str, bool]:
        """批量更新配置"""
        pass

    @abstractmethod
    async def reload_config(self) -> bool:
        """重新加载配置"""
        pass

    @abstractmethod
    async def export_config(self, format: str = "json") -> str:
        """导出配置"""
        pass


# 配置类型接口
class IContentUpdateConfig(ABC):
    """内容更新配置接口"""

    @abstractmethod
    def get_tools_update_interval(self) -> float:
        pass

    @abstractmethod
    def is_auto_update_enabled(self) -> bool:
        pass


class IServiceLifecycleConfig(ABC):
    """服务生命周期配置接口"""

    @abstractmethod
    def get_warning_threshold(self) -> int:
        pass

    @abstractmethod
    def get_reconnect_attempts(self) -> int:
        pass


class ICacheConfig(ABC):
    """缓存配置接口"""

    @abstractmethod
    def get_cache_type(self) -> str:
        pass

    @abstractmethod
    def get_cache_settings(self) -> Dict[str, Any]:
        pass


class IServerConfig(ABC):
    """服务器配置接口"""

    @abstractmethod
    def get_server_settings(self) -> Dict[str, Any]:
        pass

    @abstractmethod
    def get_port(self) -> int:
        pass


# 抽象基类实现
class BaseConfigProvider(IConfigProvider):
    """配置提供者基类"""

    def __init__(self):
        self._validators: List[IConfigValidator] = []

    def add_validator(self, validator: IConfigValidator):
        self._validators.append(validator)

    async def validate_config(self, key: str, value: Any) -> tuple[bool, str]:
        for validator in self._validators:
            is_valid, message = validator.validate_value(key, value)
            if not is_valid:
                return False, message
        return True, ""


class BaseConfigValidator(IConfigValidator):
    """配置验证器基类"""

    def __init__(self):
        self._rules: Dict[str, ConfigValidationRule] = {}

    def add_rule(self, key: str, rule: ConfigValidationRule):
        self._rules[key] = rule

    def get_validation_rule(self, key: str) -> Optional[ConfigValidationRule]:
        return self._rules.get(key)

    def validate_value(self, key: str, value: Any) -> tuple[bool, str]:
        rule = self.get_validation_rule(key)
        if not rule:
            return True, ""

        # 类型检查
        if rule.data_type and not isinstance(value, rule.data_type):
            return False, f"Expected type {rule.data_type.__name__}, got {type(value).__name__}"

        # 必填检查
        if rule.required and value is None:
            return False, "This configuration is required"

        # 范围检查
        if rule.min_value is not None and value < rule.min_value:
            return False, f"Value must be >= {rule.min_value}"

        if rule.max_value is not None and value > rule.max_value:
            return False, f"Value must be <= {rule.max_value}"

        # 枚举值检查
        if rule.allowed_values and value not in rule.allowed_values:
            return False, f"Value must be one of {rule.allowed_values}"

        return True, ""


# 工厂接口实现
class ConfigFactoryBase(IConfigFactory):
    """配置工厂基类"""

    def __init__(self, container=None):
        self.container = container

    async def create_config_service(self) -> IConfigService:
        provider = await self.create_config_provider()
        validator = await self.create_validator()

        # 组合服务
        service = CompositeConfigService(provider, validator)
        return service

    async def create_validator(self) -> IConfigValidator:
        validator = BaseConfigValidator()
        # 添加验证规则
        self._configure_validator(validator)
        return validator

    def _configure_validator(self, validator: BaseConfigValidator):
        """配置验证规则 - 子类实现"""
        pass


class CompositeConfigService(IConfigService):
    """组合配置服务"""

    def __init__(self, provider: IConfigProvider, validator: IConfigValidator):
        self._provider = provider
        self._validator = validator

    async def get_config(self, key: str, default: Any = None) -> Any:
        return await self._provider.get_config(key, default)

    async def set_config(self, key: str, value: Any, persist: bool = True) -> bool:
        # 验证
        is_valid, message = await self._validator.validate_value(key, value)
        if not is_valid:
            raise ValueError(f"Invalid config {key}: {message}")

        return await self._provider.set_config(key, value, persist)

    # 委托其他方法到提供者
    async def get_config_with_source(self, key: str) -> Optional[ConfigItem]:
        return await self._provider.get_config_with_source(key)

    async def list_configs(self, pattern: str = "*") -> List[ConfigItem]:
        return await self._provider.list_configs(pattern)

    async def validate_config(self, key: str, value: Any) -> tuple[bool, str]:
        return await self._validator.validate_value(key, value)

    async def get_all_configs(self) -> Dict[str, Any]:
        configs = await self.list_configs()
        return {item.key: item.value for item in configs}

    async def update_configs(self, updates: Dict[str, Any]) -> Dict[str, bool]:
        results = {}
        for key, value in updates.items():
            try:
                results[key] = await self.set_config(key, value)
            except Exception:
                results[key] = False
        return results

    async def reload_config(self) -> bool:
        # 实现重新加载逻辑
        return True

    async def export_config(self, format: str = "json") -> str:
        # 实现导出逻辑
        configs = await self.get_all_configs()
        import json
        return json.dumps(configs, indent=2)

    def get_metadata(self, key: str) -> Optional[Dict[str, Any]]:
        return self._validator.get_validation_rule(key).__dict__ if self._validator.get_validation_rule(key) else None

    def get_all_metadata(self) -> Dict[str, Dict[str, Any]]:
        return {key: rule.__dict__ for key, rule in self._validator._rules.items()}

    def get_dynamic_keys(self) -> Set[str]:
        return set(self._validator._rules.keys())