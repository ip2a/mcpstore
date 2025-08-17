import logging
from typing import Optional, List, Dict, Any

from mcpstore.config.json_config import MCPConfig
from mcpstore.core.models.common import (
    RegistrationResponse, ConfigResponse, ExecutionResponse
)
from mcpstore.core.models.service import (
    RegisterRequestUnion, JsonUpdateRequest,
    ServiceInfo, TransportType, ServiceInfoResponse, ServiceConnectionState
)
from mcpstore.core.models.tool import (
    ToolInfo, ToolExecutionRequest
)
from mcpstore.core.orchestrator import MCPOrchestrator
from mcpstore.core.registry import ServiceRegistry
from mcpstore.core.unified_config import UnifiedConfigManager

from .context import MCPStoreContext

logger = logging.getLogger(__name__)

class MCPStore:
    """
    MCPStore - Intelligent Agent Tool Service Store
    Provides context switching entry points and common operations
    """
    def __init__(self, orchestrator: MCPOrchestrator, config: MCPConfig,
                 tool_record_max_file_size: int = 30, tool_record_retention_days: int = 7):
        self.orchestrator = orchestrator
        self.config = config
        self.registry = orchestrator.registry
        self.client_manager = orchestrator.client_manager
        # 🔧 修复：添加LocalServiceManager访问属性
        self.local_service_manager = orchestrator.local_service_manager
        self.session_manager = orchestrator.session_manager
        self.logger = logging.getLogger(__name__)

        # Tool recording configuration
        self.tool_record_max_file_size = tool_record_max_file_size
        self.tool_record_retention_days = tool_record_retention_days

        # Unified configuration manager
        self._unified_config = UnifiedConfigManager(
            mcp_config_path=config.json_path,
            client_services_path=self.client_manager.services_path
        )

        self._context_cache: Dict[str, MCPStoreContext] = {}
        self._store_context = self._create_store_context()

        # Data space manager (optional, only set when using data spaces)
        self._data_space_manager = None

        # 🔧 新增：缓存管理器
        from mcpstore.core.registry.cache_manager import ServiceCacheManager, CacheTransactionManager
        self.cache_manager = ServiceCacheManager(self.registry, self.orchestrator.lifecycle_manager)
        self.transaction_manager = CacheTransactionManager(self.registry)

        # 🔧 新增：智能查询接口
        from mcpstore.core.registry.smart_query import SmartCacheQuery
        self.query = SmartCacheQuery(self.registry)

    def _create_store_context(self) -> MCPStoreContext:
        """Create store-level context"""
        return MCPStoreContext(self)

    def get_store_context(self) -> MCPStoreContext:
        """Get store-level context"""
        return self._store_context

    @staticmethod
    def setup_store(mcp_config_file: str = None, debug: bool = False, standalone_config=None,
                   tool_record_max_file_size: int = 30, tool_record_retention_days: int = 7,
                   monitoring: dict = None):
        """
        Initialize MCPStore instance

        Args:
            mcp_config_file: Custom mcp.json configuration file path, uses default path if not specified
                           🔧 New: This parameter now supports data space isolation, each JSON file path corresponds to an independent data space
            debug: Whether to enable debug logging, default is False (no debug info displayed)
            standalone_config: Standalone configuration object, if provided, does not depend on environment variables
            tool_record_max_file_size: Maximum size of tool record JSON file (MB), default 30MB, set to -1 for no limit
            tool_record_retention_days: Tool record retention days, default 7 days, set to -1 for no deletion
            monitoring: Monitoring configuration dictionary, optional parameters:
                - health_check_seconds: Health check interval (default 30 seconds)
                - tools_update_hours: Tool update interval (default 2 hours)
                - reconnection_seconds: Reconnection interval (default 60 seconds)
                - cleanup_hours: Cleanup interval (default 24 hours)
                - enable_tools_update: Whether to enable tool updates (default True)
                - enable_reconnection: Whether to enable reconnection (default True)
                - update_tools_on_reconnection: Whether to update tools on reconnection (default True)

                                 You can still manually call add_service method to add services

        Returns:
            MCPStore instance
        """
        # 🔧 New: Support standalone configuration
        if standalone_config is not None:
            return MCPStore._setup_with_standalone_config(standalone_config, debug,
                                                        tool_record_max_file_size, tool_record_retention_days,
                                                        monitoring)

        # 🔧 New: Data space management
        if mcp_config_file is not None:
            return MCPStore._setup_with_data_space(mcp_config_file, debug,
                                                 tool_record_max_file_size, tool_record_retention_days,
                                                 monitoring)

        # Original logic: Use default configuration
        from mcpstore.config.config import LoggingConfig
        from mcpstore.core.monitoring.config import MonitoringConfigProcessor

        LoggingConfig.setup_logging(debug=debug)

        # Process monitoring configuration
        processed_monitoring = MonitoringConfigProcessor.process_config(monitoring)
        orchestrator_config = MonitoringConfigProcessor.convert_to_orchestrator_config(processed_monitoring)

        config = MCPConfig()
        registry = ServiceRegistry()

        # Merge base configuration and monitoring configuration
        base_config = config.load_config()
        base_config.update(orchestrator_config)

        orchestrator = MCPOrchestrator(base_config, registry)

        # Initialize orchestrator (including tool update monitor)
        import asyncio
        from mcpstore.core.async_sync_helper import AsyncSyncHelper

        # Use AsyncSyncHelper to properly manage async operations
        async_helper = AsyncSyncHelper()
        try:
            # Synchronously run orchestrator.setup(), ensure completion
            async_helper.run_async(orchestrator.setup())
        except Exception as e:
            logger.error(f"Failed to setup orchestrator: {e}")
            raise

        store = MCPStore(orchestrator, config, tool_record_max_file_size, tool_record_retention_days)

        # 🔧 新增：设置orchestrator的store引用（用于统一注册架构）
        orchestrator.store = store

        # 🔧 新增：初始化缓存
        logger.info("🔄 [SETUP_STORE] 开始初始化缓存...")
        try:
            async_helper.run_async(store.initialize_cache_from_files())
            logger.info("✅ [SETUP_STORE] 缓存初始化完成")
        except Exception as e:
            logger.error(f"❌ [SETUP_STORE] 缓存初始化失败: {e}")
            import traceback
            logger.error(f"❌ [SETUP_STORE] 缓存初始化失败详情: {traceback.format_exc()}")
            # 缓存初始化失败不应该阻止系统启动

        return store

    @staticmethod
    def _setup_with_data_space(mcp_config_file: str, debug: bool = False,
                              tool_record_max_file_size: int = 30, tool_record_retention_days: int = 7,
                              monitoring: dict = None):
        """
        Initialize MCPStore with data space (supports independent data directory)

        Args:
            mcp_config_file: MCP JSON configuration file path (data space root directory)
            debug: Whether to enable debug logging
            tool_record_max_file_size: Maximum size of tool record JSON file (MB)
            tool_record_retention_days: Tool record retention days
            monitoring: Monitoring configuration dictionary


        Returns:
            MCPStore instance
        """
        from mcpstore.config.config import LoggingConfig
        from mcpstore.core.data_space_manager import DataSpaceManager
        from mcpstore.core.monitoring.config import MonitoringConfigProcessor

        # Setup logging
        LoggingConfig.setup_logging(debug=debug)

        try:
            # Initialize data space
            data_space_manager = DataSpaceManager(mcp_config_file)
            if not data_space_manager.initialize_workspace():
                raise RuntimeError(f"Failed to initialize workspace for: {mcp_config_file}")

            logger.info(f"Data space initialized: {data_space_manager.workspace_dir}")

            # Process monitoring configuration
            processed_monitoring = MonitoringConfigProcessor.process_config(monitoring)
            orchestrator_config = MonitoringConfigProcessor.convert_to_orchestrator_config(processed_monitoring)

            # Create configuration using specified MCP JSON file
            config = MCPConfig(json_path=mcp_config_file)
            registry = ServiceRegistry()

            # Get file paths in data space (using defaults subdirectory)
            client_services_path = str(data_space_manager.get_file_path("defaults/client_services.json"))
            agent_clients_path = str(data_space_manager.get_file_path("defaults/agent_clients.json"))

            # Merge base configuration and monitoring configuration
            base_config = config.load_config()
            base_config.update(orchestrator_config)

            # Create orchestrator with data space support, pass correct mcp_config instance
            orchestrator = MCPOrchestrator(
                base_config,
                registry,
                client_services_path=client_services_path,
                agent_clients_path=agent_clients_path,
                mcp_config=config  # Pass in the config instance of data space
            )

            # 🔧 重构：为数据空间模式设置FastMCP适配器的工作目录
            from mcpstore.core.local_service_manager import set_local_service_manager_work_dir
            set_local_service_manager_work_dir(str(data_space_manager.workspace_dir))

            # Create store instance and set data space manager
            store = MCPStore(orchestrator, config, tool_record_max_file_size, tool_record_retention_days)
            store._data_space_manager = data_space_manager

            # 🔧 新增：设置orchestrator的store引用（用于统一注册架构）
            orchestrator.store = store

            # Initialize orchestrator (including tool update monitor)
            from mcpstore.core.async_sync_helper import AsyncSyncHelper

            # Use AsyncSyncHelper to properly manage async operations
            async_helper = AsyncSyncHelper()
            try:
                # Run orchestrator.setup() synchronously, ensure completion
                async_helper.run_async(orchestrator.setup())
            except Exception as e:
                logger.error(f"Failed to setup orchestrator: {e}")
                raise

            # 🔧 新增：初始化缓存
            try:
                async_helper.run_async(store.initialize_cache_from_files())
            except Exception as e:
                logger.warning(f"Failed to initialize cache from files: {e}")
                # 缓存初始化失败不应该阻止系统启动

            logger.info(f"MCPStore setup with data space completed: {mcp_config_file}")
            return store

        except Exception as e:
            logger.error(f"Failed to setup MCPStore with data space: {e}")
            raise

    @staticmethod
    def _setup_with_standalone_config(standalone_config, debug: bool = False,
                                     tool_record_max_file_size: int = 30, tool_record_retention_days: int = 7,
                                     monitoring: dict = None):
        """
        使用独立配置初始化MCPStore（不依赖环境变量）

        Args:
            standalone_config: 独立配置对象
            debug: 是否启用调试日志
            tool_record_max_file_size: 工具记录JSON文件最大大小(MB)
            tool_record_retention_days: 工具记录保留天数
            monitoring: 监控配置字典

        Returns:
            MCPStore实例
        """
        from mcpstore.core.standalone_config import StandaloneConfigManager, StandaloneConfig
        from mcpstore.core.registry import ServiceRegistry
        from mcpstore.core.orchestrator import MCPOrchestrator
        from mcpstore.core.monitoring.config import MonitoringConfigProcessor
        import logging

        # 处理配置类型
        if isinstance(standalone_config, StandaloneConfig):
            config_manager = StandaloneConfigManager(standalone_config)
        elif isinstance(standalone_config, StandaloneConfigManager):
            config_manager = standalone_config
        else:
            raise ValueError("standalone_config must be StandaloneConfig or StandaloneConfigManager")

        # 设置日志
        log_level = logging.DEBUG if debug or config_manager.config.enable_debug else logging.INFO
        logging.basicConfig(
            level=log_level,
            format=config_manager.config.log_format
        )

        # 处理监控配置
        processed_monitoring = MonitoringConfigProcessor.process_config(monitoring)
        monitoring_orchestrator_config = MonitoringConfigProcessor.convert_to_orchestrator_config(processed_monitoring)

        # 创建组件
        registry = ServiceRegistry()

        # 使用独立配置创建orchestrator
        mcp_config_dict = config_manager.get_mcp_config()
        timing_config = config_manager.get_timing_config()

        # 创建一个兼容的配置对象
        class StandaloneMCPConfig:
            def __init__(self, config_dict, config_manager):
                self._config = config_dict
                self._manager = config_manager
                self.json_path = config_manager.config.mcp_config_file or ":memory:"

            def load_config(self):
                return self._config

            def get_service_config(self, name):
                return self._manager.get_service_config(name)

        config = StandaloneMCPConfig(mcp_config_dict, config_manager)

        # 创建orchestrator，合并所有配置
        orchestrator_config = mcp_config_dict.copy()
        orchestrator_config["timing"] = timing_config
        orchestrator_config["network"] = config_manager.get_network_config()
        orchestrator_config["environment"] = config_manager.get_environment_config()

        # 合并监控配置（监控配置优先级更高）
        orchestrator_config.update(monitoring_orchestrator_config)

        orchestrator = MCPOrchestrator(orchestrator_config, registry, config_manager)

        # 初始化orchestrator（包括工具更新监控器）
        import asyncio
        try:
            # 尝试在当前事件循环中运行
            loop = asyncio.get_running_loop()
            # 如果已有事件循环，创建任务稍后执行
            asyncio.create_task(orchestrator.setup())
        except RuntimeError:
            # 没有运行的事件循环，创建新的
            loop = asyncio.new_event_loop()
            asyncio.set_event_loop(loop)
            try:
                loop.run_until_complete(orchestrator.setup())
            finally:
                loop.close()

        return MCPStore(orchestrator, config, tool_record_max_file_size, tool_record_retention_days)
  
    def _create_agent_context(self, agent_id: str) -> MCPStoreContext:
        """Create agent-level context"""
        return MCPStoreContext(self, agent_id)

    def for_store(self) -> MCPStoreContext:
        """Get store-level context"""
        # global_agent_store as store agent_id
        return self._store_context

    def for_agent(self, agent_id: str) -> MCPStoreContext:
        """Get agent-level context (with caching)"""
        if agent_id not in self._context_cache:
            self._context_cache[agent_id] = self._create_agent_context(agent_id)
        return self._context_cache[agent_id]

    def get_unified_config(self) -> UnifiedConfigManager:
        """Get unified configuration manager

        Returns:
            UnifiedConfigManager: Unified configuration manager instance
        """
        return self._unified_config

    async def register_service(self, payload: RegisterRequestUnion, agent_id: Optional[str] = None) -> Dict[str, str]:
        """Refactored: Register service, supports batch service_names registration"""
        service_names = getattr(payload, 'service_names', None)
        if not service_names:
            raise ValueError("payload must contain service_names field")
        results = {}
        agent_key = agent_id or self.client_manager.global_agent_store_id
        for name in service_names:
            success, msg = await self.orchestrator.connect_service(name)
            if not success:
                results[name] = f"Connection failed: {msg}"
                continue
            session = self.registry.get_session(agent_key, name)
            if not session:
                results[name] = "Failed to get session"
                continue
            tools = []
            try:
                tools = await session.list_tools() if hasattr(session, 'list_tools') else []
            except Exception as e:
                results[name] = f"Failed to get tools: {e}"
                continue
            added_tools = self.registry.add_service(agent_key, name, session, [(tool['name'], tool) for tool in tools])
            results[name] = f"Registration successful, tool count: {len(added_tools)}"
        return results

    # === Refactored service registration methods ===

    async def register_all_services_for_store(self) -> RegistrationResponse:
        """
        @deprecated This method is deprecated, please use unified synchronization mechanism

        Store level: Register all services in configuration file

        ⚠️ Warning: This method has been replaced by unified synchronization mechanism, recommended to use:
        - store.for_store().add_service_async() - No parameter full registration
        - orchestrator.sync_manager.sync_global_agent_store_from_mcp_json() - Direct synchronization

        Temporarily retained for backward compatibility, but migration to new mechanism is recommended

        Returns:
            RegistrationResponse: Registration result
        """
        import warnings
        warnings.warn(
            "register_all_services_for_store() is deprecated, please use unified synchronization mechanism",
            DeprecationWarning,
            stacklevel=2
        )
        try:
            all_services = self.config.load_config().get("mcpServers", {})
            agent_id = self.client_manager.global_agent_store_id
            registered_client_ids = []
            registered_services = []

            logger.info(f"Store level full registration, total {len(all_services)} services")

            for name in all_services.keys():
                try:
                    # Use same-name service processing logic
                    success = self.client_manager.replace_service_in_agent(
                        agent_id=agent_id,
                        service_name=name,
                        new_service_config=all_services[name]
                    )
                    if not success:
                        logger.error(f"Failed to replace service {name}")
                        continue

                    # Get newly created/updated client_id for Registry registration
                    client_ids = self.client_manager.get_agent_clients(agent_id)
                    for client_id_check in client_ids:
                        client_config = self.client_manager.get_client_config(client_id_check)
                        if client_config and name in client_config.get("mcpServers", {}):
                            await self.orchestrator.register_json_services(client_config, client_id=client_id_check)
                            registered_client_ids.append(client_id_check)
                            registered_services.append(name)
                            logger.info(f"Successfully registered service: {name}")
                            break
                except Exception as e:
                    logger.error(f"Failed to register service {name}: {e}")
                    continue

            return RegistrationResponse(
                success=True,
                client_id=agent_id,
                service_names=registered_services,
                config={"client_ids": registered_client_ids, "services": registered_services}
            )

        except Exception as e:
            logger.error(f"Store全量服务注册失败: {e}")
            return RegistrationResponse(
                success=False,
                message=str(e),
                client_id=self.client_manager.global_agent_store_id,
                service_names=[],
                config={}
            )

    async def register_services_for_agent(self, agent_id: str, service_names: List[str]) -> RegistrationResponse:
        """
        Agent级别：为指定Agent注册指定的服务

        Args:
            agent_id: Agent ID
            service_names: 要注册的服务名称列表

        Returns:
            RegistrationResponse: 注册结果
        """
        try:
            all_services = self.config.load_config().get("mcpServers", {})
            registered_client_ids = []
            registered_services = []

            logger.info(f"Agent级别注册，agent_id: {agent_id}, 服务: {service_names}")

            for name in service_names:
                try:
                    if name not in all_services:
                        logger.warning(f"服务 {name} 未在全局配置中找到，跳过")
                        continue

                    # 使用同名服务处理逻辑
                    success = self.client_manager.replace_service_in_agent(
                        agent_id=agent_id,
                        service_name=name,
                        new_service_config=all_services[name]
                    )
                    if not success:
                        logger.error(f"替换服务 {name} 失败")
                        continue

                    # 🔧 重构：使用统一的add_service方法
                    client_ids = self.client_manager.get_agent_clients(agent_id)
                    for client_id_check in client_ids:
                        client_config = self.client_manager.get_client_config(client_id_check)
                        if client_config and name in client_config.get("mcpServers", {}):
                            # 使用统一注册架构
                            await self.for_agent(agent_id).add_service_async(client_config, source="agent_register")
                            registered_client_ids.append(client_id_check)
                            registered_services.append(name)
                            logger.info(f"成功注册服务: {name} (via unified add_service)")
                            break
                except Exception as e:
                    logger.error(f"注册服务 {name} 失败: {e}")
                    continue

            return RegistrationResponse(
                success=True,
                client_id=agent_id,
                service_names=registered_services,
                config={"client_ids": registered_client_ids, "services": registered_services}
            )

        except Exception as e:
            logger.error(f"Agent服务注册失败: {e}")
            return RegistrationResponse(
                success=False,
                message=str(e),
                client_id=agent_id,
                service_names=[],
                config={}
            )

    async def register_services_temporarily(self, service_names: List[str]) -> RegistrationResponse:
        """
        临时注册：创建临时Agent并注册指定服务

        Args:
            service_names: 要注册的服务名称列表

        Returns:
            RegistrationResponse: 注册结果
        """
        try:
            logger.info(f"临时注册模式，services: {service_names}")
            config = self.orchestrator.create_client_config_from_names(service_names)
            import time
            temp_agent_id = f"temp_agent_{int(time.time() * 1000)}"
            results = await self.orchestrator.register_json_services(config)
            return RegistrationResponse(
                success=True,
                client_id=temp_agent_id,
                service_names=list(results.get("services", {}).keys()),
                config=config
            )

        except Exception as e:
            logger.error(f"临时服务注册失败: {e}")
            return RegistrationResponse(
                success=False,
                message=str(e),
                client_id="temp_agent",
                service_names=[],
                config={}
            )

    async def register_selected_services_for_store(self, service_names: List[str]) -> RegistrationResponse:
        """
        Store级别：注册指定的服务（而非全部）

        Args:
            service_names: 要注册的服务名称列表

        Returns:
            RegistrationResponse: 注册结果
        """
        try:
            all_services = self.config.load_config().get("mcpServers", {})
            agent_id = self.client_manager.global_agent_store_id
            registered_client_ids = []
            registered_services = []

            logger.info(f"Store级别选择性注册，服务: {service_names}")

            for name in service_names:
                try:
                    if name not in all_services:
                        logger.warning(f"服务 {name} 未在全局配置中找到，跳过")
                        continue

                    # 使用同名服务处理逻辑
                    success = self.client_manager.replace_service_in_agent(
                        agent_id=agent_id,
                        service_name=name,
                        new_service_config=all_services[name]
                    )
                    if not success:
                        logger.error(f"替换服务 {name} 失败")
                        continue

                    # 🔧 重构：使用统一的add_service方法
                    client_ids = self.client_manager.get_agent_clients(agent_id)
                    for client_id_check in client_ids:
                        client_config = self.client_manager.get_client_config(client_id_check)
                        if client_config and name in client_config.get("mcpServers", {}):
                            # 使用统一注册架构
                            await self.for_store().add_service_async(client_config, source="store_selected")
                            registered_client_ids.append(client_id_check)
                            registered_services.append(name)
                            logger.info(f"成功注册服务: {name} (via unified add_service)")
                            break
                except Exception as e:
                    logger.error(f"注册服务 {name} 失败: {e}")
                    continue

            return RegistrationResponse(
                success=True,
                client_id=agent_id,
                service_names=registered_services,
                config={"client_ids": registered_client_ids, "services": registered_services}
            )

        except Exception as e:
            logger.error(f"Store选择性服务注册失败: {e}")
            return RegistrationResponse(
                success=False,
                message=str(e),
                client_id=self.client_manager.global_agent_store_id,
                service_names=[],
                config={}
            )

    # === 兼容性方法（向后兼容，但标记为废弃） ===

    async def register_json_service(self, client_id: Optional[str] = None, service_names: Optional[List[str]] = None) -> RegistrationResponse:
        """
        @deprecated 此方法已废弃，请使用更明确的方法：
        - register_all_services_for_store() - Store全量注册
        - register_selected_services_for_store(service_names) - Store选择性注册
        - register_services_for_agent(agent_id, service_names) - Agent注册
        - register_services_temporarily(service_names) - 临时注册

        为了向后兼容暂时保留，但建议迁移到新方法
        """
        import warnings
        # warnings.warn(
        #     "register_json_service() 已废弃，请使用更明确的方法",
        #     DeprecationWarning,
        #     stacklevel=2
        # )

        # 根据参数组合调用新方法
        if client_id and client_id == self.client_manager.global_agent_store_id and not service_names:
            # Store 全量注册：使用统一同步机制
            if hasattr(self.orchestrator, 'sync_manager') and self.orchestrator.sync_manager:
                sync_results = await self.orchestrator.sync_manager.sync_global_agent_store_from_mcp_json()
                return RegistrationResponse(
                    success=bool(sync_results.get("added") or sync_results.get("updated")),
                    client_id=self.client_manager.global_agent_store_id,
                    service_names=sync_results.get("added", []) + sync_results.get("updated", []),
                    config=sync_results
                )
            else:
                # 回退到旧方法（带警告）
                return await self.register_all_services_for_store()
        elif not client_id and service_names:
            # 临时注册
            return await self.register_services_temporarily(service_names)
        elif not client_id and not service_names:
            # 默认全量注册：使用统一同步机制
            if hasattr(self.orchestrator, 'sync_manager') and self.orchestrator.sync_manager:
                sync_results = await self.orchestrator.sync_manager.sync_global_agent_store_from_mcp_json()
                return RegistrationResponse(
                    success=bool(sync_results.get("added") or sync_results.get("updated")),
                    client_id=self.client_manager.global_agent_store_id,
                    service_names=sync_results.get("added", []) + sync_results.get("updated", []),
                    config=sync_results
                )
            else:
                # 回退到旧方法（带警告）
                return await self.register_all_services_for_store()
        else:
            # Agent 指定服务注册
            return await self.register_services_for_agent(client_id, service_names or [])

    async def update_json_service(self, payload: JsonUpdateRequest) -> RegistrationResponse:
        """更新服务配置，等价于 PUT /register/json"""
        # 🔧 重构：使用统一的add_service方法
        try:
            if payload.client_id and payload.client_id != self.client_manager.global_agent_store_id:
                # Agent级别更新
                context = self.for_agent(payload.client_id)
            else:
                # Store级别更新
                context = self.for_store()

            await context.add_service_async(payload.config, source="api_update")

            return RegistrationResponse(
                success=True,
                client_id=payload.client_id or self.client_manager.global_agent_store_id,
                service_names=list(payload.config.get("mcpServers", {}).keys()),
                config=payload.config
            )
        except Exception as e:
            logger.error(f"Failed to update service via unified add_service: {e}")
            return RegistrationResponse(
                success=False,
                message=str(e),
                client_id=payload.client_id or self.client_manager.global_agent_store_id,
                service_names=[],
                config={}
            )

    def get_json_config(self, client_id: Optional[str] = None) -> ConfigResponse:
        """查询服务配置，等价于 GET /register/json"""
        if not client_id or client_id == self.client_manager.global_agent_store_id:
            config = self.config.load_config()
            return ConfigResponse(
                success=True,
                client_id=self.client_manager.global_agent_store_id,
                config=config
            )
        else:
            config = self.client_manager.get_client_config(client_id)
            if not config:
                raise ValueError(f"Client configuration not found: {client_id}")
            return ConfigResponse(
                success=True,
                client_id=client_id,
                config=config
            )

    async def process_tool_request(self, request: ToolExecutionRequest) -> ExecutionResponse:
        """
        处理工具执行请求（FastMCP 标准）

        Args:
            request: 工具执行请求

        Returns:
            ExecutionResponse: 工具执行响应
        """
        import time
        start_time = time.time()

        try:
            # 验证请求参数
            if not request.tool_name:
                raise ValueError("Tool name cannot be empty")
            if not request.service_name:
                raise ValueError("Service name cannot be empty")

            logger.debug(f"Processing tool request: {request.service_name}::{request.tool_name}")

            # 检查服务生命周期状态
            agent_id = request.agent_id or self.client_manager.global_agent_store_id
            service_state = self.orchestrator.lifecycle_manager.get_service_state(agent_id, request.service_name)

            # 如果服务处于不可用状态，返回错误
            from mcpstore.core.models.service import ServiceConnectionState
            if service_state in [ServiceConnectionState.RECONNECTING, ServiceConnectionState.UNREACHABLE,
                               ServiceConnectionState.DISCONNECTING, ServiceConnectionState.DISCONNECTED]:
                error_msg = f"Service '{request.service_name}' is currently {service_state.value} and unavailable for tool execution"
                logger.warning(error_msg)
                return ExecutionResponse(
                    success=False,
                    result=None,
                    error=error_msg,
                    execution_time=time.time() - start_time,
                    service_name=request.service_name,
                    tool_name=request.tool_name,
                    agent_id=agent_id
                )

            # 执行工具（使用 FastMCP 标准）
            result = await self.orchestrator.execute_tool_fastmcp(
                service_name=request.service_name,
                tool_name=request.tool_name,
                arguments=request.args,
                agent_id=request.agent_id,
                timeout=request.timeout,
                progress_handler=request.progress_handler,
                raise_on_error=request.raise_on_error
            )

            # 📊 记录成功的工具执行
            try:
                duration_ms = (time.time() - start_time) * 1000

                # 获取对应的Context来记录监控数据
                if request.agent_id:
                    context = self.for_agent(request.agent_id)
                else:
                    context = self.for_store()

                # 使用新的详细记录方法
                context._monitoring.record_tool_execution_detailed(
                    tool_name=request.tool_name,
                    service_name=request.service_name,
                    params=request.args,
                    result=result,
                    error=None,
                    response_time=duration_ms
                )
            except Exception as monitor_error:
                logger.warning(f"Failed to record tool execution: {monitor_error}")

            return ExecutionResponse(
                success=True,
                result=result
            )
        except Exception as e:
            # 📊 记录失败的工具执行
            try:
                duration_ms = (time.time() - start_time) * 1000

                # 获取对应的Context来记录监控数据
                if request.agent_id:
                    context = self.for_agent(request.agent_id)
                else:
                    context = self.for_store()

                # 使用新的详细记录方法
                context._monitoring.record_tool_execution_detailed(
                    tool_name=request.tool_name,
                    service_name=request.service_name,
                    params=request.args,
                    result=None,
                    error=str(e),
                    response_time=duration_ms
                )
            except Exception as monitor_error:
                logger.warning(f"Failed to record failed tool execution: {monitor_error}")

            logger.error(f"Tool execution failed: {e}")
            return ExecutionResponse(
                success=False,
                error=str(e)
            )

    def register_clients(self, client_configs: Dict[str, Any]) -> RegistrationResponse:
        """注册客户端，等价于 /register_clients"""
        # 这里只是示例，具体实现需根据 client_manager 逻辑完善
        for client_id, config in client_configs.items():
            self.client_manager.save_client_config(client_id, config)
        return RegistrationResponse(
            success=True,
            message="Clients registered successfully",
            client_id="",  # 多客户端注册时不适用
            service_names=[],  # 多客户端注册时不适用
            config={"client_ids": list(client_configs.keys())}
        )

    async def get_health_status(self, id: Optional[str] = None, agent_mode: bool = False) -> Dict[str, Any]:
        # TODO:该方法带完善 这个方法有一定的混乱 要分离面向用户的直观方法名 和面向业务的独立函数功能
        """
        获取服务健康状态：
        - store未传id 或 id==global_agent_store：聚合 global_agent_store 下所有 client_id 的服务健康状态
        - store传普通 client_id：只查该 client_id 下的服务健康状态
        - agent级别：聚合 agent_id 下所有 client_id 的服务健康状态；如果 id 不是 agent_id，尝试作为 client_id 查
        """
        from mcpstore.core.client_manager import ClientManager
        client_manager: ClientManager = self.client_manager
        services = []
        # 1. store未传id 或 id==global_agent_store，聚合 global_agent_store 下所有 client_id 的服务健康状态
        if not agent_mode and (not id or id == self.client_manager.global_agent_store_id):
            client_ids = client_manager.get_agent_clients(self.client_manager.global_agent_store_id)
            for client_id in client_ids:
                service_names = self.registry.get_all_service_names(client_id)
                for name in service_names:
                    config = self.config.get_service_config(name) or {}

                    # 获取生命周期状态
                    service_state = self.orchestrator.lifecycle_manager.get_service_state(client_id, name)
                    state_metadata = self.orchestrator.lifecycle_manager.get_service_metadata(client_id, name)

                    service_status = {
                        "name": name,
                        "url": config.get("url", ""),
                        "transport_type": config.get("transport", ""),
                        "status": service_state.value,  # 使用新的7状态枚举
                        "command": config.get("command"),
                        "args": config.get("args"),
                        "package_name": config.get("package_name"),
                        # 新增生命周期相关信息
                        "response_time": state_metadata.response_time if state_metadata else None,
                        "consecutive_failures": state_metadata.consecutive_failures if state_metadata else 0,
                        "last_state_change": state_metadata.state_entered_time.isoformat() if state_metadata and state_metadata.state_entered_time else None
                    }
                    services.append(service_status)
            return {
                "orchestrator_status": "running",
                "active_services": len(services),
                "services": services
            }
        # 2. store传普通 client_id，只查该 client_id 下的服务健康状态
        if not agent_mode and id:
            if id == self.client_manager.global_agent_store_id:
                return {
                    "orchestrator_status": "running",
                    "active_services": 0,
                    "services": []
                }
            service_names = self.registry.get_all_service_names(id)
            for name in service_names:
                config = self.config.get_service_config(name) or {}

                # 获取生命周期状态
                service_state = self.orchestrator.lifecycle_manager.get_service_state(id, name)
                state_metadata = self.orchestrator.lifecycle_manager.get_service_metadata(id, name)

                service_status = {
                    "name": name,
                    "url": config.get("url", ""),
                    "transport_type": config.get("transport", ""),
                    "status": service_state.value,  # 使用新的7状态枚举
                    "command": config.get("command"),
                    "args": config.get("args"),
                    "package_name": config.get("package_name"),
                    # 新增生命周期相关信息
                    "response_time": state_metadata.response_time if state_metadata else None,
                    "consecutive_failures": state_metadata.consecutive_failures if state_metadata else 0,
                    "last_state_change": state_metadata.state_entered_time.isoformat() if state_metadata and state_metadata.state_entered_time else None
                }
                services.append(service_status)
            return {
                "orchestrator_status": "running",
                "active_services": len(services),
                "services": services
            }
        # 3. agent级别，聚合 agent_id 下所有 client_id 的服务健康状态；如果 id 不是 agent_id，尝试作为 client_id 查
        if agent_mode and id:
            client_ids = client_manager.get_agent_clients(id)
            if client_ids:
                for client_id in client_ids:
                    service_names = self.registry.get_all_service_names(client_id)
                    for name in service_names:
                        config = self.config.get_service_config(name) or {}

                        # 获取生命周期状态
                        service_state = self.orchestrator.lifecycle_manager.get_service_state(client_id, name)
                        state_metadata = self.orchestrator.lifecycle_manager.get_service_metadata(client_id, name)

                        service_status = {
                            "name": name,
                            "url": config.get("url", ""),
                            "transport_type": config.get("transport", ""),
                            "status": service_state.value,  # 使用新的7状态枚举
                            "command": config.get("command"),
                            "args": config.get("args"),
                            "package_name": config.get("package_name"),
                            # 新增生命周期相关信息
                            "response_time": state_metadata.response_time if state_metadata else None,
                            "consecutive_failures": state_metadata.consecutive_failures if state_metadata else 0,
                            "last_state_change": state_metadata.state_entered_time.isoformat() if state_metadata and state_metadata.state_entered_time else None
                        }
                        services.append(service_status)
                return {
                    "orchestrator_status": "running",
                    "active_services": len(services),
                    "services": services
                }
            else:
                service_names = self.registry.get_all_service_names(id)
                for name in service_names:
                    config = self.config.get_service_config(name) or {}

                    # 获取生命周期状态
                    service_state = self.orchestrator.lifecycle_manager.get_service_state(id, name)
                    state_metadata = self.orchestrator.lifecycle_manager.get_service_metadata(id, name)

                    service_status = {
                        "name": name,
                        "url": config.get("url", ""),
                        "transport_type": config.get("transport", ""),
                        "status": service_state.value,  # 使用新的7状态枚举
                        "command": config.get("command"),
                        "args": config.get("args"),
                        "package_name": config.get("package_name"),
                        # 新增生命周期相关信息
                        "response_time": state_metadata.response_time if state_metadata else None,
                        "consecutive_failures": state_metadata.consecutive_failures if state_metadata else 0,
                        "last_state_change": state_metadata.state_entered_time.isoformat() if state_metadata and state_metadata.state_entered_time else None
                    }
                    services.append(service_status)
                return {
                    "orchestrator_status": "running",
                    "active_services": len(services),
                    "services": services
                }
        return {
            "orchestrator_status": "running",
            "active_services": 0,
            "services": []
        }

    async def get_service_info(self, name: str, agent_id: Optional[str] = None) -> ServiceInfoResponse:
        """
        获取服务详细信息（严格按上下文隔离）：
        - 未传 agent_id：仅在 global_agent_store 下所有 client_id 中查找服务
        - 传 agent_id：仅在该 agent_id 下所有 client_id 中查找服务

        优先级：按client_id顺序返回第一个匹配的服务
        """
        from mcpstore.core.client_manager import ClientManager
        client_manager: ClientManager = self.client_manager

        # 严格按上下文获取要查找的 client_ids
        if not agent_id:
            # Store上下文：只查找global_agent_store下的服务
            client_ids = client_manager.get_agent_clients(self.client_manager.global_agent_store_id)
            context_type = "store"
        else:
            # Agent上下文：只查找指定agent下的服务
            client_ids = client_manager.get_agent_clients(agent_id)
            context_type = f"agent({agent_id})"

        if not client_ids:
            self.logger.debug(f"No clients found for {context_type} context")
            return ServiceInfoResponse(service=None, tools=[], connected=False)

        self.logger.debug(f"Searching for service '{name}' in {context_type} context, clients: {client_ids}")

        # 🔧 [REFACTOR] 修复查找逻辑：Registry按agent_id存储服务，不是client_id
        # 确定要查找的agent_id
        search_agent_id = agent_id if agent_id else self.client_manager.global_agent_store_id

        # 检查服务是否存在于指定的agent下
        if self.registry.has_service(search_agent_id, name):
            self.logger.debug(f"Found service '{name}' in agent '{search_agent_id}' for {context_type}")

            # 获取服务配置
            config = self.config.get_service_config(name) or {}
            service_tools = self.registry.get_tools_for_service(search_agent_id, name)

            # 获取工具详细信息
            detailed_tools = []
            for tool_name in service_tools:
                tool_info = self.registry._get_detailed_tool_info(search_agent_id, tool_name)
                if tool_info:
                    detailed_tools.append(tool_info)

            # 🔧 [REFACTOR] 使用Registry的get_service_info方法获取完整的ServiceInfo
            service_info = self.registry.get_service_info(search_agent_id, name)

            if service_info:
                # 获取服务健康状态
                is_healthy = await self.orchestrator.is_service_healthy(name, search_agent_id)

                # 更新状态信息
                if hasattr(service_info, 'status'):
                    # 保持原有状态，只在需要时更新健康状态
                    pass

                return ServiceInfoResponse(
                    service=service_info,
                    tools=detailed_tools,
                    connected=True
                )
            else:
                # 如果Registry没有返回ServiceInfo，构建一个基本的
                service_info = ServiceInfo(
                    url=config.get("url", ""),
                    name=name,
                    transport_type=self._infer_transport_type(config),
                    status=ServiceConnectionState.DISCONNECTED,
                    tool_count=len(service_tools),
                    keep_alive=config.get("keep_alive", False),
                    working_dir=config.get("working_dir"),
                    env=config.get("env"),
                    command=config.get("command"),
                    args=config.get("args"),
                    package_name=config.get("package_name"),
                    config=config  # 🔧 [REFACTOR] 添加config字段
                )

                return ServiceInfoResponse(
                    service=service_info,
                    tools=detailed_tools,
                    connected=False
                )

        self.logger.debug(f"Service '{name}' not found in any client for {context_type}")
        return ServiceInfoResponse(
            service=None,
            tools=[],
            connected=False
        )

    def _infer_transport_type(self, service_config: Dict[str, Any]) -> TransportType:
        """推断服务的传输类型"""
        if not service_config:
            return TransportType.STREAMABLE_HTTP
            
        # 优先使用 transport 字段
        transport = service_config.get("transport")
        if transport:
            try:
                return TransportType(transport)
            except ValueError:
                pass
                
        # 其次根据 url 判断
        if service_config.get("url"):
            return TransportType.STREAMABLE_HTTP
            
        # 根据 command/args 判断
        cmd = (service_config.get("command") or "").lower()
        args = " ".join(service_config.get("args", [])).lower()
        
        if "python" in cmd or ".py" in args:
            return TransportType.STDIO_PYTHON
        if "node" in cmd or ".js" in args:
            return TransportType.STDIO_NODE
        if "uvx" in cmd:
            return TransportType.STDIO  # 使用通用的STDIO类型
        if "npx" in cmd:
            return TransportType.STDIO  # 使用通用的STDIO类型
            
        return TransportType.STREAMABLE_HTTP

    async def list_services(self, id: Optional[str] = None, agent_mode: bool = False) -> List[ServiceInfo]:
        """
        纯缓存模式的服务列表获取

        🔧 新特点：
        - 完全从缓存获取数据
        - 包含完整的 Agent-Client 信息
        - 高性能，无文件IO
        """
        services_info = []

        # 1. Store模式：从缓存获取所有服务
        if not agent_mode and (not id or id == self.client_manager.global_agent_store_id):
            agent_id = self.client_manager.global_agent_store_id

            # 🔧 关键：纯缓存获取
            service_names = self.registry.get_all_service_names(agent_id)

            if not service_names:
                # 缓存为空，可能需要初始化
                logger.info("Cache is empty, you may need to add services first")
                return []

            for service_name in service_names:
                # 从缓存获取完整信息
                complete_info = self.registry.get_complete_service_info(agent_id, service_name)

                # 构建 ServiceInfo
                state = complete_info.get("state", "disconnected")
                # 确保状态是ServiceConnectionState枚举
                if isinstance(state, str):
                    try:
                        state = ServiceConnectionState(state)
                    except ValueError:
                        state = ServiceConnectionState.DISCONNECTED

                service_info = ServiceInfo(
                    url=complete_info.get("config", {}).get("url", ""),
                    name=service_name,
                    transport_type=self._infer_transport_type(complete_info.get("config", {})),
                    status=state,
                    tool_count=complete_info.get("tool_count", 0),
                    keep_alive=complete_info.get("config", {}).get("keep_alive", False),
                    working_dir=complete_info.get("config", {}).get("working_dir"),
                    env=complete_info.get("config", {}).get("env"),
                    last_heartbeat=complete_info.get("last_heartbeat"),
                    command=complete_info.get("config", {}).get("command"),
                    args=complete_info.get("config", {}).get("args"),
                    package_name=complete_info.get("config", {}).get("package_name"),
                    state_metadata=complete_info.get("state_metadata"),
                    last_state_change=complete_info.get("state_entered_time"),
                    client_id=complete_info.get("client_id"),  # 🔧 新增：Client ID 信息
                    config=complete_info.get("config", {})  # 🔧 [REFACTOR] 添加完整的config字段
                )
                services_info.append(service_info)

        # 2. Agent模式：从缓存获取 Agent 的服务
        elif agent_mode and id:
            service_names = self.registry.get_all_service_names(id)

            for service_name in service_names:
                complete_info = self.registry.get_complete_service_info(id, service_name)

                # Agent模式可能需要名称映射
                display_name = service_name
                if hasattr(self, '_service_mapper') and self._service_mapper:
                    display_name = self._service_mapper.to_local_name(service_name)

                # 确保状态是ServiceConnectionState枚举
                state = complete_info.get("state", "disconnected")
                if isinstance(state, str):
                    try:
                        state = ServiceConnectionState(state)
                    except ValueError:
                        state = ServiceConnectionState.DISCONNECTED

                service_info = ServiceInfo(
                    url=complete_info.get("config", {}).get("url", ""),
                    name=display_name,  # 显示本地名称
                    transport_type=self._infer_transport_type(complete_info.get("config", {})),
                    status=state,
                    tool_count=complete_info.get("tool_count", 0),
                    keep_alive=complete_info.get("config", {}).get("keep_alive", False),
                    working_dir=complete_info.get("config", {}).get("working_dir"),
                    env=complete_info.get("config", {}).get("env"),
                    last_heartbeat=complete_info.get("last_heartbeat"),
                    command=complete_info.get("config", {}).get("command"),
                    args=complete_info.get("config", {}).get("args"),
                    package_name=complete_info.get("config", {}).get("package_name"),
                    state_metadata=complete_info.get("state_metadata"),
                    last_state_change=complete_info.get("state_entered_time"),
                    client_id=complete_info.get("client_id"),
                    config=complete_info.get("config", {})  # 🔧 [REFACTOR] 添加完整的config字段
                )
                services_info.append(service_info)

        return services_info

    async def initialize_cache_from_files(self):
        """启动时从文件初始化缓存"""
        try:
            logger.info("🔄 [INIT_CACHE] 开始从持久化文件初始化缓存...")

            # 1. 从 ClientManager 同步基础数据
            logger.info("🔄 [INIT_CACHE] 步骤1: 从ClientManager同步基础数据...")
            self.cache_manager.sync_from_client_manager(self.client_manager)
            logger.info("✅ [INIT_CACHE] 步骤1完成: ClientManager数据同步完成")

            # 2. 从配置文件同步 Store 级别的服务
            import os
            config_path = getattr(self.config, 'config_path', None) or getattr(self.config, 'json_path', None)
            if config_path and os.path.exists(config_path):
                store_config = self.config.load_config()
                for service_name, service_config in store_config.get("mcpServers", {}).items():
                    # 添加到缓存但不连接
                    from mcpstore.core.models.service import ServiceConnectionState
                    self.registry.add_service(
                        agent_id=self.client_manager.global_agent_store_id,
                        name=service_name,
                        session=None,
                        tools=[],
                        service_config=service_config,
                        state=ServiceConnectionState.INITIALIZING
                    )

                    # 🔧 关键修复：同时添加到生命周期管理器
                    if hasattr(self, 'orchestrator') and self.orchestrator and hasattr(self.orchestrator, 'lifecycle_manager'):
                        self.orchestrator.lifecycle_manager.initialize_service(
                            self.client_manager.global_agent_store_id, service_name, service_config
                        )

            # 3. 标记缓存已初始化
            from datetime import datetime
            self.registry.cache_sync_status["initialized"] = datetime.now()

            logger.info("✅ Cache initialization completed")

        except Exception as e:
            logger.error(f"❌ Cache initialization failed: {e}")
            # 初始化失败不应该阻止系统启动

    def _setup_api_store_instance(self):
        """设置API使用的store实例"""
        # 将当前store实例设置为全局实例，供API使用
        import mcpstore.scripts.api_app as api_app
        api_app._global_store_instance = self
        logger.info(f"Set global store instance: data_space={self.is_using_data_space()}, workspace={self.get_workspace_dir()}")
        logger.info(f"Global instance id: {id(self)}, api module instance id: {id(api_app._global_store_instance)}")

    async def list_tools(self, id: Optional[str] = None, agent_mode: bool = False) -> List[ToolInfo]:
        """
        列出工具列表：
        - store未传id 或 id==global_agent_store：聚合 global_agent_store 下所有 client_id 的工具
        - store传普通 client_id：只查该 client_id 下的工具
        - agent级别：聚合 agent_id 下所有 client_id 的工具；如果 id 不是 agent_id，尝试作为 client_id 查
        """
        from mcpstore.core.client_manager import ClientManager
        client_manager: ClientManager = self.client_manager
        tools = []
        # 1. store未传id 或 id==global_agent_store，聚合 global_agent_store 下所有 client_id 的工具
        if not agent_mode and (not id or id == self.client_manager.global_agent_store_id):
            # 🔧 修复：直接从Registry缓存获取工具，而不是通过ClientManager
            agent_id = self.client_manager.global_agent_store_id
            self.logger.debug(f"🔧 [STORE.LIST_TOOLS] 直接从Registry缓存获取工具，agent_id={agent_id}")

            # 直接从tool_cache获取所有工具
            tool_cache = self.registry.tool_cache.get(agent_id, {})
            self.logger.debug(f"🔧 [STORE.LIST_TOOLS] Registry中的工具数量: {len(tool_cache)}")

            for tool_name, tool_def in tool_cache.items():
                # 获取工具对应的session来确定service_name
                session = self.registry.tool_to_session_map.get(agent_id, {}).get(tool_name)
                service_name = None

                # 通过session找到service_name
                for svc_name, svc_session in self.registry.sessions.get(agent_id, {}).items():
                    if svc_session is session:
                        service_name = svc_name
                        break

                # 🔧 获取该服务对应的client_id
                service_client_id = self._get_client_id_for_service(agent_id, service_name)

                # 构造ToolInfo对象
                if isinstance(tool_def, dict) and "function" in tool_def:
                    function_data = tool_def["function"]
                    tools.append(ToolInfo(
                        name=tool_name,
                        description=function_data.get("description", ""),
                        service_name=service_name or "unknown",
                        client_id=service_client_id,  # 🎯 使用正确的client_id
                        inputSchema=function_data.get("parameters", {})
                    ))
                else:
                    # 兼容其他格式
                    tools.append(ToolInfo(
                        name=tool_name,
                        description=tool_def.get("description", ""),
                        service_name=service_name or "unknown",
                        client_id=service_client_id,  # 🎯 使用正确的client_id
                        inputSchema=tool_def.get("inputSchema", {})
                    ))

            self.logger.debug(f"🔧 [STORE.LIST_TOOLS] 最终工具数量: {len(tools)}")
            return tools
        # 2. store传普通 client_id，只查该 client_id 下的工具
        if not agent_mode and id:
            if id == self.client_manager.global_agent_store_id:
                return tools
            tool_dicts = self.registry.get_all_tool_info(id)
            for tool in tool_dicts:
                # 使用存储的键名作为显示名称（现在键名就是显示名称）
                display_name = tool.get("name", "")
                tools.append(ToolInfo(
                    name=display_name,
                    description=tool.get("description", ""),
                    service_name=tool.get("service_name", ""),
                    client_id=tool.get("client_id", ""),
                    inputSchema=tool.get("inputSchema", {})
                ))
            return tools
        # 3. agent级别，聚合 agent_id 下所有 client_id 的工具；如果 id 不是 agent_id，尝试作为 client_id 查
        if agent_mode and id:
            # 🔧 修复：Agent模式也直接从Registry缓存获取工具
            self.logger.debug(f"🔧 [STORE.LIST_TOOLS] Agent模式，直接从Registry缓存获取工具，agent_id={id}")

            # 直接从tool_cache获取所有工具
            tool_cache = self.registry.tool_cache.get(id, {})
            self.logger.debug(f"🔧 [STORE.LIST_TOOLS] Agent模式Registry中的工具数量: {len(tool_cache)}")

            for tool_name, tool_def in tool_cache.items():
                # 获取工具对应的session来确定service_name
                session = self.registry.tool_to_session_map.get(id, {}).get(tool_name)
                service_name = None

                # 通过session找到service_name
                for svc_name, svc_session in self.registry.sessions.get(id, {}).items():
                    if svc_session is session:
                        service_name = svc_name
                        break

                # 🔧 获取该服务对应的client_id（Agent模式使用global_agent_store）
                service_client_id = self._get_client_id_for_service(self.client_manager.global_agent_store_id, service_name)

                # 构造ToolInfo对象
                if isinstance(tool_def, dict) and "function" in tool_def:
                    function_data = tool_def["function"]
                    tools.append(ToolInfo(
                        name=tool_name,
                        description=function_data.get("description", ""),
                        service_name=service_name or "unknown",
                        client_id=service_client_id,  # 🎯 使用正确的client_id
                        inputSchema=function_data.get("parameters", {})
                    ))
                else:
                    # 兼容其他格式
                    tools.append(ToolInfo(
                        name=tool_name,
                        description=tool_def.get("description", ""),
                        service_name=service_name or "unknown",
                        client_id=service_client_id,  # 🎯 使用正确的client_id
                        inputSchema=tool_def.get("inputSchema", {})
                    ))

            self.logger.debug(f"🔧 [STORE.LIST_TOOLS] Agent模式最终工具数量: {len(tools)}")
            return tools
        return tools

    async def call_tool(self, tool_name: str, args: Dict[str, Any]) -> Any:
        """
        调用工具（通用接口）

        Args:
            tool_name: 工具名称，格式为 service_toolname
            args: 工具参数

        Returns:
            Any: 工具执行结果
        """
        from mcpstore.core.models.tool import ToolExecutionRequest

        # 构造请求
        request = ToolExecutionRequest(
            tool_name=tool_name,
            args=args
        )

        # 处理工具请求
        return await self.process_tool_request(request)

    async def use_tool(self, tool_name: str, args: Dict[str, Any]) -> Any:
        """
        使用工具（通用接口）- 向后兼容别名

        注意：此方法是 call_tool 的别名，保持向后兼容性。
        推荐使用 call_tool 方法，与 FastMCP 命名保持一致。
        """
        return await self.call_tool(tool_name, args)

    async def _add_service(self, service_names: List[str], agent_id: Optional[str]) -> bool:
        """内部方法：批量添加服务，store级别支持全量注册，agent级别支持指定服务注册"""
        # store级别
        if agent_id is None:
            if not service_names:
                # 全量注册：使用统一同步机制
                if hasattr(self.orchestrator, 'sync_manager') and self.orchestrator.sync_manager:
                    sync_results = await self.orchestrator.sync_manager.sync_global_agent_store_from_mcp_json()
                    return bool(sync_results.get("added") or sync_results.get("updated"))
                else:
                    # 回退到旧方法（带警告）
                    resp = await self.register_all_services_for_store()
                    return bool(resp and resp.service_names)
            else:
                # 支持单独添加服务
                resp = await self.register_selected_services_for_store(service_names)
                return bool(resp and resp.service_names)
        # agent级别
        else:
            if service_names:
                resp = await self.register_services_for_agent(agent_id, service_names)
                return bool(resp and resp.service_names)
            else:
                self.logger.warning("Agent级别添加服务时必须指定service_names")
                return False

    async def add_service(self, service_names: List[str], agent_id: Optional[str] = None) -> bool:
        context = self.for_agent(agent_id) if agent_id else self.for_store()
        return await context.add_service(service_names)

    def check_services(self, agent_id: Optional[str] = None) -> Dict[str, str]:
        """兼容旧版API"""
        context = self.for_agent(agent_id) if agent_id else self.for_store()
        return context.check_services()

    def show_mcpjson(self) -> Dict[str, Any]:
        # TODO:show_mcpjson和get_json_config是否有一定程度的重合
        """
        直接读取并返回 mcp.json 文件的内容

        Returns:
            Dict[str, Any]: mcp.json 文件的内容
        """
        return self.config.load_config()

    # === 数据空间管理接口 ===

    def get_data_space_info(self) -> Optional[Dict[str, Any]]:
        """
        获取数据空间信息

        Returns:
            Dict: 数据空间信息，如果未使用数据空间则返回None
        """
        if self._data_space_manager:
            return self._data_space_manager.get_workspace_info()
        return None

    def get_workspace_dir(self) -> Optional[str]:
        """
        获取工作空间目录路径

        Returns:
            str: 工作空间目录路径，如果未使用数据空间则返回None
        """
        if self._data_space_manager:
            return str(self._data_space_manager.workspace_dir)
        return None

    def is_using_data_space(self) -> bool:
        """
        检查是否使用了数据空间

        Returns:
            bool: 是否使用数据空间
        """
        return self._data_space_manager is not None

    def start_api_server(self,
                        host: str = "0.0.0.0",
                        port: int = 18200,
                        reload: bool = False,
                        log_level: str = "info",
                        auto_open_browser: bool = False,
                        show_startup_info: bool = True) -> None:
        """
        启动API服务器

        这个方法会启动一个HTTP API服务器，提供RESTful接口来访问当前MCPStore实例的功能。
        服务器会自动使用当前store的配置和数据空间。

        Args:
            host: 服务器监听地址，默认"0.0.0.0"（所有网络接口）
            port: 服务器监听端口，默认18200
            reload: 是否启用自动重载（开发模式），默认False
            log_level: 日志级别，可选值: "critical", "error", "warning", "info", "debug", "trace"
            auto_open_browser: 是否自动打开浏览器，默认False
            show_startup_info: 是否显示启动信息，默认True

        Note:
            - 此方法会阻塞当前线程直到服务器停止
            - 使用Ctrl+C可以优雅地停止服务器
            - 如果使用了数据空间，API会自动使用对应的工作空间
            - 本地服务的子进程会被正确管理和清理

        Example:
            # 基本使用
            store = MCPStore.setup_store("./my_workspace/mcp.json")
            store.start_api_server()

            # 开发模式
            store.start_api_server(reload=True, auto_open_browser=True)

            # 自定义配置
            store.start_api_server(host="localhost", port=8080, log_level="debug")
        """
        try:
            import uvicorn
            import webbrowser
            from pathlib import Path

            logger.info(f"Starting API server for store: data_space={self.is_using_data_space()}")

            if show_startup_info:
                print("🚀 Starting MCPStore API Server...")
                print(f"   Host: {host}:{port}")
                if self.is_using_data_space():
                    workspace_dir = self.get_workspace_dir()
                    print(f"   Data Space: {workspace_dir}")
                    print(f"   MCP Config: {self.config.json_path}")
                else:
                    print(f"   MCP Config: {self.config.json_path}")

                if reload:
                    print("   Mode: Development (auto-reload enabled)")
                else:
                    print("   Mode: Production")

                print("   Press Ctrl+C to stop")
                print()

            # 设置全局store实例供API使用（在启动服务器之前）
            self._setup_api_store_instance()
            logger.info(f"Global store instance set for API: {type(self).__name__}")

            # 自动打开浏览器
            if auto_open_browser:
                import threading
                import time

                def open_browser():
                    time.sleep(2)  # 等待服务器启动
                    try:
                        webbrowser.open(f"http://{host if host != '0.0.0.0' else 'localhost'}:{port}")
                    except Exception as e:
                        if show_startup_info:
                            print(f"⚠️ Failed to open browser: {e}")

                threading.Thread(target=open_browser, daemon=True).start()

            # 启动API服务器
            # 不使用factory模式，直接创建app实例以保持全局变量
            from mcpstore.scripts.api_app import create_app
            app = create_app()

            uvicorn.run(
                app,
                host=host,
                port=port,
                reload=reload,
                log_level=log_level
            )

        except KeyboardInterrupt:
            if show_startup_info:
                print("\n🛑 Server stopped by user")
        except ImportError as e:
            raise RuntimeError(
                "Failed to import required dependencies for API server. "
                "Please install uvicorn: pip install uvicorn"
            ) from e
        except Exception as e:
            if show_startup_info:
                print(f"❌ Failed to start server: {e}")
            raise

    def _setup_api_store_instance(self):
        """设置API使用的store实例"""
        # 将当前store实例设置为全局实例，供API使用
        import mcpstore.scripts.api_app as api_app
        api_app._global_store_instance = self
        logger.info(f"Set global store instance: data_space={self.is_using_data_space()}, workspace={self.get_workspace_dir()}")
        logger.info(f"Global instance id: {id(self)}, api module instance id: {id(api_app._global_store_instance)}")

    def _get_client_id_for_service(self, agent_id: str, service_name: str) -> str:
        """获取服务对应的client_id"""
        try:
            # 1. 从agent_clients映射中查找
            client_ids = self.registry.get_agent_clients_from_cache(agent_id)
            if not client_ids:
                self.logger.warning(f"No client_ids found for agent {agent_id}")
                return ""

            # 2. 遍历每个client_id，查找包含该服务的client
            for client_id in client_ids:
                client_config = self.registry.client_configs.get(client_id, {})
                if service_name in client_config.get("mcpServers", {}):
                    return client_id

            # 3. 如果没找到，返回第一个client_id作为默认值
            if client_ids:
                self.logger.warning(f"Service {service_name} not found in any client config, using first client_id: {client_ids[0]}")
                return client_ids[0]

            return ""
        except Exception as e:
            self.logger.error(f"Error getting client_id for service {service_name}: {e}")
            return ""
