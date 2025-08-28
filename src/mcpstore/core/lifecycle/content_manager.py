"""
服务内容管理器 - 定期更新工具、资源和提示词
负责监控和更新服务的所有内容，确保缓存与实际服务保持同步
"""

import asyncio
import logging
from datetime import datetime, timedelta
from typing import Dict, Set, Optional, List, Any, Tuple
from dataclasses import dataclass
from fastmcp import Client

from mcpstore.core.configuration.config_processor import ConfigProcessor
from mcpstore.core.models.service import ServiceConnectionState

logger = logging.getLogger(__name__)


@dataclass
class ServiceContentSnapshot:
    """服务内容快照"""
    service_name: str
    agent_id: str
    tools_count: int
    tools_hash: str  # 工具列表的哈希值，用于快速比较
    resources_count: int = 0  # 预留：资源数量
    resources_hash: str = ""  # 预留：资源哈希
    prompts_count: int = 0    # 预留：提示词数量
    prompts_hash: str = ""    # 预留：提示词哈希
    last_updated: datetime = None
    
    def __post_init__(self):
        if self.last_updated is None:
            self.last_updated = datetime.now()


@dataclass
class ContentUpdateConfig:
    """内容更新配置"""
    # 更新间隔
    tools_update_interval: float = 300.0      # 工具更新间隔（5分钟）
    resources_update_interval: float = 600.0  # 资源更新间隔（10分钟）
    prompts_update_interval: float = 600.0    # 提示词更新间隔（10分钟）
    
    # 批量处理配置
    max_concurrent_updates: int = 3           # 最大并发更新数
    update_timeout: float = 30.0              # 单次更新超时（秒）
    
    # 错误处理
    max_consecutive_failures: int = 3         # 最大连续失败次数
    failure_backoff_multiplier: float = 2.0  # 失败退避倍数


