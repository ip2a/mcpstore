import importlib
from typing import Any, Dict, List, Optional


class RustRecordView(dict):
    """dict payload with the historical Python SDK attribute API."""

    _ALIASES = {
        "serviceName": "service_name",
        "service_global_name": "global_service_name",
        "structuredContent": "structured_content",
        "inputSchema": "input_schema",
        "outputSchema": "output_schema",
        "transport_type": "transport",
    }
    _OPTIONAL_DEFAULTS = {
        "args": [],
        "client_id": None,
        "command": None,
        "config": None,
        "description": None,
        "env": {},
        "headers": {},
        "meta": {},
        "tags": [],
        "state_metadata": None,
        "data": None,
        "structured_content": None,
        "url": None,
        "working_dir": None,
        "workingDir": None,
    }

    def __getattr__(self, name: str) -> Any:
        key = self._ALIASES.get(name, name)
        if key in self:
            return self[key]
        if name in self._OPTIONAL_DEFAULTS:
            return self._default_value(name)
        raise AttributeError(name)

    def __getitem__(self, key: Any) -> Any:
        if isinstance(key, str):
            aliased = self._ALIASES.get(key)
            if aliased in self:
                return super().__getitem__(aliased)
            if key in self._OPTIONAL_DEFAULTS:
                return self._default_value(key)
        return super().__getitem__(key)

    def get(self, key: Any, default: Any = None) -> Any:
        if isinstance(key, str):
            aliased = self._ALIASES.get(key)
            if aliased in self:
                return super().get(aliased, default)
            if key in self._OPTIONAL_DEFAULTS:
                return self._default_value(key)
        return super().get(key, default)

    def __contains__(self, key: object) -> bool:
        if isinstance(key, str):
            aliased = self._ALIASES.get(key)
            if aliased and super().__contains__(aliased):
                return True
            if key in self._OPTIONAL_DEFAULTS:
                return True
        return super().__contains__(key)

    @classmethod
    def _default_value(cls, name: str) -> Any:
        default = cls._OPTIONAL_DEFAULTS[name]
        if isinstance(default, dict):
            return dict(default)
        if isinstance(default, list):
            return list(default)
        return default


def _record_value(value: Any) -> Any:
    if isinstance(value, dict):
        return RustRecordView({key: _record_value(item) for key, item in value.items()})
    if isinstance(value, list):
        return [_record_value(item) for item in value]
    return value


def _tool_error_result(
    service_name: str,
    tool_name: str,
    args: Optional[Dict[str, Any]],
    error: Exception,
) -> Dict[str, Any]:
    message = f"MCP 工具调用失败: {error}"
    payload = {
        "ok": False,
        "is_error": True,
        "error_type": "mcp_tool_call_failed",
        "service_name": service_name,
        "tool_name": tool_name,
        "arguments": dict(args or {}),
        "message": message,
    }
    return _record_value(
        {
            "content": [{"type": "text", "text": message}],
            "structured_content": payload,
            "data": payload,
            "error": message,
            "is_error": True,
        }
    )


