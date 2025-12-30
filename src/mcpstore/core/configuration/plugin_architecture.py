"""
插件化配置架构
通过插件系统实现配置模块的完全解耦和热插拔
"""

import asyncio
import importlib
import inspect
import json
import logging
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path
from typing import Dict, Any, List, Type, Optional, Set, Union

import yaml

logger = logging.getLogger(__name__)


class PluginType(Enum):
    """插件类型"""
    CONFIG_PROVIDER = "config_provider"
    CONFIG_VALIDATOR = "config_validator"
    CONFIG_PERSISTENCE = "config_persistence"
    CONFIG_EXPORTER = "config_exporter"
    CONFIG_WATCHER = "config_watcher"
    CONFIG_TRANSFORMER = "config_transformer"


@dataclass
class PluginMetadata:
    """插件元数据"""
    name: str
    version: str
    description: str
    author: str
    plugin_type: PluginType
    dependencies: List[str] = field(default_factory=list)
    config_schema: Optional[Dict[str, Any]] = None
    enabled: bool = True


@dataclass
class PluginLoadResult:
    """插件加载结果"""
    success: bool
    plugin_metadata: Optional[PluginMetadata] = None
    error_message: Optional[str] = None
    plugin_instance: Optional[Any] = None


class IPlugin(ABC):
    """插件接口"""

    @property
    @abstractmethod
    def metadata(self) -> PluginMetadata:
        """插件元数据"""
        pass

    @abstractmethod
    async def initialize(self, config: Dict[str, Any]) -> bool:
        """初始化插件"""
        pass

    @abstractmethod
    async def start(self) -> bool:
        """启动插件"""
        pass

    @abstractmethod
    async def stop(self) -> bool:
        """停止插件"""
        pass

    @abstractmethod
    async def cleanup(self) -> bool:
        """清理插件资源"""
        pass


class IConfigProviderPlugin(IPlugin):
    """配置提供者插件接口"""

    @abstractmethod
    async def get_config(self, key: str, default: Any = None) -> Any:
        pass

    @abstractmethod
    async def set_config(self, key: str, value: Any, persist: bool = True) -> bool:
        pass

    @abstractmethod
    async def list_configs(self, pattern: str = "*") -> Dict[str, Any]:
        pass

    @abstractmethod
    async def reload(self) -> bool:
        pass


class IConfigValidatorPlugin(IPlugin):
    """配置验证器插件接口"""

    @abstractmethod
    async def validate_config(self, key: str, value: Any) -> tuple[bool, str]:
        pass

    @abstractmethod
    def get_validation_schema(self) -> Dict[str, Any]:
        pass

    @abstractmethod
    def get_supported_keys(self) -> Set[str]:
        pass


class IConfigPersistencePlugin(IPlugin):
    """配置持久化插件接口"""

    @abstractmethod
    async def load_config(self) -> Dict[str, Any]:
        pass

    @abstractmethod
    async def save_config(self, config: Dict[str, Any]) -> bool:
        pass

    @abstractmethod
    async def backup_config(self, backup_name: str) -> bool:
        pass

    @abstractmethod
    async def restore_config(self, backup_name: str) -> bool:
        pass