class ServiceContentManager:
    """服务内容管理器"""
    
    def __init__(self, orchestrator):
        self.orchestrator = orchestrator
        self.registry = orchestrator.registry
        self.lifecycle_manager = orchestrator.lifecycle_manager
        self.config = ContentUpdateConfig()

        # 对齐全局监控配置的工具更新时间间隔（如配置存在则覆盖默认值）
        try:
            timing_config = orchestrator.config.get("timing", {}) if isinstance(getattr(orchestrator, "config", None), dict) else {}
            interval = timing_config.get("tools_update_interval_seconds")
            if isinstance(interval, (int, float)) and interval > 0:
                self.config.tools_update_interval = float(interval)
                logger.info(f"ServiceContentManager tools_update_interval set to {self.config.tools_update_interval}s from orchestrator config")
        except Exception as e:
            logger.debug(f"Failed to read tools_update_interval from orchestrator config: {e}")

        # 内容快照缓存：agent_id -> service_name -> snapshot
        self.content_snapshots: Dict[str, Dict[str, ServiceContentSnapshot]] = {}

        # 更新队列和状态
        self.update_queue: Set[Tuple[str, str]] = set()  # (agent_id, service_name)
        self.updating_services: Set[Tuple[str, str]] = set()  # 正在更新的服务
        
        # 失败统计：(agent_id, service_name) -> consecutive_failures
        self.failure_counts: Dict[Tuple[str, str], int] = {}
        
        # 定时任务
        self.content_update_task: Optional[asyncio.Task] = None
        self.is_running = False
        
        logger.info("ServiceContentManager initialized")
    
    async def start(self):
        """启动内容管理器"""
        if self.is_running:
            logger.warning("ServiceContentManager is already running")
            return
        
        self.is_running = True
        self.content_update_task = asyncio.create_task(self._content_update_loop())
        logger.info("ServiceContentManager started")
    
    async def stop(self):
        """停止内容管理器"""
        self.is_running = False
        if self.content_update_task and not self.content_update_task.done():
            self.content_update_task.cancel()
            try:
                # 🔧 修复：检查当前事件循环，避免循环冲突
                current_loop = asyncio.get_running_loop()
                task_loop = getattr(self.content_update_task, '_loop', None)

                if task_loop and task_loop != current_loop:
                    logger.warning("Task belongs to different event loop, skipping await")
                else:
                    # 添加超时保护，避免无限等待
                    await asyncio.wait_for(self.content_update_task, timeout=5.0)
            except asyncio.CancelledError:
                logger.debug("Content update task cancelled successfully")
            except asyncio.TimeoutError:
                logger.warning("Content update task cancellation timed out")
            except Exception as e:
                logger.warning(f"Error stopping content update task: {e}")

        # 🔧 修复：清理任务引用
        self.content_update_task = None
        logger.info("ServiceContentManager stopped")
    
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
        logger.info(f"Added service {service_name} to content monitoring (agent_id={agent_id})")
    
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
        """强制更新指定服务的内容"""
        try:
            return await self._update_service_content(agent_id, service_name)
        except Exception as e:
            logger.error(f"Failed to force update content for {service_name}: {e}")
            return False
    
    def get_service_snapshot(self, agent_id: str, service_name: str) -> Optional[ServiceContentSnapshot]:
        """获取服务内容快照"""
        return self.content_snapshots.get(agent_id, {}).get(service_name)
    
    async def _content_update_loop(self):
        """内容更新主循环"""
        consecutive_failures = 0
        max_consecutive_failures = 5
        
        while self.is_running:
            try:
                await asyncio.sleep(30)  # 每30秒检查一次
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
                
                # 指数退避延迟
                backoff_delay = min(60 * (2 ** consecutive_failures), 300)  # 最大5分钟
                await asyncio.sleep(backoff_delay)
    
    async def _process_content_updates(self):
        """处理内容更新队列"""
        if not self.update_queue:
            # 检查是否有服务需要定期更新
            await self._check_scheduled_updates()
            return
        
        # 限制并发更新数量
        available_slots = self.config.max_concurrent_updates - len(self.updating_services)
        if available_slots <= 0:
            return
        
        # 获取待更新的服务
        services_to_update = list(self.update_queue)[:available_slots]
        
        # 并发更新
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
    
    async def _check_scheduled_updates(self):
        """检查需要定期更新的服务"""
        now = datetime.now()
        
        for agent_id, services in self.content_snapshots.items():
            for service_name, snapshot in services.items():
                # 检查服务是否健康
                service_state = self.lifecycle_manager.get_service_state(agent_id, service_name)
                if service_state not in [ServiceConnectionState.HEALTHY, ServiceConnectionState.WARNING]:
                    continue
                
                # 检查是否需要更新工具
                time_since_update = (now - snapshot.last_updated).total_seconds()
                if time_since_update >= self.config.tools_update_interval:
                    self.update_queue.add((agent_id, service_name))
                    logger.debug(f"Scheduled content update for {service_name} (last updated {time_since_update:.0f}s ago)")
    
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
            # 获取服务配置
            service_config = self.orchestrator.mcp_config.get_service_config(service_name)
            if not service_config:
                logger.warning(f"No configuration found for service {service_name}")
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
        if agent_id not in self.registry.tool_cache:
            self.registry.tool_cache[agent_id] = {}
        if agent_id not in self.registry.tool_to_session_map:
            self.registry.tool_to_session_map[agent_id] = {}

        # 获取服务会话
        service_session = self.registry.sessions.get(agent_id, {}).get(service_name)
        if not service_session:
            logger.warning(f"No session found for service {service_name}")
            return

        # 清理旧的工具缓存（只清理该服务的工具）
        tools_to_remove = []
        for tool_name, session in self.registry.tool_to_session_map[agent_id].items():
            if session == service_session:
                tools_to_remove.append(tool_name)

        for tool_name in tools_to_remove:
            self.registry.tool_cache[agent_id].pop(tool_name, None)
            self.registry.tool_to_session_map[agent_id].pop(tool_name, None)

        # 添加新的工具缓存
        for tool in tools:
            # 兼容字典和对象两种格式
            if hasattr(tool, 'get'):
                # 字典格式
                tool_name = tool.get("name")
                tool_dict = tool
            else:
                # 对象格式（如FastMCP的Tool对象）
                tool_name = getattr(tool, 'name', None)
                # 将对象转换为字典格式存储
                tool_dict = {
                    'name': getattr(tool, 'name', ''),
                    'description': getattr(tool, 'description', ''),
                    'inputSchema': getattr(tool, 'inputSchema', {})
                }

            if tool_name:
                self.registry.tool_cache[agent_id][tool_name] = tool_dict
                self.registry.tool_to_session_map[agent_id][tool_name] = service_session

        logger.debug(f"Updated tool cache for {service_name}: {len(tools)} tools")
