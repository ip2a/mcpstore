"""
AgentProxy - objectified agent-view proxy.
Lightweight, stateless handle bound to a specific agent_id.
Delegates to existing context/mixins/registry for all operations.
"""

from typing import Any, Dict, List, TYPE_CHECKING

if TYPE_CHECKING:
    from .base_context import MCPStoreContext
    from .service_proxy import ServiceProxy
    from .tool_proxy import ToolProxy


class AgentProxy:
    def __init__(self, context: "MCPStoreContext", agent_id: str):
        self._context = context
        self._agent_id = agent_id
        # Prepare an agent-scoped context for delegations to avoid relying on store.for_agent()
        try:
            from .base_context import MCPStoreContext as _Ctx
            self._agent_ctx = _Ctx(self._context._store, agent_id)
        except Exception:
            self._agent_ctx = None

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
        # Reuse AgentStatisticsMixin via context
        try:
            stats = self._context._sync_helper.run_async(self._context._get_agent_statistics(self._agent_id))
            if hasattr(stats, "__dict__"):
                d = dict(stats.__dict__)
                # Normalize dataclass-like nested services
                services = d.get("services", [])
                d["services"] = [s.__dict__ if hasattr(s, "__dict__") else s for s in services]
                return d
            return stats
        except Exception:
            # Fallback minimal stats
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

    # ---- Services & tools ----
    def list_services(self) -> List[Dict[str, Any]]:
        # Delegate to agent-scoped context for consistent style; coerce to dicts preferring model_dump
        ctx = self._agent_ctx or self._context
        items = ctx.list_services()
        result: List[Dict[str, Any]] = []
        for s in items:
            if hasattr(s, "model_dump"):
                try:
                    result.append(s.model_dump())
                    continue
                except Exception:
                    pass
            if hasattr(s, "dict"):
                try:
                    result.append(s.dict())
                    continue
                except Exception:
                    pass
            result.append(s if isinstance(s, dict) else {"value": str(s)})
        return result

    def find_service(self, name: str) -> "ServiceProxy":
        from .service_proxy import ServiceProxy
        return ServiceProxy(self._agent_ctx or self._context, name)

    def list_tools(self) -> List[Dict[str, Any]]:
        # Delegate to agent-scoped context list_tools for consistent mapping and snapshot behavior
        ctx = self._agent_ctx or self._context
        items = ctx.list_tools()
        result: List[Dict[str, Any]] = []
        for t in items:
            if isinstance(t, dict):
                result.append(t)
                continue
            if hasattr(t, "model_dump"):
                try:
                    result.append(t.model_dump())
                    continue
                except Exception:
                    pass
            if hasattr(t, "dict"):
                try:
                    result.append(t.dict())
                    continue
                except Exception:
                    pass
            result.append({"name": getattr(t, "name", str(t))})
        return result

    # ---- Health & runtime ----
    def check_services(self) -> Dict[str, Any]:
        return self._context._sync_helper.run_async(self._context._store.get_health_status(self._agent_id, agent_mode=True))

    def call_tool(self, tool_name: str, args: Dict[str, Any]) -> Dict[str, Any]:
        # Delegate to agent-view context and normalize
        agent_ctx = self._agent_ctx or self._context
        res = agent_ctx.call_tool(tool_name, args)
        try:
            if hasattr(res, 'content'):
                items = []
                for c in getattr(res, 'content', []) or []:
                    try:
                        if isinstance(c, dict):
                            items.append(c)
                        elif hasattr(c, 'type') and hasattr(c, 'text'):
                            items.append({"type": getattr(c, 'type', 'text'), "text": getattr(c, 'text', '')})
                        elif hasattr(c, 'type') and hasattr(c, 'uri'):
                            items.append({"type": getattr(c, 'type', 'uri'), "uri": getattr(c, 'uri', '')})
                        else:
                            items.append(str(c))
                    except Exception:
                        items.append(str(c))
                return {"content": items, "is_error": bool(getattr(res, 'is_error', False))}
            if isinstance(res, dict):
                return res
            if isinstance(res, list):
                return {"result": res}
            return {"result": str(res)}
        except Exception:
            return {"result": str(res)}

    # ---- Mutations ----
    def add_service(self, config: Dict[str, Any]) -> bool:
        ctx = self._agent_ctx or self._context
        return bool(ctx.add_service(config))

    def update_service(self, name: str, patch: Dict[str, Any]) -> bool:
        ctx = self._agent_ctx or self._context
        return bool(ctx.update_service(name, patch))

    def delete_service(self, name: str) -> bool:
        ctx = self._agent_ctx or self._context
        return bool(ctx.delete_service(name))

    # Async counterparts (explicit wrappers)
    async def add_service_async(self, *args, **kwargs):
        ctx = self._agent_ctx or self._context
        return await ctx.add_service_async(*args, **kwargs)

    async def call_tool_async(self, tool_name: str, args: Dict[str, Any]):
        ctx = self._agent_ctx or self._context
        return await ctx.call_tool_async(tool_name, args)

    async def show_config_async(self, scope: str = "all") -> Dict[str, Any]:
        ctx = self._agent_ctx or self._context
        return await ctx.show_config_async(scope)

    async def delete_config_async(self, client_id_or_service_name: str) -> Dict[str, Any]:
        ctx = self._agent_ctx or self._context
        return await ctx.delete_config_async(client_id_or_service_name)

    async def update_config_async(self, client_id_or_service_name: str, new_config: Dict[str, Any]) -> Dict[str, Any]:
        ctx = self._agent_ctx or self._context
        return await ctx.update_config_async(client_id_or_service_name, new_config)

    async def reset_config_async(self, scope: str = "all") -> bool:
        ctx = self._agent_ctx or self._context
        return await ctx.reset_config_async(scope)

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


    def patch_service(self, name: str, updates: Dict[str, Any]) -> bool:
        ctx = self._agent_ctx or self._context
        return bool(ctx.patch_service(name, updates))

    async def patch_service_async(self, name: str, updates: Dict[str, Any]) -> bool:
        ctx = self._agent_ctx or self._context
        return await ctx.patch_service_async(name, updates)

    def restart_service(self, name: str) -> bool:
        ctx = self._agent_ctx or self._context
        return bool(ctx.restart_service(name))

    async def restart_service_async(self, name: str) -> bool:
        ctx = self._agent_ctx or self._context
        return await ctx.restart_service_async(name)

    def use_tool(self, tool_name: str, args: Any = None, **kwargs) -> Any:
        ctx = self._agent_ctx or self._context
        return ctx.use_tool(tool_name, args, **kwargs)

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

    def hub_services(self):
        ctx = self._agent_ctx or self._context
        return ctx.hub_services()

    def hub_tools(self):
        ctx = self._agent_ctx or self._context
        return ctx.hub_tools()

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
    def reset_config(self, scope: str = "all") -> bool:
        ctx = self._agent_ctx or self._context
        return bool(ctx.reset_config(scope))

    def show_config(self, scope: str = "all") -> Dict[str, Any]:
        ctx = self._agent_ctx or self._context
        return ctx.show_config(scope)

    # ---- Escape hatch ----
    def get_context(self):
        return self._agent_ctx or self._context

    # ---- Compatibility: delegate unknown attrs to agent-scoped context ----
    def __getattr__(self, name: str):
        target = self._agent_ctx or self._context
        return getattr(target, name)