class PluginRegistry:
    """插件注册表"""

    def __init__(self):
        self._plugins: Dict[str, IPlugin] = {}
        self._plugins_by_type: Dict[PluginType, List[IPlugin]] = {}
        self._plugin_configs: Dict[str, Dict[str, Any]] = {}
        self._lock = asyncio.Lock()

    async def register_plugin(self, plugin: IPlugin, config: Dict[str, Any] = None) -> PluginLoadResult:
        """注册插件"""
        async with self._lock:
            metadata = plugin.metadata

            # 检查依赖
            if not await self._check_dependencies(metadata.dependencies):
                return PluginLoadResult(
                    success=False,
                    error_message=f"Dependencies not satisfied: {metadata.dependencies}"
                )

            try:
                # 初始化插件
                if await plugin.initialize(config or {}):
                    # 启动插件
                    if await plugin.start():
                        self._plugins[metadata.name] = plugin
                        self._plugin_configs[metadata.name] = config or {}

                        # 按类型分类
                        if metadata.plugin_type not in self._plugins_by_type:
                            self._plugins_by_type[metadata.plugin_type] = []
                        self._plugins_by_type[metadata.plugin_type].append(plugin)

                        logger.info(f"Plugin {metadata.name} registered successfully")
                        return PluginLoadResult(
                            success=True,
                            plugin_metadata=metadata,
                            plugin_instance=plugin
                        )
                    else:
                        await plugin.cleanup()
                        return PluginLoadResult(
                            success=False,
                            error_message="Plugin failed to start"
                        )
                else:
                    return PluginLoadResult(
                        success=False,
                        error_message="Plugin failed to initialize"
                    )

            except Exception as e:
                logger.error(f"Failed to register plugin {metadata.name}: {e}")
                return PluginLoadResult(
                    success=False,
                    error_message=str(e)
                )

    async def unregister_plugin(self, plugin_name: str) -> bool:
        """注销插件"""
        async with self._lock:
            if plugin_name not in self._plugins:
                return False

            plugin = self._plugins[plugin_name]
            metadata = plugin.metadata

            try:
                await plugin.stop()
                await plugin.cleanup()

                # 从注册表移除
                self._plugins.pop(plugin_name, None)
                self._plugin_configs.pop(plugin_name, None)

                # 从类型分类中移除
                if metadata.plugin_type in self._plugins_by_type:
                    self._plugins_by_type[metadata.plugin_type] = [
                        p for p in self._plugins_by_type[metadata.plugin_type]
                        if p.metadata.name != plugin_name
                    ]

                logger.info(f"Plugin {plugin_name} unregistered successfully")
                return True

            except Exception as e:
                logger.error(f"Failed to unregister plugin {plugin_name}: {e}")
                return False

    def get_plugin(self, plugin_name: str) -> Optional[IPlugin]:
        """获取插件"""
        return self._plugins.get(plugin_name)

    def get_plugins_by_type(self, plugin_type: PluginType) -> List[IPlugin]:
        """按类型获取插件"""
        return self._plugins_by_type.get(plugin_type, [])

    def list_plugins(self) -> List[PluginMetadata]:
        """列出所有插件"""
        return [plugin.metadata for plugin in self._plugins.values()]

    async def _check_dependencies(self, dependencies: List[str]) -> bool:
        """检查插件依赖"""
        for dep in dependencies:
            if dep not in self._plugins:
                return False
        return True


class PluginLoader:
    """插件加载器"""

    def __init__(self, registry: PluginRegistry):
        self.registry = registry
        self._loaded_modules: Dict[str, Any] = {}

    async def load_plugin_from_file(self, file_path: Union[str, Path],
                                   config: Dict[str, Any] = None) -> PluginLoadResult:
        """从文件加载插件"""
        file_path = Path(file_path)

        if not file_path.exists():
            return PluginLoadResult(
                success=False,
                error_message=f"Plugin file not found: {file_path}"
            )

        try:
            # 动态导入模块
            spec = importlib.util.spec_from_file_location(file_path.stem, file_path)
            module = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(module)
            self._loaded_modules[file_path.stem] = module

            # 查找插件类
            plugin_class = self._find_plugin_class(module)
            if not plugin_class:
                return PluginLoadResult(
                    success=False,
                    error_message="No plugin class found in module"
                )

            # 创建插件实例
            plugin_instance = plugin_class()

            # 验证插件接口
            if not isinstance(plugin_instance, IPlugin):
                return PluginLoadResult(
                    success=False,
                    error_message="Plugin class does not implement IPlugin interface"
                )

            # 注册插件
            return await self.registry.register_plugin(plugin_instance, config)

        except Exception as e:
            return PluginLoadResult(
                success=False,
                error_message=f"Failed to load plugin: {str(e)}"
            )

    async def load_plugin_from_module(self, module_name: str,
                                     config: Dict[str, Any] = None) -> PluginLoadResult:
        """从模块名加载插件"""
        try:
            module = importlib.import_module(module_name)
            self._loaded_modules[module_name] = module

            plugin_class = self._find_plugin_class(module)
            if not plugin_class:
                return PluginLoadResult(
                    success=False,
                    error_message="No plugin class found in module"
                )

            plugin_instance = plugin_class()
            if not isinstance(plugin_instance, IPlugin):
                return PluginLoadResult(
                    success=False,
                    error_message="Plugin class does not implement IPlugin interface"
                )

            return await self.registry.register_plugin(plugin_instance, config)

        except Exception as e:
            return PluginLoadResult(
                success=False,
                error_message=f"Failed to load plugin from module {module_name}: {str(e)}"
            )

    def _find_plugin_class(self, module) -> Optional[Type[IPlugin]]:
        """查找插件类"""
        for name, obj in inspect.getmembers(module, inspect.isclass):
            if (issubclass(obj, IPlugin) and
                obj != IPlugin and
                not inspect.isabstract(obj)):
                return obj
        return None


