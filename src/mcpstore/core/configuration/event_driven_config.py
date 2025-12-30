"""
事件驱动配置架构
通过事件系统解耦配置模块之间的直接依赖
"""

import asyncio
import logging
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from typing import Dict, Any, List, Optional

logger = logging.getLogger(__name__)


class ConfigEventType(Enum):
    """配置事件类型"""
    CONFIG_CHANGED = "config_changed"
    CONFIG_LOADED = "config_loaded"
    CONFIG_SAVED = "config_saved"
    CONFIG_VALIDATED = "config_validated"
    CONFIG_ERROR = "config_error"
    SNAPSHOT_CREATED = "snapshot_created"
    SERVICE_REGISTERED = "service_registered"


@dataclass
class ConfigEvent:
    """配置事件"""
    event_type: ConfigEventType
    key: Optional[str] = None
    old_value: Any = None
    new_value: Any = None
    source: Optional[str] = None
    timestamp: datetime = field(default_factory=datetime.now)
    metadata: Dict[str, Any] = field(default_factory=dict)
    correlation_id: Optional[str] = None


class IEventHandler(ABC):
    """事件处理器接口"""

    @abstractmethod
    async def handle(self, event: ConfigEvent) -> None:
        """处理事件"""
        pass

    @property
    @abstractmethod
    def handled_events(self) -> List[ConfigEventType]:
        """处理的事件类型"""
        pass


class EventBus:
    """配置事件总线"""

    def __init__(self):
        self._handlers: Dict[ConfigEventType, List[IEventHandler]] = {}
        self._global_handlers: List[IEventHandler] = []
        self._lock = asyncio.Lock()
        self._event_history: List[ConfigEvent] = []
        self._max_history = 1000

    async def subscribe(self, event_type: ConfigEventType, handler: IEventHandler):
        """订阅事件"""
        async with self._lock:
            if event_type not in self._handlers:
                self._handlers[event_type] = []
            self._handlers[event_type].append(handler)
            logger.debug(f"Handler subscribed to {event_type}")

    async def subscribe_all(self, handler: IEventHandler):
        """订阅所有事件"""
        async with self._lock:
            self._global_handlers.append(handler)
            logger.debug("Global handler subscribed")

    async def unsubscribe(self, event_type: ConfigEventType, handler: IEventHandler):
        """取消订阅"""
        async with self._lock:
            if event_type in self._handlers:
                try:
                    self._handlers[event_type].remove(handler)
                    logger.debug(f"Handler unsubscribed from {event_type}")
                except ValueError:
                    pass

    async def publish(self, event: ConfigEvent) -> None:
        """发布事件"""
        async with self._lock:
            # 记录事件历史
            self._event_history.append(event)
            if len(self._event_history) > self._max_history:
                self._event_history.pop(0)

        # 分发事件
        await self._dispatch_event(event)

    async def _dispatch_event(self, event: ConfigEvent) -> None:
        """分发事件到处理器"""
        handlers = []

        # 获取特定事件类型的处理器
        if event.event_type in self._handlers:
            handlers.extend(self._handlers[event.event_type])

        # 添加全局处理器
        handlers.extend(self._global_handlers)

        # 并发处理事件
        tasks = []
        for handler in handlers:
            if not handler.handled_events or event.event_type in handler.handled_events:
                task = asyncio.create_task(self._safe_handle(handler, event))
                tasks.append(task)

        if tasks:
            await asyncio.gather(*tasks, return_exceptions=True)

    async def _safe_handle(self, handler: IEventHandler, event: ConfigEvent):
        """安全处理事件"""
        try:
            await handler.handle(event)
        except Exception as e:
            logger.error(f"Event handler error: {e}", exc_info=True)

    def get_event_history(self, event_type: Optional[ConfigEventType] = None,
                         since: Optional[datetime] = None) -> List[ConfigEvent]:
        """获取事件历史"""
        history = self._event_history

        if event_type:
            history = [e for e in history if e.event_type == event_type]

        if since:
            history = [e for e in history if e.timestamp >= since]

        return history


# 全局事件总线
_global_event_bus: Optional[EventBus] = None


def get_event_bus() -> EventBus:
    """获取全局事件总线"""
    global _global_event_bus
    if _global_event_bus is None:
        _global_event_bus = EventBus()
    return _global_event_bus


