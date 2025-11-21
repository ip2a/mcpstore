"""
MCPOrchestrator Base Module
Orchestrator core base module - contains infrastructure and lifecycle management
"""

import logging
import time
from typing import Dict, Any, Optional

from fastmcp import Client

from mcpstore.config.json_config import MCPConfig
from mcpstore.core.agents.session_manager import SessionManager
from mcpstore.core.store.client_manager import ClientManager
from mcpstore.core.integration.local_service_adapter import get_local_service_manager
from mcpstore.core.registry import ServiceRegistry
# Import mixin classes
from .monitoring_tasks import MonitoringTasksMixin
from .network_utils import NetworkUtilsMixin
from .resources_prompts import ResourcesPromptsMixin
from .service_connection import ServiceConnectionMixin
from .service_management import ServiceManagementMixin
from .standalone_config import StandaloneConfigMixin
from .tool_execution import ToolExecutionMixin

logger = logging.getLogger(__name__)

class MCPOrchestrator(
    MonitoringTasksMixin,
    ServiceConnectionMixin,
    ToolExecutionMixin,
    ServiceManagementMixin,
    ResourcesPromptsMixin,
    NetworkUtilsMixin,
    StandaloneConfigMixin
):
    """
    MCP服务编排器
    
    负责管理服务连接、工具调用和查询处理。
    """

    def __init__(self, config: Dict[str, Any], registry: ServiceRegistry, standalone_config_manager=None, client_services_path=None, agent_clients_path=None, mcp_config=None):
        """
        初始化MCP编排器

        Args:
            config: 配置字典
            registry: 服务注册表实例
            standalone_config_manager: 独立配置管理器（可选）
            client_services_path: 客户端服务配置文件路径（可选，用于数据空间）
            agent_clients_path: Agent客户端映射文件路径（可选，用于数据空间）
            mcp_config: MCPConfig实例（可选，用于数据空间）
        """
        self.config = config
        self.registry = registry
        self.clients: Dict[str, Client] = {}  # key为mcpServers的服务名
        self.global_agent_store: Optional[Client] = None
        self.global_agent_store_ctx = None  # async context manager for global_agent_store
        self.global_agent_store_config = {"mcpServers": {}}  # 中央配置
        self.agent_clients: Dict[str, Client] = {}  # agent_id -> client映射
        # 智能重连功能已集成到ServiceLifecycleManager中
        self.react_agent = None

        #  新增：独立配置管理器
        self.standalone_config_manager = standalone_config_manager

        #  新增：统一同步管理器
        self.sync_manager = None

        #  新增：ServiceContainer引用（替代 store 引用，解除循环依赖）
        self.container = None

        #  新增：Context工厂函数（用于服务注册，替代 store.for_store()）
        self._context_factory = None

        #  新增：异步同步助手（用于Resources和Prompts的同步方法）
        from mcpstore.core.utils.async_sync_helper import AsyncSyncHelper
        self._sync_helper = AsyncSyncHelper()

        # 旧的心跳和重连配置已被ServiceLifecycleManager替代
        timing_config = config.get("timing", {})
        # 保留http_timeout，其他配置已废弃
        self.http_timeout = int(timing_config.get("http_timeout_seconds", 10))

        # 监控任务已集成到ServiceLifecycleManager和ServiceContentManager中

        #  修改：根据是否有独立配置管理器或传入的mcp_config决定如何初始化MCPConfig
        if standalone_config_manager:
            # 使用独立配置，不依赖文件系统
            self.mcp_config = self._create_standalone_mcp_config(standalone_config_manager)
        elif mcp_config:
            # 使用传入的MCPConfig实例（用于数据空间）
            self.mcp_config = mcp_config
        else:
            # 使用传统配置
            self.mcp_config = MCPConfig()

        # 旧的资源管理配置已被ServiceLifecycleManager替代
        # 保留一些配置以避免错误，但实际不再使用

        #  单一数据源架构：简化客户端管理器初始化
        self.client_manager = ClientManager(
            global_agent_store_id=None  # 使用默认的"global_agent_store"
        )
        # 注意：client_services_path和agent_clients_path参数已废弃，保留在__init__参数中只为向后兼容

        # 会话管理器
        self.session_manager = SessionManager()

        # 本地服务管理器
        self.local_service_manager = get_local_service_manager()


        # 🆕 事件驱动架构：生命周期管理器将由 ServiceContainer 管理
        # 保留属性以兼容旧代码，但实际使用 store.container.lifecycle_manager
        self.lifecycle_manager = None  # 将在 store 初始化后设置

        # 🆕 事件驱动架构：内容管理器暂时保留（未来可能迁移到事件驱动）
        # self.content_manager = ServiceContentManager(self)
        self.content_manager = None  # 暂时禁用，避免依赖旧的 lifecycle_manager

        # 旧的工具更新监控器（保留兼容性，但将被废弃）
        self.tools_update_monitor = None

    def _get_timestamp(self) -> str:
        """获取统一格式的时间戳"""
        return time.strftime("%Y-%m-%d %H:%M:%S")

    def _safe_model_dump(self, obj) -> Dict[str, Any]:
        """安全地调用model_dump方法"""
        try:
            if hasattr(obj, 'model_dump'):
                return obj.model_dump()
            elif hasattr(obj, 'dict'):
                return obj.dict()
            else:
                # 如果没有序列化方法，尝试转换为字典
                return dict(obj) if hasattr(obj, '__dict__') else str(obj)
        except Exception as e:
            logger.warning(f"Failed to serialize object {type(obj)}: {e}")
            return {"error": f"Serialization failed: {str(e)}", "type": str(type(obj))}

    def _validate_configuration(self) -> bool:
        """验证配置的有效性
        
        Returns:
            bool: 配置是否有效
        """
        try:
            # 检查基本配置
            if not isinstance(self.config, dict):
                logger.error("Configuration must be a dictionary")
                return False
            
            # 检查timing配置
            timing_config = self.config.get("timing", {})
            if not isinstance(timing_config, dict):
                logger.error("Timing configuration must be a dictionary")
                return False
            
            # 检查http_timeout
            http_timeout = timing_config.get("http_timeout_seconds", 10)
            if not isinstance(http_timeout, (int, float)) or http_timeout <= 0:
                logger.error("http_timeout_seconds must be a positive number")
                return False
            
            logger.info("Configuration validation passed")
            return True
        except Exception as e:
            logger.error(f"Configuration validation failed: {e}")
            return False

    async def setup(self):
        """初始化编排器资源"""
        logger.info("Setting up MCP Orchestrator...")

        # 健康管理器配置已移除（事件驱动架构直接使用容器的 HealthMonitor）

        # 初始化工具更新监控器
        self._setup_tools_update_monitor()

        # 🆕 事件驱动架构：启动 ServiceContainer（如果已设置）
        if self.container:
            logger.info("Starting ServiceContainer components...")
            await self.container.start()
            logger.info("ServiceContainer components started")
        else:
            logger.warning("ServiceContainer not available, skipping container startup")

        # 启动监控任务（仅启动保留的工具更新监控器）
        try:
            await self.start_monitoring()
        except Exception as e:
            logger.warning(f"Failed to start monitoring tasks: {e}")

        #  新增：启动统一同步管理器
        try:
            logger.info("About to call _setup_sync_manager()...")
            await self._setup_sync_manager()
            logger.info("_setup_sync_manager() completed successfully")
        except Exception as e:
            logger.error(f"Exception in _setup_sync_manager(): {e}")
            import traceback
            logger.error(f"_setup_sync_manager() traceback: {traceback.format_exc()}")

        # 只做必要的资源初始化
        logger.info("MCP Orchestrator setup completed with event-driven architecture")

    async def _setup_sync_manager(self):
        """设置统一同步管理器"""
        try:
            logger.info(f"Setting up sync manager... standalone_config_manager={self.standalone_config_manager}")

            # 检查是否已经启动
            if hasattr(self, 'sync_manager') and self.sync_manager and self.sync_manager.is_running:
                logger.info("Unified sync manager already running, skipping...")
                return

            # 只有在非独立配置模式下才启用文件监听同步
            if not self.standalone_config_manager:
                logger.info("Creating unified sync manager...")
                from mcpstore.core.sync.unified_sync_manager import UnifiedMCPSyncManager
                if not hasattr(self, 'sync_manager') or not self.sync_manager:
                    logger.info("Initializing UnifiedMCPSyncManager...")
                    self.sync_manager = UnifiedMCPSyncManager(self)
                    logger.info("UnifiedMCPSyncManager created successfully")

                logger.info("Starting sync manager...")
                await self.sync_manager.start()
                logger.info("Unified sync manager started successfully")
            else:
                logger.info("Standalone mode: sync manager disabled (no file watching)")
        except Exception as e:
            logger.error(f"Failed to setup sync manager: {e}")
            import traceback
            logger.error(f"Sync manager setup traceback: {traceback.format_exc()}")
            # 不抛出异常，允许系统继续运行

    async def cleanup(self):
        """清理orchestrator资源"""
        try:
            logger.info("Cleaning up MCP Orchestrator...")

            # 停止同步管理器
            if self.sync_manager:
                await self.sync_manager.stop()
                self.sync_manager = None

            # 🆕 事件驱动架构：停止 ServiceContainer
            if self.container:
                logger.info("Stopping ServiceContainer components...")
                await self.container.stop()
                logger.info("ServiceContainer components stopped")

            logger.info("MCP Orchestrator cleanup completed")

        except Exception as e:
            logger.error(f"Error during orchestrator cleanup: {e}")

    async def shutdown(self):
        """关闭编排器并清理资源"""
        logger.info("Shutting down MCP Orchestrator...")

        # 🆕 事件驱动架构：停止 ServiceContainer
        try:
            if self.container:
                logger.debug("Stopping ServiceContainer...")
                await self.container.stop()
                logger.debug("ServiceContainer stopped")
        except Exception as e:
            logger.error(f"Error stopping ServiceContainer: {e}")

        logger.info("MCP Orchestrator shutdown completed")


    def _setup_tools_update_monitor(self):
        """设置工具更新监控器"""
        try:
            from mcpstore.extensions.monitoring import ToolsUpdateMonitor
            self.tools_update_monitor = ToolsUpdateMonitor(self)
            logger.info("Tools update monitor initialized")
        except Exception as e:
            logger.error(f"Failed to setup tools update monitor: {e}")

    # _create_standalone_mcp_config 方法现在在 StandaloneConfigMixin 中实现