class RustStoreBackend:
    """Python facade for mcpstore._rust.MCPStore."""

    def __init__(self, rust_store):
        self._inner = rust_store

    @classmethod
    def setup(
        cls,
        config_path: Optional[str] = None,
        cache_config=None,
        only_db: bool = False,
    ) -> "RustStoreBackend":
        rust_mod = importlib.import_module("mcpstore._rust")
        backend, redis_url, namespace = cls._cache_options(cache_config)
        rust_store = rust_mod.MCPStore.setup_with_options(
            config_path,
            "db" if only_db else "local",
            backend,
            redis_url,
            namespace,
        )
        store = cls(rust_store)
        store.load_from_config()
        return store

    @staticmethod
    def _redis_url(cache_config) -> Optional[str]:
        if getattr(cache_config, "client", None) is not None:
            raise ValueError("Rust core 不支持直接传入 Redis client 对象，请改用 url 或 host/port/db 配置")
        redis_url = getattr(cache_config, "url", None)
        if redis_url:
            return redis_url
        host = getattr(cache_config, "host", None)
        if not host:
            return None
        port = getattr(cache_config, "port", None) or 6379
        db = getattr(cache_config, "db", None) or 0
        password = getattr(cache_config, "password", None)
        auth = f":{password}@" if password else ""
        return f"redis://{auth}{host}:{port}/{db}"

    @classmethod
    def _cache_options(cls, cache_config):
        if cache_config is None:
            return None, None, None
        cache_type = getattr(cache_config, "cache_type", None)
        cache_value = getattr(cache_type, "value", cache_type)
        namespace = getattr(cache_config, "namespace", None)

        if cache_value == "memory":
            return "memory", None, namespace
        if cache_value == "redis":
            redis_url = cls._redis_url(cache_config)
            if not redis_url:
                raise ValueError("Redis 缓存配置缺少 url 或 host，Rust core 不会使用隐式默认 Redis 地址")
            return "redis", redis_url, namespace
        if cache_value == "openkeyv_memory":
            return "openkeyv_memory", None, namespace
        if cache_value == "openkeyv_redis":
            redis_url = cls._redis_url(cache_config)
            if not redis_url:
                raise ValueError("OpenKeyvRedisConfig 缺少 url 或 host，Rust core 不会使用隐式默认 Redis 地址")
            return "openkeyv_redis", redis_url, namespace
        raise ValueError(f"不支持的 Rust 缓存配置: {cache_config!r}")

    @staticmethod
    def _validate_dict(value: Dict[str, Any]) -> Dict[str, Any]:
        if not isinstance(value, dict):
            raise ValueError(f"Rust core 只接受 dict 对象，实际类型: {type(value).__name__}")
        return value

    def for_store(self):
        return RustStoreContext(self)

    def for_agent(self, agent_id: str):
        return RustStoreContext(self, agent_id=agent_id)

    def namespace(self) -> str:
        return self._inner.namespace()

    def current_backend(self) -> str:
        return self._inner.current_backend()

    def add_service(self, config: Dict[str, Any]) -> bool:
        mcp_servers = config.get("mcpServers", {})
        if mcp_servers:
            for name, server_config in mcp_servers.items():
                self._inner.add_service(name, self._validate_dict(server_config))
            return True

        name = config.get("name")
        if not name:
            raise ValueError("服务配置缺少 name，或缺少 mcpServers 批量配置")
        server_config = {k: v for k, v in config.items() if k != "name"}
        self._inner.add_service(name, self._validate_dict(server_config))
        return True

    def add_service_for_agent(self, agent_id: str, config: Dict[str, Any]) -> List[str]:
        added: List[str] = []
        mcp_servers = config.get("mcpServers", {})
        if mcp_servers:
            for local_name, server_config in mcp_servers.items():
                added.append(
                    self._inner.add_service_for_agent(
                        agent_id,
                        local_name,
                        self._validate_dict(server_config),
                    )
                )
            return added

        local_name = config.get("name")
        if not local_name:
            raise ValueError("Agent 服务配置缺少 name，或缺少 mcpServers 批量配置")
        server_config = {k: v for k, v in config.items() if k != "name"}
        added.append(
            self._inner.add_service_for_agent(
                agent_id,
                local_name,
                self._validate_dict(server_config),
            )
        )
        return added

    def patch_service(self, name: str, updates: Dict[str, Any]) -> bool:
        self._inner.patch_service(name, self._validate_dict(updates))
        return True

    def update_service(self, name: str, config: Dict[str, Any]) -> bool:
        self._inner.update_service(name, self._validate_dict(config))
        return True

    def remove_service(self, name: str) -> bool:
        self._inner.remove_service(name)
        return True

    def restart_service(self, name: str) -> bool:
        self._inner.restart_service(name)
        return True

    def list_services(self) -> List[Dict[str, Any]]:
        return _record_value(self._inner.list_services())

    def list_services_scoped(self, agent_id: Optional[str] = None) -> List[Dict[str, Any]]:
        return _record_value(self._inner.list_services_scoped(agent_id))

    def find_service(self, name: str) -> Optional[Dict[str, Any]]:
        return _record_value(self._inner.find_service(name))

    def get_service_config(self, name: str) -> Optional[Dict[str, Any]]:
        return _record_value(self._inner.get_service_config(name))

    def check_services_scoped(self, agent_id: Optional[str] = None) -> Dict[str, Any]:
        return _record_value(self._inner.check_services_scoped(agent_id))

    def service_status_scoped(
        self,
        agent_id: Optional[str],
        service_name: str,
    ) -> Dict[str, Any]:
        return _record_value(self._inner.service_status_scoped(agent_id, service_name))

    def list_resources_scoped(
        self,
        agent_id: Optional[str] = None,
        service_name: Optional[str] = None,
    ) -> List[Dict[str, Any]]:
        return _record_value(self._inner.list_resources_scoped(agent_id, service_name))

    def list_resource_templates_scoped(
        self,
        agent_id: Optional[str] = None,
        service_name: Optional[str] = None,
    ) -> List[Dict[str, Any]]:
        return _record_value(self._inner.list_resource_templates_scoped(agent_id, service_name))

    def read_resource_scoped(
        self,
        agent_id: Optional[str],
        uri: str,
        service_name: Optional[str] = None,
    ) -> Dict[str, Any]:
        return _record_value(self._inner.read_resource_scoped(agent_id, uri, service_name))

    def list_prompts_scoped(
        self,
        agent_id: Optional[str] = None,
        service_name: Optional[str] = None,
    ) -> List[Dict[str, Any]]:
        return _record_value(self._inner.list_prompts_scoped(agent_id, service_name))

    def get_prompt_scoped(
        self,
        agent_id: Optional[str],
        prompt_name: str,
        arguments: Dict[str, Any],
        service_name: Optional[str] = None,
    ) -> Dict[str, Any]:
        return _record_value(
            self._inner.get_prompt_scoped(
                agent_id,
                prompt_name,
                self._validate_dict(arguments or {}),
                service_name,
            )
        )

    def list_tools_scoped(
        self,
        agent_id: Optional[str] = None,
        service_name: Optional[str] = None,
        *,
        filter: str = "available",
    ) -> List[Dict[str, Any]]:
        if filter != "available":
            raise ValueError(f"Rust core 当前不支持工具过滤器: {filter}")
        return _record_value(self._inner.list_tools_scoped(agent_id, service_name))

    def call_tool(
        self,
        service_name: str,
        tool_name: str,
        args: Dict[str, Any],
    ) -> Dict[str, Any]:
        try:
            return _record_value(
                self._inner.call_tool(
                    service_name,
                    tool_name,
                    self._validate_dict(args or {}),
                )
            )
        except Exception as error:
            return _tool_error_result(service_name, tool_name, args, error)

    def resolve_tool_for_agent(self, agent_id: str, user_input: str) -> Dict[str, Any]:
        return _record_value(self._inner.resolve_tool_for_agent(agent_id, user_input))

    def resolve_service_name_for_agent(self, agent_id: str, service_name: str) -> str:
        return self._inner.resolve_service_name_for_agent(agent_id, service_name)

    def show_config(self) -> Dict[str, Any]:
        return _record_value(self._inner.show_config())

    def cache_health_check(self) -> Dict[str, Any]:
        return _record_value(self._inner.cache_health_check())

    def cache_inspect(self) -> Dict[str, Any]:
        return _record_value(self._inner.cache_inspect())

    def reset_config(self) -> bool:
        self._inner.reset_config()
        return True

    def load_from_config(self) -> bool:
        self._inner.load_from_config()
        return True

    def wait_service_ready(self, name: str, timeout: float = 10.0) -> Dict[str, Any]:
        return _record_value(self._inner.wait_service_ready(name, int(timeout)))