class PluginManager:
    """插件管理器"""

    def __init__(self, config_path: Optional[Union[str, Path]] = None):
        self.registry = PluginRegistry()
        self.loader = PluginLoader(self.registry)
        self.config_path = Path(config_path) if config_path else None
        self._config: Dict[str, Any] = {}

    async def initialize(self):
        """初始化插件管理器"""
        if self.config_path and self.config_path.exists():
            await self._load_config()

        # 加载默认插件
        await self._load_default_plugins()

        # 加载配置文件中定义的插件
        await self._load_configured_plugins()

        logger.info("Plugin manager initialized")

    async def _load_config(self):
        """加载配置文件"""
        try:
            with open(self.config_path, 'r', encoding='utf-8') as f:
                if self.config_path.suffix.lower() in ['.yaml', '.yml']:
                    self._config = yaml.safe_load(f)
                else:
                    self._config = json.load(f)
        except Exception as e:
            logger.error(f"Failed to load plugin config: {e}")
            self._config = {}

    async def _load_default_plugins(self):
        """加载默认插件"""
        # 内置插件
        default_plugins = [
            "mcpstore.core.configuration.builtin_plugins.file_config_provider",
            "mcpstore.core.configuration.builtin_plugins.env_config_provider",
            "mcpstore.core.configuration.builtin_plugins.json_config_validator",
        ]

        for plugin_module in default_plugins:
            try:
                result = await self.loader.load_plugin_from_module(plugin_module)
                if result.success:
                    logger.info(f"Default plugin {plugin_module} loaded")
                else:
                    logger.warning(f"Failed to load default plugin {plugin_module}: {result.error_message}")
            except Exception as e:
                logger.warning(f"Failed to load default plugin {plugin_module}: {e}")

    async def _load_configured_plugins(self):
        """加载配置文件中定义的插件"""
        plugins_config = self._config.get("plugins", {})
        for plugin_name, plugin_config in plugins_config.items():
            if not plugin_config.get("enabled", True):
                continue

            plugin_path = plugin_config.get("path")
            plugin_module = plugin_config.get("module")

            if plugin_path:
                result = await self.loader.load_plugin_from_file(plugin_path, plugin_config)
            elif plugin_module:
                result = await self.loader.load_plugin_from_module(plugin_module, plugin_config)
            else:
                logger.warning(f"Plugin {plugin_name} has no path or module specified")
                continue

            if result.success:
                logger.info(f"Configured plugin {plugin_name} loaded")
            else:
                logger.error(f"Failed to load configured plugin {plugin_name}: {result.error_message}")

    async def reload_plugin(self, plugin_name: str) -> bool:
        """重新加载插件"""
        plugin = self.registry.get_plugin(plugin_name)
        if not plugin:
            return False

        config = self.registry._plugin_configs.get(plugin_name, {})
        metadata = plugin.metadata

        # 先注销
        await self.registry.unregister_plugin(plugin_name)

        # 重新加载
        if plugin_name in self.registry._loaded_modules:
            # 重新导入模块
            module = self.registry._loaded_modules[plugin_name]
            importlib.reload(module)

        # 根据元数据重新加载
        # 这里需要根据插件的来源重新加载

        return True

    async def shutdown(self):
        """关闭插件管理器"""
        # 注销所有插件
        plugins = list(self.registry._plugins.keys())
        for plugin_name in plugins:
            await self.registry.unregister_plugin(plugin_name)

        logger.info("Plugin manager shutdown")


