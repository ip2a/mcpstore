"""
Service Content Manager - Periodically updates tools, resources and prompts
Responsible for monitoring and updating all service content, ensuring cache stays synchronized with actual services
"""

import asyncio
import logging
from dataclasses import dataclass
from datetime import datetime
from typing import Dict, Set, Optional, List, Any, Tuple

from fastmcp import Client

from mcpstore.config.config_dataclasses import ContentUpdateConfig
from mcpstore.core.configuration.config_processor import ConfigProcessor

logger = logging.getLogger(__name__)


@dataclass
class ServiceContentSnapshot:
    """Service content snapshot"""
    service_name: str
    agent_id: str
    tools_count: int
    tools_hash: str  # Hash value of tool list for fast comparison
    resources_count: int = 0  # Reserved: resource count
    resources_hash: str = ""  # Reserved: resource hash
    prompts_count: int = 0    # Reserved: prompt count
    prompts_hash: str = ""    # Reserved: prompt hash
    last_updated: datetime = None

    def __post_init__(self):
        if self.last_updated is None:
            self.last_updated = datetime.now()


# ContentUpdateConfig is now imported from mcpstore.config.toml_config


class ServiceContentManager:
    """服务内容管理器"""

    def __init__(self, orchestrator):
        self.orchestrator = orchestrator
        self.registry = orchestrator.registry
        self.lifecycle_manager = orchestrator.lifecycle_manager

        # 使用 MCPStoreConfig 获取内容更新配置 - 延迟导入避免循环依赖
        try:
            from mcpstore.config.toml_config import get_content_update_config_with_defaults
            self.config = get_content_update_config_with_defaults()
        except Exception as e:
            logger.warning(f"Failed to get content update config, using defaults: {e}")
            self.config = ContentUpdateConfig()
        logger.debug(f"ContentManager initialized with config from MCPStoreConfig: tools_update_interval={self.config.tools_update_interval}s")

        # 事件总线（可选）
        self.event_bus = None
        try:
            self.event_bus = getattr(getattr(orchestrator, 'store', None), 'container', None).event_bus  # type: ignore
        except Exception:
            self.event_bus = None

        # 内容快照缓存：agent_id -> service_name -> snapshot
        self.content_snapshots: Dict[str, Dict[str, ServiceContentSnapshot]] = {}

        # 更新队列和状态
        self.update_queue: Set[Tuple[str, str]] = set()  # (agent_id, service_name)
        self.updating_services: Set[Tuple[str, str]] = set()  # 正在更新的服务

        # 失败统计：(agent_id, service_name) -> consecutive_failures
        self.failure_counts: Dict[Tuple[str, str], int] = {}

        # 事件驱动处理任务
        self._process_task: Optional[asyncio.Task] = None

        # 订阅事件：仅在 HEALTHY/DEGRADED 触发更新
        try:
            if self.event_bus is not None:
                from mcpstore.core.events.service_events import ServiceStateChanged
                async def _on_state_changed(event: 'ServiceStateChanged'):
                    try:
                        if event.new_state in ("healthy", "degraded"):
                            self.update_queue.add((event.agent_id, event.service_name))
                            self._schedule_queue_processing()
                        elif event.new_state in ("disconnected", "disconnected", "disconnected"):
                            # 终止/不可达时清理队列，避免无效更新
                            self.update_queue.discard((event.agent_id, event.service_name))
                            self.updating_services.discard((event.agent_id, event.service_name))
                    except Exception as e:
                        logger.debug(f"ContentManager state-change handler error: {e}")
                self.event_bus.subscribe(ServiceStateChanged, _on_state_changed, priority=10)
        except Exception as e:
            logger.debug(f"EventBus subscription skipped: {e}")

        logger.debug("ServiceContentManager initialized")

    def _schedule_queue_processing(self):
        """调度一次队列处理（去抖：避免重复并发）"""
        try:
            if self._process_task is None or self._process_task.done():
                self._process_task = asyncio.create_task(self._drain_queue())
        except Exception as e:
            logger.debug(f"Failed to schedule queue processing: {e}")

    async def _drain_queue(self):
        """持续处理队列直到清空（避免阻塞事件总线）"""
        try:
            # 循环直到队列清空
            while self.update_queue:
                await self._process_content_updates()
                await asyncio.sleep(0)
        except Exception as e:
            logger.debug(f"Drain queue error: {e}")

    async def start(self):
        """启动内容管理器（事件驱动，无主循环）"""
        logger.debug("ServiceContentManager started (event-driven mode; no loop)")

    async def stop(self):
        """停止内容管理器（事件驱动，无主循环）"""
        logger.debug("ServiceContentManager stopped (event-driven mode; no loop)")

    def add_service_for_monitoring(self, agent_id: str, service_name: str):
        """添加服务到内容监控"""
        if agent_id not in self.content_snapshots:
            self.content_snapshots[agent_id] = {}

        # 创建初始快照（工具数量为0，等待首次更新）
        self.content_snapshots[agent_id][service_name] = ServiceContentSnapshot(
            service_name=service_name,
            agent_id=agent_id,
            tools_count=0,
            tools_hash="",
            last_updated=datetime.now()
        )

        # 添加到更新队列
        self.update_queue.add((agent_id, service_name))
        self._schedule_queue_processing()
        logger.debug(f"Added service {service_name} to content monitoring (agent_id={agent_id})")

    def remove_service_from_monitoring(self, agent_id: str, service_name: str):
        """从内容监控中移除服务"""
        if agent_id in self.content_snapshots:
            self.content_snapshots[agent_id].pop(service_name, None)
            if not self.content_snapshots[agent_id]:
                del self.content_snapshots[agent_id]

        self.update_queue.discard((agent_id, service_name))
        self.updating_services.discard((agent_id, service_name))
        self.failure_counts.pop((agent_id, service_name), None)

        logger.info(f"Removed service {service_name} from content monitoring (agent_id={agent_id})")

    async def force_update_service_content(self, agent_id: str, service_name: str) -> bool:
        """Force update content of specified service"""
        try:
            return await self._update_service_content(agent_id, service_name)
        except Exception as e:
            logger.error(f"Failed to force update content for {service_name}: {e}")
            return False

    def get_service_snapshot(self, agent_id: str, service_name: str) -> Optional[ServiceContentSnapshot]:
        """Get service content snapshot"""
        return self.content_snapshots.get(agent_id, {}).get(service_name)

    async def _content_update_loop(self):
        """Content update main loop"""
        consecutive_failures = 0
        max_consecutive_failures = 5

        while self.is_running:
            try:
                await asyncio.sleep(30)  # Check every 30 seconds
                await self._process_content_updates()
                consecutive_failures = 0

            except asyncio.CancelledError:
                logger.info("Content update loop cancelled")
                break
            except Exception as e:
                consecutive_failures += 1
                logger.error(f"Content update loop error (failure {consecutive_failures}/{max_consecutive_failures}): {e}")

                if consecutive_failures >= max_consecutive_failures:
                    logger.critical("Too many consecutive content update failures, stopping loop")
                    break

                # Exponential backoff delay
                backoff_delay = min(60 * (2 ** consecutive_failures), 300)  # Max 5 minutes
                await asyncio.sleep(backoff_delay)

    async def _process_content_updates(self):
        """Process content update queue"""
        if not self.update_queue:
            # Event-driven: return directly if no pending tasks
            return

        # Limit concurrent update count
        available_slots = self.config.max_concurrent_updates - len(self.updating_services)
        if available_slots <= 0:
            return

        # Get services to be updated
        services_to_update = list(self.update_queue)[:available_slots]

        # Concurrent updates
        update_tasks = []
        for agent_id, service_name in services_to_update:
            self.update_queue.discard((agent_id, service_name))
            self.updating_services.add((agent_id, service_name))

            task = asyncio.create_task(
                self._update_service_content_with_cleanup(agent_id, service_name)
            )
            update_tasks.append(task)

        if update_tasks:
            await asyncio.gather(*update_tasks, return_exceptions=True)


    async def _get_service_config_from_pykv_async(self, agent_id: str, service_name: str) -> Optional[Dict[str, Any]]:
        """
        从 pykv 获取服务配置

        遵循 "pykv 唯一真相数据源" 原则，从 ServiceEntity 中读取配置。

        Args:
            agent_id: Agent ID
            service_name: 服务名称

        Returns:
            服务配置字典，如果不存在返回 None
        """
        try:
            # 生成服务全局名称
            from mcpstore.core.cache.naming_service import NamingService
            global_name = NamingService.generate_service_global_name(service_name, agent_id)

            # 从 pykv 获取服务实体
            # 使用 ServiceRegistry 的 _cache_service_manager（ServiceEntityManager）
            service_entity_manager = self.registry._cache_service_manager
            service_entity = await service_entity_manager.get_service(global_name)

            if service_entity is None:
                logger.debug(f"Service entity not found in pykv: {global_name}")
                return None

            # 返回服务配置（ServiceEntity 是 dataclass，直接访问 config 属性）
            config = service_entity.config
            if not config:
                logger.debug(f"Service entity config is empty: {global_name}")
                return None

            logger.debug(f"Successfully retrieved service config from pykv: {global_name}")
            return config

        except Exception as e:
            logger.error(f"Failed to get service config from pykv: agent_id={agent_id}, service_name={service_name}, error={e}")
            raise

    async def _update_service_content_with_cleanup(self, agent_id: str, service_name: str):
        """带清理的服务内容更新"""
        try:
            success = await self._update_service_content(agent_id, service_name)
            if success:
                # 重置失败计数
                self.failure_counts.pop((agent_id, service_name), None)
            else:
                # 增加失败计数
                key = (agent_id, service_name)
                self.failure_counts[key] = self.failure_counts.get(key, 0) + 1
        finally:
            self.updating_services.discard((agent_id, service_name))

    async def _update_service_content(self, agent_id: str, service_name: str) -> bool:
        """更新服务内容（工具、资源、提示词）"""
        try:
            # 从 pykv 获取服务配置（遵循 pykv 唯一真相数据源原则）
            service_config = await self._get_service_config_from_pykv_async(agent_id, service_name)
            if not service_config:
                logger.warning(f"Service config not found in pykv: agent_id={agent_id}, service_name={service_name}")
                return False

            # 创建临时客户端
            user_config = {"mcpServers": {service_name: service_config}}
            fastmcp_config = ConfigProcessor.process_user_config_for_fastmcp(user_config)

            if service_name not in fastmcp_config.get("mcpServers", {}):
                logger.warning(f"Service {service_name} not found in processed config")
                return False

            client = Client(fastmcp_config)

            async with asyncio.timeout(self.config.update_timeout):
                async with client:
                    # 获取工具列表
                    tools = await client.list_tools()
                    tools_count = len(tools)
                    tools_hash = self._calculate_tools_hash(tools)

                    # 检查是否有变化
                    current_snapshot = self.get_service_snapshot(agent_id, service_name)
                    if current_snapshot and current_snapshot.tools_hash == tools_hash:
                        # 没有变化，只更新时间戳
                        current_snapshot.last_updated = datetime.now()
                        logger.debug(f"No content changes detected for {service_name}")
                        return True

                    # 有变化，更新缓存
                    await self._update_service_tools_cache(agent_id, service_name, tools)

                    # 更新快照
                    new_snapshot = ServiceContentSnapshot(
                        service_name=service_name,
                        agent_id=agent_id,
                        tools_count=tools_count,
                        tools_hash=tools_hash,
                        last_updated=datetime.now()
                    )

                    if agent_id not in self.content_snapshots:
                        self.content_snapshots[agent_id] = {}
                    self.content_snapshots[agent_id][service_name] = new_snapshot

                    logger.info(f"Updated content for {service_name}: {tools_count} tools")
                    return True

        except asyncio.TimeoutError:
            logger.warning(f"Content update timeout for {service_name}")
            return False
        except Exception as e:
            logger.error(f"Failed to update content for {service_name}: {e}")
            return False

    def _calculate_tools_hash(self, tools: List[Any]) -> str:
        """计算工具列表的哈希值"""
        import hashlib

        # 提取关键信息用于哈希计算
        tool_signatures = []
        for tool in tools:
            # 兼容字典和对象两种格式
            if hasattr(tool, 'get'):
                # 字典格式
                name = tool.get('name', '')
                description = tool.get('description', '')
            else:
                # 对象格式（如FastMCP的Tool对象）
                name = getattr(tool, 'name', '')
                description = getattr(tool, 'description', '')

            signature = f"{name}:{description}"
            tool_signatures.append(signature)

        # 排序确保一致性
        tool_signatures.sort()
        content = "|".join(tool_signatures)

        return hashlib.md5(content.encode()).hexdigest()

    async def _update_service_tools_cache(self, agent_id: str, service_name: str, tools: List[Any]):
        """更新服务工具缓存"""
        # 获取服务会话
        service_session = self.registry.get_session(agent_id, service_name)
        if not service_session:
            logger.warning(f"No session found for service {service_name}")
            return

        # 统一通过 Registry API 更新工具缓存，避免直访内部字典
        # - 先清理该服务的工具缓存
        # - 再批量注册当前工具定义
        processed_tools: List[Tuple[str, Dict[str, Any]]] = []
        for tool in tools:
            if hasattr(tool, 'get'):
                tool_name = tool.get("name")
                tool_dict = dict(tool)
            else:
                tool_name = getattr(tool, 'name', None)
                tool_dict = {
                    'name': getattr(tool, 'name', ''),
                    'description': getattr(tool, 'description', ''),
                    'inputSchema': getattr(tool, 'inputSchema', {})
                }
            if not tool_name:
                continue
            # 规范化为 function 形式，便于后续 full 模式与硬映射
            if "function" not in tool_dict:
                tool_def = {"type": "function", "function": tool_dict}
            else:
                tool_def = tool_dict
            processed_tools.append((tool_name, tool_def))

        # 加锁执行原子更新，使用异步版本避免事件循环冲突
        locks_owner = getattr(self.orchestrator, 'store', None)
        agent_locks = getattr(locks_owner, 'agent_locks', None) if locks_owner else None
        if agent_locks:
            async with agent_locks.write(agent_id):
                self.registry.clear_service_tools_only(agent_id, service_name)
                await self.registry.add_service_async(agent_id=agent_id, name=service_name, session=service_session, tools=processed_tools, preserve_mappings=True)
        else:
            self.registry.clear_service_tools_only(agent_id, service_name)
            await self.registry.add_service_async(agent_id=agent_id, name=service_name, session=service_session, tools=processed_tools, preserve_mappings=True)

        logger.debug(f"Updated tool cache for {service_name}: {len(processed_tools)} tools")
