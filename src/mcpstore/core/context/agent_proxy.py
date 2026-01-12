"""
AgentProxy - objectified agent-view proxy.
Lightweight, stateless handle bound to a specific agent_id.
Delegates to existing context/mixins/registry for all operations.
"""

import logging
from typing import Any, Dict, TYPE_CHECKING

if TYPE_CHECKING:
    from .base_context import MCPStoreContext
    from .service_proxy import ServiceProxy
    from .tool_proxy import ToolProxy

logger = logging.getLogger(__name__)


class AgentProxy:
    """
    Proxy object for agent-specific operations.

    Provides a unified interface for managing agent-level services, tools,
    and operations with proper context isolation and caching.
    """

    def __init__(self, context: "MCPStoreContext", agent_id: str):
        """
        Initialize AgentProxy with context and agent identifier.

        Args:
            context: The MCPStoreContext instance for operations
            agent_id: Unique identifier for this agent
        """
        self._context = context
        self._agent_id = agent_id
        # Use the provided context directly instead of creating a duplicate
        self._agent_ctx = context

    # ---- Identity ----
    def get_id(self) -> str:
        return self._agent_id

    # ---- Info & stats ----
    def get_info(self) -> Dict[str, Any]:
        # Compose a lightweight info dict; metadata fields may be None
        return {
            "agent_id": self._agent_id,
            "name": None,
            "description": None,
            "created_at": None,
            "last_active": None,
            "metadata": None,
        }

    def get_stats(self) -> Dict[str, Any]:
        raise RuntimeError("[AGENT_PROXY] Synchronous get_stats is disabled, please use get_stats_async.")

    async def get_stats_async(self) -> Dict[str, Any]:
        """异步获取 Agent 统计，供异步场景和 FastAPI 使用。"""
        try:
            stats = await self._context._get_agent_statistics(self._agent_id)
            if hasattr(stats, "__dict__"):
                d = dict(stats.__dict__)
                services = d.get("services", [])
                d["services"] = [s.__dict__ if hasattr(s, "__dict__") else s for s in services]
                return d
            return stats
        except Exception:
            return {
                "agent_id": self._agent_id,
                "service_count": 0,
                "tool_count": 0,
                "healthy_services": 0,
                "unhealthy_services": 0,
                "total_tool_executions": 0,
                "is_active": False,
                "last_activity": None,
                "services": [],
            }

    def find_cache(self) -> "CacheProxy":
        from .cache_proxy import CacheProxy
        return CacheProxy(self._context, scope="agent", scope_value=self._agent_id)

    # ---- Services & tools ----
    def list_services(self):
        """
        列出 Agent 视角的服务，直接返回 ServiceInfo 列表
        """
        ctx = self._agent_ctx or self._context
        return ctx.list_services()

    def find_service(self, name: str) -> "ServiceProxy":
        """
        查找服务并返回服务代理对象
        
        验证服务归属于当前 Agent
        
        Args:
            name: 服务名称（本地名称）
        
        Returns:
            ServiceProxy: 绑定到当前 Agent 的服务代理对象
        
        Raises:
            ServiceNotFoundException: 服务不存在
            ServiceBindingError: 服务不属于当前 Agent
        
        Validates: Requirements 6.6, 6.7 (服务归属验证)
        """
        from .service_proxy import ServiceProxy
        from mcpstore.core.exceptions import ServiceNotFoundException, ServiceBindingError
        
        ctx = self._agent_ctx or self._context
        
        # 验证服务归属
        try:
            verified, global_name = self._verify_service_ownership(name)
            if not verified:
                raise ServiceBindingError(
                    service_name=name,
                    agent_id=self._agent_id,
                    reason="服务不属于当前 Agent"
                )
            
            # 创建 ServiceProxy 时传入 agent_id 和 global_name
            return ServiceProxy(
                ctx,
                name,
                agent_id=self._agent_id,
                global_name=global_name
            )
        except ServiceNotFoundException:
            raise
        except ServiceBindingError:
            raise
        except Exception as e:
            logger.error(f"[AGENT_PROXY] Failed to find service '{name}': {e}")
            raise ServiceNotFoundException(service_name=name, agent_id=self._agent_id)
    
    def _verify_service_ownership(self, service_name: str) -> tuple[bool, str]:
        """
        验证服务归属于当前 Agent
        
        Args:
            service_name: 服务名称（本地名称）
        
        Returns:
            tuple[bool, str]: (是否验证通过, 全局服务名称)
        
        Raises:
            ServiceNotFoundException: 服务不存在
        
        Validates: Requirements 6.6, 6.7 (服务归属验证)
        """
        from mcpstore.core.exceptions import ServiceNotFoundException
        
        ctx = self._agent_ctx or self._context
        
        # 通过 Registry 验证服务映射
        try:
            # 使用 Registry 获取服务的全局名称
            global_name = ctx._store.registry.get_global_name_from_agent_service(
                self._agent_id,
                service_name
            )
            
            if not global_name:
                raise ServiceNotFoundException(
                    service_name=service_name,
                    agent_id=self._agent_id
                )
            
            logger.debug(f"[AGENT_PROXY] Verified ownership of service '{service_name}' for agent '{self._agent_id}'")
            return True, global_name
            
        except ServiceNotFoundException:
            raise
        except Exception as e:
            logger.error(f"[AGENT_PROXY] Failed to verify service ownership: {e}")
            raise ServiceNotFoundException(
                service_name=service_name,
                agent_id=self._agent_id
            )

    def list_tools(
        self,
        service_name: str = None,
        *,
        filter: str = "available"
    ):
        """
        列出工具
        
        Args:
            service_name: 服务名称(可选)
            filter: 筛选范围 ("available" 或 "all")
        
        Returns:
            工具列表（ToolInfo 对象列表）
        """
        ctx = self._agent_ctx or self._context
        return ctx.list_tools(service_name=service_name, filter=filter)

    # ---- Health & runtime ----
    def check_services(self) -> Dict[str, Any]:
        raise RuntimeError("[AGENT_PROXY] Synchronous check_services is disabled, please use check_services_async.")

    def call_tool(self, tool_name: str, args: Dict[str, Any]) -> Dict[str, Any]:
        # 为兼容同步示例，桥接到异步实现；若当前线程已有事件循环，会抛出异常提示使用异步
        import asyncio
        try:
            loop = asyncio.get_running_loop()
            if loop.is_running():
                raise RuntimeError("[AGENT_PROXY] Current thread already has an event loop, please use call_tool_async.")
        except RuntimeError:
            pass  # 无运行中的 loop，可安全使用 asyncio.run

        return asyncio.run(self.call_tool_async(tool_name, args))

    # ---- Mutations ----
    def add_service(self, config: Dict[str, Any]) -> bool:
        """
        同步添加服务（仅在当前线程不存在事件循环时使用）。

        - 如果当前线程已有事件循环，会提醒使用 add_service_async 以避免 AOB 冲突。
        - 在普通同步脚本中，可直接调用；内部通过 asyncio.run 执行异步逻辑。
        """
        return self.add_service_blocking(config)

    def add_service_blocking(self, *args, **kwargs) -> bool:
        """
        便捷同步包装：在当前线程没有事件循环时，阻塞调用 add_service_async。
        若线程已有事件循环，仍需显式使用 add_service_async 以避免死锁。
        """
        import asyncio

        try:
            loop = asyncio.get_running_loop()
            if loop.is_running():
                raise RuntimeError("[AGENT_PROXY] Current thread already has an event loop, please use add_service_async.")
        except RuntimeError:
            # 没有运行中的事件循环，安全使用 asyncio.run
            return asyncio.run(self.add_service_async(*args, **kwargs))

        # 理论上不会走到这里
        return False

    def update_service(self, name: str, patch: Dict[str, Any]) -> bool:
        raise RuntimeError("[AGENT_PROXY] Synchronous update_service is disabled, please use update_service_async.")

    def delete_service(self, name: str) -> bool:
        raise RuntimeError("[AGENT_PROXY] Synchronous delete_service is disabled, please use delete_service_async.")

    # Async counterparts (explicit wrappers)
    async def add_service_async(self, *args, **kwargs):
        ctx = self._agent_ctx or self._context
        return await ctx.add_service_async(*args, **kwargs)

    async def call_tool_async(self, tool_name: str, args: Dict[str, Any]):
        ctx = self._agent_ctx or self._context
        return await ctx.call_tool_async(tool_name, args)

    async def show_config_async(self) -> Dict[str, Any]:
        ctx = self._agent_ctx or self._context
        return await ctx.show_config_async()

    async def delete_config_async(self, client_id_or_service_name: str) -> Dict[str, Any]:
        ctx = self._agent_ctx or self._context
        return await ctx.delete_config_async(client_id_or_service_name)

    async def update_config_async(self, client_id_or_service_name: str, new_config: Dict[str, Any]) -> Dict[str, Any]:
        ctx = self._agent_ctx or self._context
        return await ctx.update_config_async(client_id_or_service_name, new_config)

    async def reset_config_async(self) -> bool:
        ctx = self._agent_ctx or self._context
        return await ctx.reset_config_async()

    async def get_tool_records_async(self, limit: int = 50) -> Dict[str, Any]:
        ctx = self._agent_ctx or self._context
        return await ctx.get_tool_records_async(limit)

    # ---- Service info/status & extended ops ----
    def get_service_info(self, name: str) -> Dict[str, Any]:
        ctx = self._agent_ctx or self._context
        info = ctx.get_service_info(name)
        try:
            if hasattr(info, "model_dump"):
                return info.model_dump()
            if hasattr(info, "dict"):
                return info.dict()
            if isinstance(info, dict):
                return info
            return {"result": str(info)}
        except Exception:
            return {"result": str(info)}

    def get_service_status(self, name: str) -> Dict[str, Any]:
        ctx = self._agent_ctx or self._context
        status = ctx.get_service_status(name)
        try:
            if hasattr(status, "model_dump"):
                return status.model_dump()
            if hasattr(status, "dict"):
                return status.dict()
            if isinstance(status, dict):
                return status
            return {"result": str(status)}
        except Exception:
            return {"result": str(status)}

    # 别名：符合命名规范
    def service_info(self, name: str) -> Dict[str, Any]:
        return self.get_service_info(name)

    def service_status(self, name: str) -> Dict[str, Any]:
        return self.get_service_status(name)

    async def service_info_async(self, name: str) -> Dict[str, Any]:
        return await self.get_service_info_async(name)

    async def service_status_async(self, name: str) -> Dict[str, Any]:
        return await self.get_service_status_async(name)

    async def tool_records_async(self, limit: int = 50) -> Dict[str, Any]:
        ctx = self._agent_ctx or self._context
        return await ctx.tool_records_async(limit)


    def patch_service(self, name: str, updates: Dict[str, Any]) -> bool:
        ctx = self._agent_ctx or self._context
        return bool(ctx.patch_service(name, updates))

    async def patch_service_async(self, name: str, updates: Dict[str, Any]) -> bool:
        ctx = self._agent_ctx or self._context
        return await ctx.patch_service_async(name, updates)

    def restart_service(self, name: str) -> bool:
        raise RuntimeError("[AGENT_PROXY] Synchronous restart_service is disabled, please use restart_service_async.")

    async def restart_service_async(self, name: str) -> bool:
        ctx = self._agent_ctx or self._context
        return await ctx.restart_service_async(name)

    def use_tool(self, tool_name: str, args: Any = None, **kwargs) -> Any:
        raise RuntimeError("[AGENT_PROXY] Synchronous use_tool is disabled, please use call_tool_async.")

    async def check_services_async(self) -> Dict[str, Any]:
        ctx = self._agent_ctx or self._context
        return await ctx.check_services_async()

    async def get_service_info_async(self, name: str) -> Dict[str, Any]:
        ctx = self._agent_ctx or self._context
        info = await ctx.get_service_info_async(name)
        try:
            if hasattr(info, "model_dump"):
                return info.model_dump()
            if hasattr(info, "dict"):
                return info.dict()
            if isinstance(info, dict):
                return info
            return {"result": str(info)}
        except Exception:
            return {"result": str(info)}

    async def get_service_status_async(self, name: str) -> Dict[str, Any]:
        ctx = self._agent_ctx or self._context
        status = await ctx.get_service_status_async(name)
        try:
            if hasattr(status, "model_dump"):
                return status.model_dump()
            if hasattr(status, "dict"):
                return status.dict()
            if isinstance(status, dict):
                return status
            return {"result": str(status)}
        except Exception:
            return {"result": str(status)}

    # ---- Name mapping ----
    def map_local(self, name: str) -> str:
        from .agent_service_mapper import AgentServiceMapper
        # If global name, try rsplit to extract local
        if AgentServiceMapper.is_any_agent_service(name):
            try:
                parts = name.rsplit("_byagent_", 1)
                return parts[0] if len(parts) == 2 else name
            except Exception:
                return name
        return name

    def map_global(self, name: str) -> str:
        from .agent_service_mapper import AgentServiceMapper
        return AgentServiceMapper(self._agent_id).to_global_name(name)

    # ---- Adapters (delegations) ----
    def for_langchain(self, response_format: str = "text"):
        ctx = self._agent_ctx or self._context
        return ctx.for_langchain(response_format=response_format)

    def for_llamaindex(self):
        ctx = self._agent_ctx or self._context
        return ctx.for_llamaindex()

    def for_crewai(self):
        ctx = self._agent_ctx or self._context
        return ctx.for_crewai()

    def for_langgraph(self, response_format: str = "text"):
        ctx = self._agent_ctx or self._context
        return ctx.for_langgraph(response_format=response_format)

    def for_autogen(self):
        ctx = self._agent_ctx or self._context
        return ctx.for_autogen()

    def for_semantic_kernel(self):
        ctx = self._agent_ctx or self._context
        return ctx.for_semantic_kernel()

    def for_openai(self):
        ctx = self._agent_ctx or self._context
        return ctx.for_openai()

    # ---- Hub MCP helpers ----
    def hub_http(self, port: int = 8000, host: str = "0.0.0.0", path: str = "/mcp", *, block: bool = False, show_banner: bool = False, **fastmcp_kwargs):
        """
        将当前 Agent 暴露为 HTTP MCP 端点。

        Args:
            port: 监听端口
            host: 监听地址
            path: HTTP 路径
            background: 是否在后台线程运行
            show_banner: 是否显示 FastMCP 启动横幅
            **fastmcp_kwargs: 透传给 FastMCP 的参数
        """
        from mcpstore.core.hub.server import HubMCPServer

        hub = HubMCPServer(
            exposed_object=self._agent_ctx or self._context,
            transport="http",
            port=port,
            host=host,
            path=path,
            **fastmcp_kwargs,
        )
        hub.start(block=block, show_banner=show_banner)
        return hub

    def hub_sse(self, port: int = 8000, host: str = "0.0.0.0", path: str = "/sse", *, block: bool = False, show_banner: bool = False, **fastmcp_kwargs):
        """将当前 Agent 暴露为 SSE MCP 端点。"""
        from mcpstore.core.hub.server import HubMCPServer

        hub = HubMCPServer(
            exposed_object=self._agent_ctx or self._context,
            transport="sse",
            port=port,
            host=host,
            path=path,
            **fastmcp_kwargs,
        )
        hub.start(block=block, show_banner=show_banner)
        return hub

    def hub_stdio(self, *, block: bool = False, show_banner: bool = False, **fastmcp_kwargs):
        """将当前 Agent 暴露为 stdio MCP 端点。"""
        from mcpstore.core.hub.server import HubMCPServer

        hub = HubMCPServer(
            exposed_object=self._agent_ctx or self._context,
            transport="stdio",
            **fastmcp_kwargs,
        )
        hub.start(block=block, show_banner=show_banner)
        return hub

    # ---- Sessions (delegations) ----
    def with_session(self, session_id: str):
        ctx = self._agent_ctx or self._context
        return ctx.with_session(session_id)

    async def with_session_async(self, session_id: str):
        ctx = self._agent_ctx or self._context
        return await ctx.with_session_async(session_id)

    def create_session(self, session_id: str, user_session_id: str = None):
        ctx = self._agent_ctx or self._context
        return ctx.create_session(session_id, user_session_id)

    def find_session(self, session_id: str = None, is_user_session_id: bool = False):
        ctx = self._agent_ctx or self._context
        return ctx.find_session(session_id, is_user_session_id)

    def get_session(self, session_id: str):
        ctx = self._agent_ctx or self._context
        return ctx.get_session(session_id)

    def list_sessions(self):
        ctx = self._agent_ctx or self._context
        return ctx.list_sessions()

    def close_all_sessions(self):
        ctx = self._agent_ctx or self._context
        return ctx.close_all_sessions()

    def cleanup_sessions(self):
        ctx = self._agent_ctx or self._context
        return ctx.cleanup_sessions()

    def restart_sessions(self):
        ctx = self._agent_ctx or self._context
        return ctx.restart_sessions()

    def find_user_session(self, user_session_id: str):
        ctx = self._agent_ctx or self._context
        return ctx.find_user_session(user_session_id)

    def create_shared_session(self, session_id: str, shared_id: str):
        ctx = self._agent_ctx or self._context
        return ctx.create_shared_session(session_id, shared_id)

    # ---- Lifecycle / waiters ----
    def wait_service(self, client_id_or_service_name: str, status = 'healthy', timeout: float = 10.0, raise_on_timeout: bool = False) -> bool:
        ctx = self._agent_ctx or self._context
        return ctx.wait_service(client_id_or_service_name, status, timeout, raise_on_timeout)

    async def wait_service_async(self, client_id_or_service_name: str, status = 'healthy', timeout: float = 10.0, raise_on_timeout: bool = False) -> bool:
        ctx = self._agent_ctx or self._context
        return await ctx.wait_service_async(client_id_or_service_name, status, timeout, raise_on_timeout)

    def init_service(self, client_id_or_service_name: str = None, *, client_id: str = None, service_name: str = None):
        ctx = self._agent_ctx or self._context
        return ctx.init_service(client_id_or_service_name, client_id=client_id, service_name=service_name)

    async def init_service_async(self, client_id_or_service_name: str = None, *, client_id: str = None, service_name: str = None):
        ctx = self._agent_ctx or self._context
        return await ctx.init_service_async(client_id_or_service_name, client_id=client_id, service_name=service_name)

    # ---- Advanced features ----
    def import_api(self, api_url: str, api_name: str = None):
        ctx = self._agent_ctx or self._context
        return ctx.import_api(api_url, api_name)

    async def import_api_async(self, api_url: str, api_name: str = None):
        ctx = self._agent_ctx or self._context
        return await ctx.import_api_async(api_url, api_name)

    def reset_mcp_json_file(self) -> bool:
        ctx = self._agent_ctx or self._context
        return ctx.reset_mcp_json_file()

    async def reset_mcp_json_file_async(self, scope: str = "all") -> bool:
        ctx = self._agent_ctx or self._context
        return await ctx.reset_mcp_json_file_async(scope)

    # ---- Tool lookup ----
    def find_tool(self, tool_name: str):
        from .tool_proxy import ToolProxy
        return ToolProxy(self._agent_ctx or self._context, tool_name, scope='context')

    # ---- 工具集管理方法 ----
    def add_tools(self, service, tools) -> 'AgentProxy':
        """
        添加工具到当前可用集合
        
        Args:
            service: 服务标识（服务名称、ServiceProxy 或 "_all_services"）
            tools: 工具标识（工具名称列表或 "_all_tools"）
        
        Returns:
            self (支持链式调用)
        """
        ctx = self._agent_ctx or self._context
        ctx.add_tools(service=service, tools=tools)
        return self

    def remove_tools(self, service, tools) -> 'AgentProxy':
        """
        从当前可用集合移除工具
        
        Args:
            service: 服务标识（服务名称、ServiceProxy 或 "_all_services"）
            tools: 工具标识（工具名称列表或 "_all_tools"）
        
        Returns:
            self (支持链式调用)
        """
        ctx = self._agent_ctx or self._context
        ctx.remove_tools(service=service, tools=tools)
        return self

    def reset_tools(self, service) -> 'AgentProxy':
        """
        重置服务的工具集为默认状态
        
        Args:
            service: 服务标识（服务名称、ServiceProxy 或 "_all_services"）
        
        Returns:
            self (支持链式调用)
        """
        ctx = self._agent_ctx or self._context
        ctx.reset_tools(service=service)
        return self

    def get_tool_set_info(self, service) -> Dict[str, Any]:
        """
        获取服务的工具集信息
        
        Args:
            service: 服务标识（服务名称或 ServiceProxy）
        
        Returns:
            工具集信息字典
        """
        ctx = self._agent_ctx or self._context
        return ctx.get_tool_set_info(service=service)

    def get_tool_set_summary(self) -> Dict[str, Any]:
        """
        获取工具集摘要
        
        Returns:
            摘要信息字典
        """
        ctx = self._agent_ctx or self._context
        return ctx.get_tool_set_summary()

    # ---- Resources & Prompts ----
    def list_resources(self, service_name: str = None) -> Dict[str, Any]:
        ctx = self._agent_ctx or self._context
        return ctx.list_resources(service_name)

    def list_resource_templates(self, service_name: str = None) -> Dict[str, Any]:
        ctx = self._agent_ctx or self._context
        return ctx.list_resource_templates(service_name)

    def read_resource(self, uri: str, service_name: str = None) -> Dict[str, Any]:
        ctx = self._agent_ctx or self._context
        return ctx.read_resource(uri, service_name)

    def list_prompts(self, service_name: str = None) -> Dict[str, Any]:
        ctx = self._agent_ctx or self._context
        return ctx.list_prompts(service_name)

    def get_prompt(self, name: str, arguments: Dict[str, Any] = None, service_name: str = None) -> Dict[str, Any]:
        ctx = self._agent_ctx or self._context
        return ctx.get_prompt(name, arguments, service_name)

    def list_changed_tools(self, service_name: str = None, force_refresh: bool = False) -> Dict[str, Any]:
        ctx = self._agent_ctx or self._context
        return ctx.list_changed_tools(service_name, force_refresh)

    # ---- Config management ----
    def reset_config(self) -> bool:
        ctx = self._agent_ctx or self._context
        return bool(ctx.reset_config())

    def show_config(self) -> Dict[str, Any]:
        ctx = self._agent_ctx or self._context
        return ctx.show_config()

    # ---- Escape hatch ----
    def get_context(self):
        return self._agent_ctx or self._context

    # ---- Compatibility: delegate unknown attrs to agent-scoped context ----
    def __getattr__(self, name: str):
        target = self._agent_ctx or self._context
        return getattr(target, name)
