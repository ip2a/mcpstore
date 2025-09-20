"""
MCPOrchestrator Base Module
Orchestrator core base module - contains infrastructure and lifecycle management
"""

import os
import sys
import asyncio
import logging
import time
from typing import Dict, List, Any, Optional, Tuple
from datetime import datetime, timedelta

from mcpstore.core.registry import ServiceRegistry
from mcpstore.core.client_manager import ClientManager
from mcpstore.core.configuration.config_processor import ConfigProcessor
from mcpstore.core.integration.local_service_adapter import get_local_service_manager
from fastmcp import Client
from mcpstore.config.json_config import MCPConfig
from mcpstore.core.agents.session_manager import SessionManager
from mcpstore.core.lifecycle import get_health_manager, HealthStatus, HealthCheckResult, ServiceLifecycleManager, ServiceContentManager
from mcpstore.core.models.service import ServiceConnectionState

# Import mixin classes
from .monitoring_tasks import MonitoringTasksMixin
from .service_connection import ServiceConnectionMixin
from .tool_execution import ToolExecutionMixin
from .service_management import ServiceManagementMixin
from .resources_prompts import ResourcesPromptsMixin
from .network_utils import NetworkUtilsMixin
from .standalone_config import StandaloneConfigMixin

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

        # 🔧 新增：独立配置管理器
        self.standalone_config_manager = standalone_config_manager

        # 🔧 新增：统一同步管理器
        self.sync_manager = None

        # 🔧 新增：store引用（用于统一注册架构）
        self.store = None

        # 🔧 新增：异步同步助手（用于Resources和Prompts的同步方法）
        from mcpstore.core.utils.async_sync_helper import AsyncSyncHelper
        self._sync_helper = AsyncSyncHelper()

        # 旧的心跳和重连配置已被ServiceLifecycleManager替代
        timing_config = config.get("timing", {})
        # 保留http_timeout，其他配置已废弃
        self.http_timeout = int(timing_config.get("http_timeout_seconds", 10))

        # 监控任务已集成到ServiceLifecycleManager和ServiceContentManager中

        # 🔧 修改：根据是否有独立配置管理器或传入的mcp_config决定如何初始化MCPConfig
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

        # 🔧 单一数据源架构：简化客户端管理器初始化
        self.client_manager = ClientManager(
            global_agent_store_id=None  # 使用默认的"global_agent_store"
        )
        # 注意：client_services_path和agent_clients_path参数已废弃，保留在__init__参数中只为向后兼容

        # 会话管理器
        self.session_manager = SessionManager()

        # 本地服务管理器
        self.local_service_manager = get_local_service_manager()

        # 健康管理器
        self.health_manager = get_health_manager()

        # 服务生命周期管理器
        self.lifecycle_manager = ServiceLifecycleManager(self)

        # 服务内容管理器（替代旧的工具更新监控器）
        self.content_manager = ServiceContentManager(self)

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
        # 检查是否已经初始化
        if (hasattr(self, 'lifecycle_manager') and
            self.lifecycle_manager and
            self.lifecycle_manager.is_running):
            logger.info("MCP Orchestrator already set up, skipping...")
            return

        logger.info("Setting up MCP Orchestrator...")

        # 初始化健康管理器配置
        self._update_health_manager_config()

        # 初始化工具更新监控器
        self._setup_tools_update_monitor()

        # 启动生命周期管理器
        await self.lifecycle_manager.start()

        # 启动内容管理器
        await self.content_manager.start()

        # 启动监控任务（仅启动保留的工具更新监控器）
        try:
            await self.start_monitoring()
        except Exception as e:
            logger.warning(f"Failed to start monitoring tasks: {e}")

        # 🔧 新增：启动统一同步管理器
        try:
            logger.info("About to call _setup_sync_manager()...")
            await self._setup_sync_manager()
            logger.info("_setup_sync_manager() completed successfully")
        except Exception as e:
            logger.error(f"Exception in _setup_sync_manager(): {e}")
            import traceback
            logger.error(f"_setup_sync_manager() traceback: {traceback.format_exc()}")

        # 只做必要的资源初始化
        logger.info("MCP Orchestrator setup completed with lifecycle, content management and unified sync")

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

            # 停止生命周期管理器
            if hasattr(self, 'lifecycle_manager') and self.lifecycle_manager:
                await self.lifecycle_manager.stop()

            # 停止内容管理器
            if hasattr(self, 'content_manager') and self.content_manager:
                await self.content_manager.stop()

            logger.info("MCP Orchestrator cleanup completed")

        except Exception as e:
            logger.error(f"Error during orchestrator cleanup: {e}")

    async def shutdown(self):
        """关闭编排器并清理资源"""
        logger.info("Shutting down MCP Orchestrator...")

        # 🔧 修复：按正确顺序停止管理器，并添加错误处理
        try:
            # 先停止生命周期管理器（停止状态转换）
            logger.debug("Stopping lifecycle manager...")
            await self.lifecycle_manager.stop()
            logger.debug("Lifecycle manager stopped")
        except Exception as e:
            logger.error(f"Error stopping lifecycle manager: {e}")

        try:
            # 再停止内容管理器（停止内容更新）
            logger.debug("Stopping content manager...")
            await self.content_manager.stop()
            logger.debug("Content manager stopped")
        except Exception as e:
            logger.error(f"Error stopping content manager: {e}")

        # 旧的后台任务已被废弃，无需停止
        logger.info("Legacy monitoring tasks were already disabled")

        logger.info("MCP Orchestrator shutdown completed")

    def _update_health_manager_config(self):
        """更新健康管理器配置"""
        try:
            # 从配置中提取健康相关设置
            timing_config = self.config.get("timing", {})

            # 构建健康管理器配置
            health_config = {
                "local_service_ping_timeout": timing_config.get("local_service_ping_timeout", 3),
                "remote_service_ping_timeout": timing_config.get("remote_service_ping_timeout", 5),
                "startup_wait_time": timing_config.get("startup_wait_time", 2),
                "healthy_response_threshold": timing_config.get("healthy_response_threshold", 1.0),
                "warning_response_threshold": timing_config.get("warning_response_threshold", 3.0),
                "slow_response_threshold": timing_config.get("slow_response_threshold", 10.0),
                "enable_adaptive_timeout": timing_config.get("enable_adaptive_timeout", False),
                "adaptive_timeout_multiplier": timing_config.get("adaptive_timeout_multiplier", 2.0),
                "response_time_history_size": timing_config.get("response_time_history_size", 10)
            }

            # 更新健康管理器配置
            self.health_manager.update_config(health_config)
            logger.info(f"Health manager configuration updated: {health_config}")

        except Exception as e:
            logger.warning(f"Failed to update health manager config: {e}")

    def update_monitoring_config(self, monitoring_config: Dict[str, Any]):
        """更新监控配置（包括健康检查配置）"""
        try:
            # 更新时间配置
            if "timing" not in self.config:
                self.config["timing"] = {}

            # 映射监控配置到时间配置
            timing_mapping = {
                "local_service_ping_timeout": "local_service_ping_timeout",
                "remote_service_ping_timeout": "remote_service_ping_timeout",
                "startup_wait_time": "startup_wait_time",
                "healthy_response_threshold": "healthy_response_threshold",
                "warning_response_threshold": "warning_response_threshold",
                "slow_response_threshold": "slow_response_threshold",
                "enable_adaptive_timeout": "enable_adaptive_timeout",
                "adaptive_timeout_multiplier": "adaptive_timeout_multiplier",
                "response_time_history_size": "response_time_history_size"
            }

            for monitor_key, timing_key in timing_mapping.items():
                if monitor_key in monitoring_config and monitoring_config[monitor_key] is not None:
                    self.config["timing"][timing_key] = monitoring_config[monitor_key]

            # 更新健康管理器配置
            self._update_health_manager_config()

            logger.info("Monitoring configuration updated successfully")

        except Exception as e:
            logger.error(f"Failed to update monitoring config: {e}")
            raise

    def _setup_tools_update_monitor(self):
        """设置工具更新监控器"""
        try:
            from mcpstore.core.monitoring import ToolsUpdateMonitor
            self.tools_update_monitor = ToolsUpdateMonitor(self)
            logger.info("Tools update monitor initialized")
        except Exception as e:
            logger.error(f"Failed to setup tools update monitor: {e}")

    # _create_standalone_mcp_config 方法现在在 StandaloneConfigMixin 中实现