class RustServiceProxy:
    def __init__(self, context: "RustStoreContext", service_name: str):
        self._context = context
        self._service_name = service_name

    @property
    def name(self) -> str:
        return self._service_name

    @property
    def service_name(self) -> str:
        return self._service_name

    @property
    def agent_id(self) -> Optional[str]:
        return self._context.agent_id

    def service_info(self) -> Dict[str, Any]:
        return self._context.get_service_info(self._service_name)

    def get_service_info(self) -> Dict[str, Any]:
        return self.service_info()

    def service_status(self) -> Dict[str, Any]:
        return self._context.get_service_status(self._service_name)

    def health_details(self) -> Dict[str, Any]:
        return self.service_status()

    def check_health(self) -> Dict[str, Any]:
        status = self.service_status()
        health_status = status.get("health_status", status.get("status", "unknown"))
        return _record_value(
            {
                "service_name": self._service_name,
                "status": health_status,
                "healthy": health_status in ("healthy", "connected", "ok"),
                "error_message": status.get("error_message") or status.get("error"),
            }
        )

    def is_healthy(self) -> bool:
        return bool(self.check_health().get("healthy"))

    def list_tools(self) -> List[Dict[str, Any]]:
        return self._context.list_tools(service_name=self._service_name)

    def find_tool(self, tool_name: str) -> "RustToolProxy":
        return RustToolProxy(self._context, tool_name, service_name=self._service_name)

    def find_cache(self) -> "RustCacheProxy":
        return RustCacheProxy(self._context, scope="service", scope_value=self._service_name)

    def list_resources(self) -> List[Dict[str, Any]]:
        return self._context.list_resources(service_name=self._service_name)

    def list_resource_templates(self) -> List[Dict[str, Any]]:
        return self._context.list_resource_templates(service_name=self._service_name)

    def read_resource(self, uri: str) -> Dict[str, Any]:
        return self._context.read_resource(uri, service_name=self._service_name)

    def list_prompts(self) -> List[Dict[str, Any]]:
        return self._context.list_prompts(service_name=self._service_name)

    def get_prompt(
        self,
        prompt_name: str,
        arguments: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        return self._context.get_prompt(
            prompt_name,
            arguments or {},
            service_name=self._service_name,
        )

    def update_config(self, config: Dict[str, Any]) -> bool:
        return self._context.update_service(self._service_name, config)

    def patch_config(self, updates: Dict[str, Any]) -> bool:
        return self._context.patch_service(self._service_name, updates)

    def restart_service(self) -> bool:
        return self._context.restart_service(self._service_name)

    def delete_service(self) -> bool:
        return self._context.delete_service(self._service_name)


class RustToolProxy:
    def __init__(
        self,
        context: "RustStoreContext",
        tool_name: str,
        service_name: Optional[str] = None,
    ):
        self._context = context
        self._tool_name = tool_name
        self._service_name = service_name
        self._cached_info: Optional[Dict[str, Any]] = None

    @property
    def name(self) -> str:
        return self._tool_name

    @property
    def tool_name(self) -> str:
        return self._tool_name

    @property
    def service_name(self) -> Optional[str]:
        return self._service_name

    def tool_info(self) -> Dict[str, Any]:
        if self._cached_info is None:
            self._cached_info = self._load_tool_info()
        return self._cached_info

    def tool_schema(self) -> Dict[str, Any]:
        return self.tool_info().get("inputSchema", {}) or {}

    def tool_tags(self) -> List[str]:
        return self.tool_info().get("tags", []) or []

    def tool_meta(self) -> Dict[str, Any]:
        return self.tool_info().get("meta", {}) or {}

    def find_cache(self) -> "RustCacheProxy":
        return RustCacheProxy(self._context, scope="tool", scope_value=self._tool_name)

    def call_tool(
        self,
        args: Optional[Dict[str, Any]] = None,
        *,
        return_extracted: bool = False,
    ) -> Any:
        result = self._context.call_tool(self._tool_name, args or {})
        if not return_extracted:
            return result
        content = result.get("content", []) if isinstance(result, dict) else []
        text_blocks = [
            item.get("text", "")
            for item in content
            if isinstance(item, dict) and item.get("type") == "text"
        ]
        return "\n".join(text for text in text_blocks if text)

    async def call_tool_async(
        self,
        args: Optional[Dict[str, Any]] = None,
        *,
        return_extracted: bool = False,
    ) -> Any:
        return self.call_tool(args, return_extracted=return_extracted)

    def _load_tool_info(self) -> Dict[str, Any]:
        tools = self._context.list_tools(service_name=self._service_name)
        for tool in tools:
            names = {
                tool.get("name"),
                tool.get("original_name"),
                tool.get("tool_original_name"),
            }
            if self._tool_name in names:
                return tool

        if self._service_name is None:
            resolution = self._context._backend.resolve_tool_for_agent(
                self._context.get_id(),
                self._tool_name,
            )
            service_name = resolution.get("service_name") or resolution.get("global_service_name")
            canonical = resolution.get("canonical_tool_name") or self._tool_name
            scoped_tools = self._context.list_tools(service_name=service_name)
            for tool in scoped_tools:
                names = {
                    tool.get("name"),
                    tool.get("original_name"),
                    tool.get("tool_original_name"),
                    canonical,
                }
                if self._tool_name in names or canonical in names:
                    return tool

        return _record_value({"name": self._tool_name, "service_name": self._service_name})


class RustCacheProxy:
    def __init__(
        self,
        context: "RustStoreContext",
        scope: str = "global",
        scope_value: Optional[str] = None,
    ):
        self._context = context
        self._scope = scope
        self._scope_value = scope_value

    @property
    def scope(self) -> str:
        return self._scope

    @property
    def scope_value(self) -> Optional[str]:
        return self._scope_value

    def inspect(self) -> Dict[str, Any]:
        return _record_value(self._context._backend.cache_inspect())

    def health_check(self) -> Dict[str, Any]:
        return _record_value(self._context._backend.cache_health_check())

    def stats(self) -> Dict[str, Any]:
        return self.inspect()


class RustStoreContext:
    def __init__(self, backend: RustStoreBackend, agent_id: Optional[str] = None):
        self._backend = backend
        self._agent_id = agent_id

    def __getattr__(self, name: str) -> Any:
        if name.endswith("_async"):
            sync_name = name[:-6]
            if hasattr(self, sync_name):
                sync_method = getattr(self, sync_name)
                async def _async_wrapper(*args, **kwargs):
                    return sync_method(*args, **kwargs)
                return _async_wrapper
        raise AttributeError(name)

    def add_service(self, config: Dict[str, Any]) -> bool:
        if self._agent_id:
            self._backend.add_service_for_agent(self._agent_id, config)
            return True
        return self._backend.add_service(config)

    @property
    def agent_id(self) -> Optional[str]:
        return self._agent_id

    @property
    def context_type(self) -> str:
        return "agent" if self._agent_id else "store"

    def list_services(self) -> List[Dict[str, Any]]:
        return _record_value(self._backend.list_services_scoped(self._agent_id))

    def list_tools(self, service_name: Optional[str] = None, *, filter: str = "available") -> List[Dict[str, Any]]:
        return _record_value(
            self._backend.list_tools_scoped(
                self._agent_id,
                service_name,
                filter=filter,
            )
        )

    def find_service(self, service_name: str) -> RustServiceProxy:
        return RustServiceProxy(self, service_name)

    def find_tool(self, tool_name: str) -> RustToolProxy:
        return RustToolProxy(self, tool_name)

    def find_agent(self, agent_id: str) -> "RustStoreContext":
        return RustStoreContext(self._backend, agent_id=agent_id)

    def find_cache(self) -> RustCacheProxy:
        scope = "agent" if self._agent_id else "global"
        return RustCacheProxy(self, scope=scope, scope_value=self._agent_id)

    def get_service_info(self, name: str) -> Dict[str, Any]:
        service_name = self._resolve_service_name(name)
        service = self._backend.find_service(service_name)
        return _record_value(service or {})

    def service_info(self, name: str) -> Dict[str, Any]:
        return self.get_service_info(name)

    def get_service_status(self, name: str) -> Dict[str, Any]:
        return self._backend.service_status_scoped(self._agent_id, name)

    def check_services(self) -> Dict[str, Any]:
        return self._backend.check_services_scoped(self._agent_id)

    def list_resources(self, service_name: Optional[str] = None) -> List[Dict[str, Any]]:
        return _record_value(
            self._backend.list_resources_scoped(self._agent_id, service_name)
        )

    def list_resource_templates(
        self,
        service_name: Optional[str] = None,
    ) -> List[Dict[str, Any]]:
        return _record_value(
            self._backend.list_resource_templates_scoped(self._agent_id, service_name)
        )

    def read_resource(
        self,
        uri: str,
        service_name: Optional[str] = None,
    ) -> Dict[str, Any]:
        return _record_value(
            self._backend.read_resource_scoped(self._agent_id, uri, service_name)
        )

    def list_prompts(self, service_name: Optional[str] = None) -> List[Dict[str, Any]]:
        return _record_value(
            self._backend.list_prompts_scoped(self._agent_id, service_name)
        )

    def get_prompt(
        self,
        prompt_name: str,
        arguments: Optional[Dict[str, Any]] = None,
        service_name: Optional[str] = None,
    ) -> Dict[str, Any]:
        return _record_value(
            self._backend.get_prompt_scoped(
                self._agent_id,
                prompt_name,
                arguments or {},
                service_name,
            )
        )

    def patch_service(self, name: str, updates: Dict[str, Any]) -> bool:
        service_name = self._resolve_service_name(name)
        return self._backend.patch_service(service_name, updates)

    def update_service(self, name: str, config: Dict[str, Any]) -> bool:
        return self.patch_service(name, config)

    def replace_service_config(self, name: str, config: Dict[str, Any]) -> bool:
        service_name = self._resolve_service_name(name)
        return self._backend.update_service(service_name, config)

    def delete_service(self, name: str) -> bool:
        service_name = self._resolve_service_name(name)
        return self._backend.remove_service(service_name)

    def remove_service(self, name: str) -> bool:
        return self.delete_service(name)

    def restart_service(self, name: str) -> bool:
        service_name = self._resolve_service_name(name)
        return self._backend.restart_service(service_name)

    def for_langchain(self):
        from mcpstore.adapters.langchain_adapter import LangChainAdapter

        return LangChainAdapter(self)

    def for_langgraph(self):
        return self.for_langchain()

    def for_openai(self):
        from mcpstore.adapters.openai_adapter import OpenAIAdapter

        return OpenAIAdapter(self)

    def for_autogen(self):
        from mcpstore.adapters.autogen_adapter import AutoGenAdapter

        return AutoGenAdapter(self)

    def _call_tool_direct(self, tool_name: str, args: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        service_name, original_tool = self._resolve_tool(tool_name)
        return self._backend.call_tool(service_name, original_tool, args or {})

    def call_tool(self, tool_name: str, args: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        return self._call_tool_direct(tool_name, args)

    async def call_tool_async(
        self,
        tool_name: str,
        args: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        return self._call_tool_direct(tool_name, args)

    def show_config(self) -> Dict[str, Any]:
        return self._backend.show_config()

    def reset_config(self) -> bool:
        return self._backend.reset_config()

    def get_id(self) -> str:
        return self._agent_id or "global_agent_store"

    def _resolve_service_name(self, name: str) -> str:
        if not self._agent_id:
            return name
        return self._backend.resolve_service_name_for_agent(self._agent_id, name)

    def wait_service(
        self,
        name: str,
        status: Optional[Any] = None,
        timeout: float = 10.0,
    ) -> Dict[str, Any]:
        service_name = self._resolve_service_name(name)
        return self._backend.wait_service_ready(service_name, timeout)

    def _resolve_tool(self, tool_name: str) -> tuple[str, str]:
        resolution = self._backend.resolve_tool_for_agent(self.get_id(), tool_name)
        return resolution["global_service_name"], resolution["canonical_tool_name"]