# 插件化配置管理器
class PluginBasedConfigManager:
    """基于插件的配置管理器"""

    def __init__(self, plugin_manager: Optional[PluginManager] = None):
        self.plugin_manager = plugin_manager or PluginManager()
        self._config_cache: Dict[str, Any] = {}
        self._cache_lock = asyncio.Lock()
        self._provider_priority: List[str] = []

    async def initialize(self):
        """初始化配置管理器"""
        await self.plugin_manager.initialize()

        # 确定配置提供者优先级
        await self._setup_provider_priority()

        logger.info("Plugin-based config manager initialized")

    async def _setup_provider_priority(self):
        """设置配置提供者优先级"""
        providers = self.plugin_manager.registry.get_plugins_by_type(PluginType.CONFIG_PROVIDER)

        # 按优先级排序（环境变量 > 文件 > 远程等）
        priority_order = ["env", "file", "remote", "database"]
        sorted_providers = []

        for provider_type in priority_order:
            matching = [p for p in providers if provider_type in p.metadata.name.lower()]
            sorted_providers.extend(matching)

        # 添加其他提供者
        others = [p for p in providers if p not in sorted_providers]
        sorted_providers.extend(others)

        self._provider_priority = [p.metadata.name for p in sorted_providers]

    async def get_config(self, key: str, default: Any = None) -> Any:
        """获取配置值（按提供者优先级）"""
        async with self._cache_lock:
            if key in self._config_cache:
                return self._config_cache[key]

            # 按优先级从提供者获取配置
            for provider_name in self._provider_priority:
                provider = self.plugin_manager.registry.get_plugin(provider_name)
                if isinstance(provider, IConfigProviderPlugin):
                    try:
                        value = await provider.get_config(key, None)
                        if value is not None:
                            self._config_cache[key] = value
                            return value
                    except Exception as e:
                        logger.warning(f"Provider {provider_name} failed to get config {key}: {e}")

            # 返回默认值
            if default is not None:
                self._config_cache[key] = default
            return default

    async def set_config(self, key: str, value: Any, persist: bool = True) -> bool:
        """设置配置值"""
        async with self._cache_lock:
            # 更新缓存
            old_value = self._config_cache.get(key)
            self._config_cache[key] = value

            # 通知所有提供者
            providers = self.plugin_manager.registry.get_plugins_by_type(PluginType.CONFIG_PROVIDER)
            success_count = 0

            for provider in providers:
                if isinstance(provider, IConfigProviderPlugin):
                    try:
                        if await provider.set_config(key, value, persist):
                            success_count += 1
                    except Exception as e:
                        logger.warning(f"Provider {provider.metadata.name} failed to set config {key}: {e}")

            return success_count > 0

    async def validate_config(self, key: str, value: Any) -> tuple[bool, str]:
        """验证配置值"""
        validators = self.plugin_manager.registry.get_plugins_by_type(PluginType.CONFIG_VALIDATOR)

        for validator in validators:
            if isinstance(validator, IConfigValidatorPlugin):
                try:
                    is_valid, message = await validator.validate_config(key, value)
                    if not is_valid:
                        return False, message
                except Exception as e:
                    logger.warning(f"Validator {validator.metadata.name} failed: {e}")

        return True, ""

    async def reload_config(self) -> bool:
        """重新加载配置"""
        providers = self.plugin_manager.registry.get_plugins_by_type(PluginType.CONFIG_PROVIDER)

        for provider in providers:
            if isinstance(provider, IConfigProviderPlugin):
                try:
                    await provider.reload()
                except Exception as e:
                    logger.warning(f"Provider {provider.metadata.name} failed to reload: {e}")

        # 清空缓存
        async with self._cache_lock:
            self._config_cache.clear()

        return True


# 示例插件实现
class FileConfigProviderPlugin(IConfigProviderPlugin):
    """文件配置提供者插件示例"""

    def __init__(self, config_file: str = "config.toml"):
        self.config_file = Path(config_file)
        self._config: Dict[str, Any] = {}
        self._metadata = PluginMetadata(
            name="file_config_provider",
            version="1.0.0",
            description="File-based configuration provider",
            author="MCPStore",
            plugin_type=PluginType.CONFIG_PROVIDER
        )

    @property
    def metadata(self) -> PluginMetadata:
        return self._metadata

    async def initialize(self, config: Dict[str, Any]) -> bool:
        try:
            if self.config_file.exists():
                # 根据文件扩展名选择解析器
                if self.config_file.suffix.lower() == '.json':
                    with open(self.config_file, 'r') as f:
                        self._config = json.load(f)
                elif self.config_file.suffix.lower() in ['.toml']:
                    import toml
                    with open(self.config_file, 'r') as f:
                        self._config = toml.load(f)
            return True
        except Exception as e:
            logger.error(f"Failed to initialize file config provider: {e}")
            return False

    async def start(self) -> bool:
        return True

    async def stop(self) -> bool:
        return True

    async def cleanup(self) -> bool:
        return True

    async def get_config(self, key: str, default: Any = None) -> Any:
        keys = key.split('.')
        value = self._config
        try:
            for k in keys:
                value = value[k]
            return value
        except (KeyError, TypeError):
            return default

    async def set_config(self, key: str, value: Any, persist: bool = True) -> bool:
        keys = key.split('.')
        config = self._config
        for k in keys[:-1]:
            config = config.setdefault(k, {})
        config[keys[-1]] = value

        if persist:
            return await self._save_config()
        return True

    async def list_configs(self, pattern: str = "*") -> Dict[str, Any]:
        return self._config

    async def reload(self) -> bool:
        return await self.initialize({})

    async def _save_config(self) -> bool:
        try:
            with open(self.config_file, 'w') as f:
                if self.config_file.suffix.lower() == '.json':
                    json.dump(self._config, f, indent=2)
                elif self.config_file.suffix.lower() in ['.toml']:
                    import toml
                    toml.dump(self._config, f)
            return True
        except Exception as e:
            logger.error(f"Failed to save config: {e}")
            return False


# 使用示例
async def setup_plugin_based_config():
    """设置基于插件的配置系统"""
    plugin_manager = PluginManager("plugins_config.json")
    config_manager = PluginBasedConfigManager(plugin_manager)

    await config_manager.initialize()

    # 使用配置管理器
    value = await config_manager.get_config("server.port", 8080)
    print(f"Server port: {value}")

    await config_manager.set_config("server.host", "0.0.0.0")

    return config_manager