class EventDrivenConfigProvider:
    """事件驱动的配置提供者"""

    def __init__(self, event_bus: Optional[EventBus] = None):
        self.event_bus = event_bus or get_event_bus()
        self._config: Dict[str, Any] = {}
        self._sources: Dict[str, str] = {}
        self._lock = asyncio.Lock()

    async def get_config(self, key: str, default: Any = None) -> Any:
        """获取配置"""
        async with self._lock:
            value = self._config.get(key, default)
            source = self._sources.get(key, "unknown")

            # 发布配置访问事件（可选）
            # await self.event_bus.publish(ConfigEvent(
            #     event_type=ConfigEventType.CONFIG_ACCESSED,
            #     key=key,
            #     new_value=value,
            #     source=source
            # ))

            return value

    async def set_config(self, key: str, value: Any, persist: bool = True,
                        source: str = "runtime") -> bool:
        """设置配置"""
        async with self._lock:
            old_value = self._config.get(key)
            old_source = self._sources.get(key)

            # 设置值
            self._config[key] = value
            self._sources[key] = source

            # 发布变更事件
            await self.event_bus.publish(ConfigEvent(
                event_type=ConfigEventType.CONFIG_CHANGED,
                key=key,
                old_value=old_value,
                new_value=value,
                source=source,
                metadata={"persist": persist}
            ))

            return True

    async def load_config_from_dict(self, config: Dict[str, Any],
                                   source: str = "loaded") -> None:
        """从字典加载配置"""
        async with self._lock:
            for key, value in config.items():
                old_value = self._config.get(key)
                self._config[key] = value
                self._sources[key] = source

            await self.event_bus.publish(ConfigEvent(
                event_type=ConfigEventType.CONFIG_LOADED,
                source=source,
                metadata={"keys_loaded": list(config.keys())}
            ))


# 配置事件处理器
class ConfigValidationHandler(IEventHandler):
    """配置验证处理器"""

    def __init__(self, validator=None):
        self.validator = validator

    @property
    def handled_events(self) -> List[ConfigEventType]:
        return [ConfigEventType.CONFIG_CHANGED]

    async def handle(self, event: ConfigEvent) -> None:
        """处理配置变更验证"""
        if self.validator and event.key:
            is_valid, message = await self.validator.validate_config(event.key, event.new_value)
            if not is_valid:
                await get_event_bus().publish(ConfigEvent(
                    event_type=ConfigEventType.CONFIG_ERROR,
                    key=event.key,
                    new_value=event.new_value,
                    metadata={"validation_error": message, "correlation_id": event.correlation_id}
                ))
            else:
                await get_event_bus().publish(ConfigEvent(
                    event_type=ConfigEventType.CONFIG_VALIDATED,
                    key=event.key,
                    new_value=event.new_value,
                    correlation_id=event.correlation_id
                ))


class ConfigPersistenceHandler(IEventHandler):
    """配置持久化处理器"""

    def __init__(self, persistence_service=None):
        self.persistence_service = persistence_service

    @property
    def handled_events(self) -> List[ConfigEventType]:
        return [ConfigEventType.CONFIG_CHANGED, ConfigEventType.CONFIG_SAVED]

    async def handle(self, event: ConfigEvent) -> None:
        """处理配置持久化"""
        if self.persistence_service and event.metadata.get("persist", True):
            if event.event_type == ConfigEventType.CONFIG_CHANGED:
                # 异步持久化
                asyncio.create_task(self._persist_config(event))
            elif event.event_type == ConfigEventType.CONFIG_SAVED:
                logger.info(f"Config saved to {event.source}")

    async def _persist_config(self, event: ConfigEvent):
        """持久化配置"""
        try:
            if event.key is not None:
                await self.persistence_service.save_config_key(event.key, event.new_value)
            else:
                # 批量保存
                await self.persistence_service.save_all_config()
        except Exception as e:
            logger.error(f"Failed to persist config: {e}")


class ConfigSnapshotHandler(IEventHandler):
    """配置快照处理器"""

    def __init__(self, snapshot_service=None):
        self.snapshot_service = snapshot_service

    @property
    def handled_events(self) -> List[ConfigEventType]:
        return [ConfigEventType.CONFIG_CHANGED, ConfigEventType.SNAPSHOT_CREATED]

    async def handle(self, event: ConfigEvent) -> None:
        """处理配置快照"""
        if self.snapshot_service and event.event_type == ConfigEventType.CONFIG_CHANGED:
            # 重要配置变更时创建快照
            if self._should_create_snapshot(event):
                await self.snapshot_service.create_snapshot(
                    reason=f"Config change: {event.key}",
                    trigger_event=event
                )

    def _should_create_snapshot(self, event: ConfigEvent) -> bool:
        """判断是否应该创建快照"""
        # 关键配置变更
        critical_keys = ["server.port", "database.url", "security.enabled"]
        return event.key in critical_keys


class ConfigChangeLogger(IEventHandler):
    """配置变更日志处理器"""

    @property
    def handled_events(self) -> List[ConfigEventType]:
        return [ConfigEventType.CONFIG_CHANGED, ConfigEventType.CONFIG_ERROR]

    async def handle(self, event: ConfigEvent) -> None:
        """记录配置变更日志"""
        if event.event_type == ConfigEventType.CONFIG_CHANGED:
            logger.info(f"Config changed: {event.key} = {event.new_value} (from {event.source})")
        elif event.event_type == ConfigEventType.CONFIG_ERROR:
            logger.error(f"Config error: {event.key} = {event.new_value} - {event.metadata.get('validation_error')}")


