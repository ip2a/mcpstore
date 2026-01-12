"""
StoreProxy - objectified store-view proxy.
Lightweight, stateless handle that delegates to the underlying context.
All data is retrieved on demand from registry/cache to ensure freshness.
"""

from typing import Any, Dict, List, TYPE_CHECKING

from mcpstore.core.models.tool import ToolInfo

if TYPE_CHECKING:
    from .base_context import MCPStoreContext
    from .service_proxy import ServiceProxy
    from .agent_proxy import AgentProxy
    from .tool_proxy import ToolProxy


class StoreProxy:
    def __init__(self, context: "MCPStoreContext"):
        self._context = context

    # ---- Identity & info ----
    def get_id(self) -> str:
        return getattr(self._context._store.client_manager, "global_agent_store_id", "global_agent_store")

    def get_info(self) -> Dict[str, Any]:
        # Reuse setup_config snapshot as store info
        return self._context.setup_config()

    def get_stats(self) -> Dict[str, Any]:
        services = self.list_services()
        tools = self.list_tools()
        return {
            "services": len(services),
            "tools": len(tools),
        }

    # ---- Lists & queries ----
    def list_services(self, *args, **kwargs) -> List[Dict[str, Any]]:
        # 直接返回 ServiceInfo 模型列表，调用方如需 JSON 可自行 model_dump()
        return self._context.list_services(*args, **kwargs)

    def list_tools(self, *args, **kwargs):
        """
        列出工具列表
        
        直接返回 ToolInfo 对象列表，不转换为字典。
        
        Returns:
            List[ToolInfo]: 工具列表
        """
        return self._context.list_tools(*args, **kwargs)

    def find_service(self, name: str) -> "ServiceProxy":
        from .service_proxy import ServiceProxy
        return ServiceProxy(self._context, name)

    def list_agents(self) -> List[Dict[str, Any]]:
        # 同步方法，使用异步桥在统一事件循环中执行
        return self._context._run_async_via_bridge(
            self.list_agents_async(),
            op_name="store_proxy.list_agents"
        )

    async def list_agents_async(self) -> List[Dict[str, Any]]:
        registry = self._context._store.registry
        global_agent_id = self._context._store.client_manager.global_agent_store_id
        agent_ids = set(await registry.get_all_agent_ids_async() or [])
        agent_ids.add(global_agent_id)

        # 读取 Agent 元数据（可选）
        agents_entities = await registry._cache_layer_manager.get_all_entities_async("agents") or {}

        result: List[Dict[str, Any]] = []
        for agent_id in sorted(agent_ids):
            # 从 pykv 获取 Agent 客户端
            client_ids = await registry.get_agent_clients_async(agent_id)
            # Agent 服务列表（全局名）
            global_service_names = await registry.get_agent_services_async(agent_id) or []
            tool_count = 0
            healthy = 0
            unhealthy = 0
            for gname in global_service_names:
                tools = await registry.get_tools_for_service_async(global_agent_id, gname) or []
                tool_count += len(tools)
                state = await registry._service_state_service.get_service_state_async(
                    global_agent_id,
                    gname
                )
                state_value = getattr(state, "value", str(state))
                if state_value in ("healthy", "degraded"):
                    healthy += 1
                else:
                    unhealthy += 1

            agent_meta = agents_entities.get(agent_id) if isinstance(agents_entities, dict) else {}
            result.append({
                "agent_id": agent_id,
                "client_ids": client_ids,
                "service_count": len(global_service_names),
                "tool_count": tool_count,
                "healthy_services": healthy,
                "unhealthy_services": unhealthy,
                "is_active": bool(global_service_names and healthy > 0),
                "last_activity": agent_meta.get("last_active") if isinstance(agent_meta, dict) else None,
            })
        return result

    def find_cache(self) -> "CacheProxy":
        from .cache_proxy import CacheProxy
        return CacheProxy(self._context, scope="global", scope_value=None)

    def find_agent(self, agent_id: str) -> "AgentProxy":
        """
        Find agent proxy with unified caching.

        Uses the centralized AgentProxy caching system to ensure that the same
        agent_id always returns the same AgentProxy instance across all access
        methods in the MCPStore.

        Args:
            agent_id: Unique identifier for the agent

        Returns:
            AgentProxy: Cached or newly created AgentProxy instance
        """
        # Use unified AgentProxy caching system from the store
        return self._context._store._get_or_create_agent_proxy(self._context, agent_id)

    # ---- Health & runtime ----
    def check_services(self) -> Dict[str, Any]:
        return self._context.check_services()

    def call_tool(self, tool_name: str, args: Dict[str, Any]):
        """
        调用工具（同步版本），直接返回 FastMCP CallToolResult。

        需要结构化/文本化视图的调用方，应该自行从结果的 content / structured_content / data 中提取。
        """
        return self._context.call_tool(tool_name, args)

    # ---- Mutations ----
    def add_service(self, config: Dict[str, Any]) -> bool:
        return bool(self._context.add_service(config))

    def update_service(self, name: str, patch: Dict[str, Any]) -> bool:
        return bool(self._context.update_service(name, patch))

    def delete_service(self, name: str) -> bool:
        return bool(self._context.delete_service(name))

    # Async counterparts (explicit wrappers)
    async def add_service_async(self, *args, **kwargs):
        return await self._context.add_service_async(*args, **kwargs)

    async def call_tool_async(self, tool_name: str, args: Dict[str, Any]):
        return await self._context.call_tool_async(tool_name, args)

    async def show_config_async(self) -> Dict[str, Any]:
        return await self._context.show_config_async()

    async def delete_config_async(self, client_id_or_service_name: str) -> Dict[str, Any]:
        return await self._context.delete_config_async(client_id_or_service_name)

    async def reset_config_async(self) -> bool:
        return await self._context.reset_config_async()

    async def get_tool_records_async(self, limit: int = 50) -> Dict[str, Any]:
        return await self._context.get_tool_records_async(limit)

    # ---- Service info/status & extended ops ----
    def get_service_info(self, name: str) -> Dict[str, Any]:
        info = self._context.get_service_info(name)
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
        status = self._context.get_service_status(name)
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


    def patch_service(self, name: str, updates: Dict[str, Any]) -> bool:
        return bool(self._context.patch_service(name, updates))

    async def patch_service_async(self, name: str, updates: Dict[str, Any]) -> bool:
        return await self._context.patch_service_async(name, updates)

    def restart_service(self, name: str) -> bool:
        return bool(self._context.restart_service(name))

    async def restart_service_async(self, name: str) -> bool:
        return await self._context.restart_service_async(name)

    def use_tool(self, tool_name: str, args: Any = None, **kwargs) -> Any:
        # Delegate to context; leave result as-is to match use_tool semantics
        return self._context.use_tool(tool_name, args, **kwargs)

    async def check_services_async(self) -> Dict[str, Any]:
        return await self._context.check_services_async()

    # 别名：符合命名规范
    async def service_info_async(self, name: str) -> Dict[str, Any]:
        return await self.get_service_info_async(name)

    async def get_service_info_async(self, name: str) -> Dict[str, Any]:
        info = await self._context.get_service_info_async(name)
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

    async def service_status_async(self, name: str) -> Dict[str, Any]:
        return await self.get_service_status_async(name)

    async def get_service_status_async(self, name: str) -> Dict[str, Any]:
        status = await self._context.get_service_status_async(name)
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

    async def tool_records_async(self, limit: int = 50) -> Dict[str, Any]:
        return await self._context.tool_records_async(limit)

    # ---- Resources & Prompts ----
    def list_resources(self, service_name: str = None) -> Dict[str, Any]:
        return self._context.list_resources(service_name)

    def list_resource_templates(self, service_name: str = None) -> Dict[str, Any]:
        return self._context.list_resource_templates(service_name)

    def read_resource(self, uri: str, service_name: str = None) -> Dict[str, Any]:
        return self._context.read_resource(uri, service_name)

    def list_prompts(self, service_name: str = None) -> Dict[str, Any]:
        return self._context.list_prompts(service_name)

    def get_prompt(self, name: str, arguments: Dict[str, Any] = None, service_name: str = None) -> Dict[str, Any]:
        return self._context.get_prompt(name, arguments, service_name)

    def list_changed_tools(self, service_name: str = None, force_refresh: bool = False) -> Dict[str, Any]:
        return self._context.list_changed_tools(service_name, force_refresh)

    # ---- Config management ----
    def reset_config(self) -> bool:
        return bool(self._context.reset_config())

    def show_config(self) -> Dict[str, Any]:
        return self._context.show_config()

    def switch_cache(self, cache_config: Any) -> bool:
        """Runtime cache backend switching (synchronous version)."""
        return bool(self._context.switch_cache(cache_config))

    async def switch_cache_async(self, cache_config: Any) -> bool:
        """Runtime cache backend switching (asynchronous version)."""
        return await self._context.switch_cache_async(cache_config)

    # ---- Statistics ----
    def get_agents_summary(self) -> Any:
        summary = getattr(self._context, "get_agents_summary", None)
        if callable(summary):
            res = summary()
            try:
                if hasattr(res, "model_dump"):
                    return res.model_dump()
                if hasattr(res, "dict"):
                    return res.dict()
            except Exception:
                pass
            return res
        return {}

    # ---- Adapters (delegations) ----
    def for_langchain(self, response_format: str = "text"):
        return self._context.for_langchain(response_format=response_format)

    def for_llamaindex(self):
        return self._context.for_llamaindex()

    def for_crewai(self):
        return self._context.for_crewai()

    def for_langgraph(self, response_format: str = "text"):
        return self._context.for_langgraph(response_format=response_format)

    def for_autogen(self):
        return self._context.for_autogen()

    def for_semantic_kernel(self):
        return self._context.for_semantic_kernel()

    def for_openai(self):
        return self._context.for_openai()

    # ---- Sessions (delegations) ----
    def with_session(self, session_id: str):
        return self._context.with_session(session_id)

    async def with_session_async(self, session_id: str):
        return await self._context.with_session_async(session_id)

    def create_session(self, session_id: str, user_session_id: str = None):
        return self._context.create_session(session_id, user_session_id)

    def find_session(self, session_id: str = None, is_user_session_id: bool = False):
        return self._context.find_session(session_id, is_user_session_id)

    def get_session(self, session_id: str):
        return self._context.get_session(session_id)

    def list_sessions(self):
        return self._context.list_sessions()

    def close_all_sessions(self):
        return self._context.close_all_sessions()

    def cleanup_sessions(self):
        return self._context.cleanup_sessions()

    def restart_sessions(self):
        return self._context.restart_sessions()

    def find_user_session(self, user_session_id: str):
        return self._context.find_user_session(user_session_id)

    def create_shared_session(self, session_id: str, shared_id: str):
        return self._context.create_shared_session(session_id, shared_id)

    # ---- Lifecycle / waiters ----
    def wait_service(self, client_id_or_service_name: str, status = 'healthy', timeout: float = 10.0, raise_on_timeout: bool = False) -> bool:
        return self._context.wait_service(client_id_or_service_name, status, timeout, raise_on_timeout)

    async def wait_service_async(self, client_id_or_service_name: str, status = 'healthy', timeout: float = 10.0, raise_on_timeout: bool = False) -> bool:
        return await self._context.wait_service_async(client_id_or_service_name, status, timeout, raise_on_timeout)

    def init_service(self, client_id_or_service_name: str = None, *, client_id: str = None, service_name: str = None):
        return self._context.init_service(client_id_or_service_name, client_id=client_id, service_name=service_name)

    async def init_service_async(self, client_id_or_service_name: str = None, *, client_id: str = None, service_name: str = None):
        return await self._context.init_service_async(client_id_or_service_name, client_id=client_id, service_name=service_name)

    # ---- Advanced features ----
    def import_api(self, api_url: str, api_name: str = None):
        return self._context.import_api(api_url, api_name)

    async def import_api_async(self, api_url: str, api_name: str = None):
        return await self._context.import_api_async(api_url, api_name)

    def reset_mcp_json_file(self) -> bool:
        return self._context.reset_mcp_json_file()

    async def reset_mcp_json_file_async(self, scope: str = "all") -> bool:
        return await self._context.reset_mcp_json_file_async(scope)

    # ---- Hub MCP helpers ----
    def hub_http(self, port: int = 8000, host: str = "0.0.0.0", path: str = "/mcp", *, block: bool = False, show_banner: bool = False, **fastmcp_kwargs):
        """
        将当前 Store 暴露为 HTTP MCP 端点。

        Args:
            port: 监听端口
            host: 监听地址
            path: HTTP 路径
            background: 是否在后台线程运行（默认阻塞当前调用）
            show_banner: 是否显示 FastMCP 启动横幅
            **fastmcp_kwargs: 透传给 FastMCP 的参数（如 auth）
        Returns:
            HubMCPServer: Hub 服务器实例，可用于 stop()/restart()
        """
        from mcpstore.core.hub.server import HubMCPServer

        hub = HubMCPServer(
            exposed_object=self._context,
            transport="http",
            port=port,
            host=host,
            path=path,
            **fastmcp_kwargs,
        )
        hub.start(block=block, show_banner=show_banner)
        return hub

    def hub_sse(self, port: int = 8000, host: str = "0.0.0.0", path: str = "/sse", *, block: bool = False, show_banner: bool = False, **fastmcp_kwargs):
        """将当前 Store 暴露为 SSE MCP 端点。"""
        from mcpstore.core.hub.server import HubMCPServer

        hub = HubMCPServer(
            exposed_object=self._context,
            transport="sse",
            port=port,
            host=host,
            path=path,
            **fastmcp_kwargs,
        )
        hub.start(block=block, show_banner=show_banner)
        return hub

    def hub_stdio(self, *, block: bool = False, show_banner: bool = False, **fastmcp_kwargs):
        """将当前 Store 暴露为 stdio MCP 端点。"""
        from mcpstore.core.hub.server import HubMCPServer

        hub = HubMCPServer(
            exposed_object=self._context,
            transport="stdio",
            **fastmcp_kwargs,
        )
        hub.start(block=block, show_banner=show_banner)
        return hub

    # ---- Tool lookup ----
    def find_tool(self, tool_name: str):
        from .tool_proxy import ToolProxy
        return ToolProxy(self._context, tool_name, scope='context')

    # ---- Escape hatch ----
    def get_context(self):
        return self._context

    # ---- Compatibility: delegate unknown attrs to context ----
    def __getattr__(self, name: str):
        # Fallback delegation to preserve existing callsites expecting context methods
        return getattr(self._context, name)