# 集成配置管理器
class EventDrivenConfigManager:
    """事件驱动配置管理器"""

    def __init__(self, event_bus: Optional[EventBus] = None):
        self.event_bus = event_bus or get_event_bus()
        self.provider = EventDrivenConfigProvider(self.event_bus)
        self._handlers: List[IEventHandler] = []

    async def initialize(self):
        """初始化配置管理器"""
        # 注册默认事件处理器
        validation_handler = ConfigValidationHandler()
        persistence_handler = ConfigPersistenceHandler()
        snapshot_handler = ConfigSnapshotHandler()
        logger_handler = ConfigChangeLogger()

        await self.event_bus.subscribe(ConfigEventType.CONFIG_CHANGED, validation_handler)
        await self.event_bus.subscribe(ConfigEventType.CONFIG_CHANGED, persistence_handler)
        await self.event_bus.subscribe(ConfigEventType.CONFIG_CHANGED, snapshot_handler)
        await self.event_bus.subscribe(ConfigEventType.CONFIG_CHANGED, logger_handler)

        self._handlers.extend([validation_handler, persistence_handler, snapshot_handler, logger_handler])

        logger.info("Event-driven config manager initialized")

    async def get_config(self, key: str, default: Any = None) -> Any:
        return await self.provider.get_config(key, default)

    async def set_config(self, key: str, value: Any, persist: bool = True) -> bool:
        return await self.provider.set_config(key, value, persist)

    async def load_config(self, config: Dict[str, Any], source: str = "loaded"):
        await self.provider.load_config_from_dict(config, source)

    def add_custom_handler(self, handler: IEventHandler):
        """添加自定义事件处理器"""
        self._handlers.append(handler)

    async def shutdown(self):
        """关闭配置管理器"""
        # 清理资源
        pass


# 使用示例
async def setup_event_driven_config():
    """设置事件驱动配置系统"""
    manager = EventDrivenConfigManager()
    await manager.initialize()

    # 加载初始配置
    initial_config = {
        "server.port": 8080,
        "database.url": "sqlite:///app.db",
        "logging.level": "INFO"
    }
    await manager.load_config(initial_config, "initial")

    return manager


# 自定义事件处理器示例
class ConfigChangeNotifier(IEventHandler):
    """配置变更通知器"""

    @property
    def handled_events(self) -> List[ConfigEventType]:
        return [ConfigEventType.CONFIG_CHANGED]

    async def handle(self, event: ConfigEvent) -> None:
        """发送通知"""
        if event.key and event.key.startswith("notification."):
            # 发送通知逻辑
            print(f"Notification triggered by config change: {event.key} = {event.new_value}")


# 解耦的服务注册
class ServiceRegistry:
    """通过事件系统解耦的服务注册表"""

    def __init__(self, event_bus: Optional[EventBus] = None):
        self.event_bus = event_bus or get_event_bus()
        self._services: Dict[str, Any] = {}

    async def register_service(self, name: str, service_config: Dict[str, Any]):
        """注册服务"""
        self._services[name] = service_config

        # 发布服务注册事件
        await self.event_bus.publish(ConfigEvent(
            event_type=ConfigEventType.SERVICE_REGISTERED,
            key=f"services.{name}",
            new_value=service_config,
            source="service_registry"
        ))


# 通过事件系统实现的配置模块间通信
class ConfigModuleCommunicator:
    """配置模块通信器"""

    def __init__(self, module_name: str, event_bus: Optional[EventBus] = None):
        self.module_name = module_name
        self.event_bus = event_bus or get_event_bus()
        self._pending_requests: Dict[str, asyncio.Future] = {}

    async def request_config(self, key: str, target_module: str) -> Any:
        """请求其他模块的配置"""
        correlation_id = f"{self.module_name}-{target_module}-{key}-{id(asyncio.current_task())}"

        # 创建Future等待响应
        future = asyncio.Future()
        self._pending_requests[correlation_id] = future

        # 发布请求事件
        await self.event_bus.publish(ConfigEvent(
            event_type=ConfigEventType.CONFIG_CHANGED,  # 复用现有事件类型
            key=key,
            source=self.module_name,
            correlation_id=correlation_id,
            metadata={"request_type": "get_config", "target_module": target_module}
        ))

        # 等待响应
        try:
            result = await asyncio.wait_for(future, timeout=5.0)
            return result
        except asyncio.TimeoutError:
            raise TimeoutError(f"Config request timeout: {key} from {target_module}")
        finally:
            self._pending_requests.pop(correlation_id, None)