import importlib
import json
import subprocess
from typing import Any, Dict, List, Optional
from urllib.parse import quote


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
        "called_at": None,
        "client_id": None,
        "command": None,
        "config": None,
        "content": [],
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
        if name == "text_output":
            return _extract_text_result(self)
        key = self._ALIASES.get(name, name)
        if key in self:
            return self[key]
        if name in self._OPTIONAL_DEFAULTS:
            return self._default_value(name)
        raise AttributeError(name)

    def __getitem__(self, key: Any) -> Any:
        if isinstance(key, str):
            if key == "text_output":
                return _extract_text_result(self)
            aliased = self._ALIASES.get(key)
            if aliased in self:
                return super().__getitem__(aliased)
            if super().__contains__(key):
                return super().__getitem__(key)
            if key in self._OPTIONAL_DEFAULTS:
                return self._default_value(key)
        return super().__getitem__(key)

    def get(self, key: Any, default: Any = None) -> Any:
        if isinstance(key, str):
            if key == "text_output":
                return _extract_text_result(self) or default
            aliased = self._ALIASES.get(key)
            if aliased in self:
                return super().get(aliased, default)
            if super().__contains__(key):
                return super().get(key, default)
            if key in self._OPTIONAL_DEFAULTS:
                return self._default_value(key)
        return super().get(key, default)

    def __contains__(self, key: object) -> bool:
        if isinstance(key, str):
            if key == "text_output":
                return True
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


def _extract_text_result(result: Any) -> str:
    content = result.get("content", []) if isinstance(result, dict) else []
    text_blocks = [
        item.get("text", "")
        for item in content
        if isinstance(item, dict) and item.get("type") == "text"
    ]
    return "\n".join(text for text in text_blocks if text)


def _normalize_status_targets(status: Optional[Any]) -> Optional[set[str]]:
    if status is None:
        return None
    values = status if isinstance(status, (list, tuple, set)) else [status]
    targets: set[str] = set()
    for value in values:
        key = str(value).lower()
        targets.add(key)
        if key == "healthy":
            targets.add("ready")
        elif key == "ready":
            targets.add("healthy")
        elif key == "warning":
            targets.add("degraded")
        elif key == "ok":
            targets.update({"healthy", "ready"})
    return targets


def _status_value(status: Any) -> str:
    if isinstance(status, dict):
        value = status.get("health_status") or status.get("status")
        if isinstance(value, dict):
            value = value.get("value") or value.get("status") or value.get("name")
        return str(value or "").lower()
    value = getattr(status, "health_status", None) or getattr(status, "status", None)
    return str(value or "").lower()


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
        self._sessions: Dict[tuple[str, str], "RustSession"] = {}
        self._active_sessions: Dict[str, "RustSession"] = {}
        self._auto_sessions: Dict[str, "RustSession"] = {}
        self._tool_overrides: Dict[str, Dict[str, Any]] = {}
        self._tool_visibility: Dict[str, Dict[str, Optional[set[str]]]] = {}
        self.registry = RustRegistryFacade(self)
        self._config_path: Optional[str] = None
        self._cache_config: Any = None
        self._only_db: bool = False

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
        store._config_path = config_path
        store._cache_config = cache_config
        store._only_db = only_db
        store.load_from_config()
        return store

    @staticmethod
    def _redis_url(cache_config) -> Optional[str]:
        redis_url = getattr(cache_config, "url", None)
        if redis_url:
            return redis_url
        host = getattr(cache_config, "host", None)
        if host:
            port = getattr(cache_config, "port", None) or 6379
            db = getattr(cache_config, "db", None) or 0
            password = getattr(cache_config, "password", None)
            auth = f":{quote(str(password), safe='')}@" if password else ""
            return f"redis://{auth}{host}:{port}/{db}"

        client = getattr(cache_config, "client", None)
        if client is None:
            return None

        pool = getattr(client, "connection_pool", None)
        kwargs = getattr(pool, "connection_kwargs", None)
        if not isinstance(kwargs, dict):
            raise ValueError(
                "Rust core 无法直接复用 Python Redis client；请使用带 connection_pool.connection_kwargs 的 client，"
                "或改用 RedisConfig(url=...)。"
            )

        host = kwargs.get("host")
        if not host:
            raise ValueError("Redis client connection_kwargs 缺少 host，Rust core 无法推导 Redis URL")
        port = getattr(cache_config, "port", None) or 6379
        port = kwargs.get("port", port)
        db = kwargs.get("db", 0)
        username = kwargs.get("username")
        password = kwargs.get("password")
        if username and password:
            auth = f"{quote(str(username), safe='')}:{quote(str(password), safe='')}@"
        elif password:
            auth = f":{quote(str(password), safe='')}@"
        else:
            auth = ""
        connection_class = kwargs.get("connection_class")
        class_name = getattr(connection_class, "__name__", "")
        scheme = "rediss" if kwargs.get("ssl") or "SSL" in class_name else "redis"
        return f"{scheme}://{auth}{host}:{port}/{db}"

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

    @staticmethod
    def _is_wide_service_map(config: Dict[str, Any]) -> bool:
        service_fields = {
            "args",
            "command",
            "description",
            "env",
            "headers",
            "transport",
            "type",
            "url",
            "workingDir",
            "working_dir",
        }
        return (
            "name" not in config
            and "mcpServers" not in config
            and not any(key in config for key in service_fields)
            and bool(config)
            and all(isinstance(value, dict) for value in config.values())
        )

    @classmethod
    def _normalize_service_config(
        cls,
        config: Any = None,
        *,
        json_file: Optional[str] = None,
        headers: Optional[Dict[str, Any]] = None,
    ) -> List[Dict[str, Any]]:
        if json_file is not None:
            with open(json_file, "r", encoding="utf-8") as file:
                config = json.load(file)
        elif isinstance(config, str):
            config = json.loads(config)

        if config is None:
            raise ValueError("服务配置缺少 config 或 json_file")

        configs = config if isinstance(config, list) else [config]
        normalized: List[Dict[str, Any]] = []
        for item in configs:
            item = cls._validate_dict(item)
            if headers:
                item = dict(item)
                if "mcpServers" in item and isinstance(item["mcpServers"], dict):
                    servers = {}
                    for name, server_config in item["mcpServers"].items():
                        server = cls._validate_dict(server_config)
                        merged = dict(server)
                        merged["headers"] = {**dict(server.get("headers") or {}), **headers}
                        servers[name] = merged
                    item["mcpServers"] = servers
                elif cls._is_wide_service_map(item):
                    item = {
                        name: {
                            **dict(server_config),
                            "headers": {
                                **dict(server_config.get("headers") or {}),
                                **headers,
                            },
                        }
                        for name, server_config in item.items()
                    }
                else:
                    item["headers"] = {**dict(item.get("headers") or {}), **headers}
            normalized.append(item)
        return normalized

    def for_store(self):
        return RustStoreContext(self)

    def for_agent(self, agent_id: str):
        return RustStoreContext(self, agent_id=agent_id)

    def namespace(self) -> str:
        return self._inner.namespace()

    def current_backend(self) -> str:
        return self._inner.current_backend()

    def add_service(
        self,
        config: Any = None,
        *,
        json_file: Optional[str] = None,
        headers: Optional[Dict[str, Any]] = None,
    ) -> bool:
        configs = self._normalize_service_config(config, json_file=json_file, headers=headers)
        for config in configs:
            self._add_service_one(config)
        return True

    def _add_service_one(self, config: Dict[str, Any]) -> None:
        mcp_servers = config.get("mcpServers", {})
        if mcp_servers:
            for name, server_config in mcp_servers.items():
                self._inner.add_service(name, self._validate_dict(server_config))
            return
        if self._is_wide_service_map(config):
            for name, server_config in config.items():
                self._inner.add_service(name, self._validate_dict(server_config))
            return

        name = config.get("name")
        if not name:
            raise ValueError("服务配置缺少 name，或缺少 mcpServers 批量配置")
        server_config = {k: v for k, v in config.items() if k != "name"}
        self._inner.add_service(name, self._validate_dict(server_config))

    def add_service_for_agent(
        self,
        agent_id: str,
        config: Any = None,
        *,
        json_file: Optional[str] = None,
        headers: Optional[Dict[str, Any]] = None,
    ) -> List[str]:
        added: List[str] = []
        configs = self._normalize_service_config(config, json_file=json_file, headers=headers)
        for item in configs:
            added.extend(self._add_service_for_agent_one(agent_id, item))
        return added

    def _add_service_for_agent_one(self, agent_id: str, config: Dict[str, Any]) -> List[str]:
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
        if self._is_wide_service_map(config):
            for local_name, server_config in config.items():
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

    def connect_service(self, name: str) -> bool:
        self._inner.connect_service(name)
        return True

    def disconnect_service(self, name: str) -> bool:
        self._inner.disconnect_service(name)
        return True

    def list_services(self) -> List[Dict[str, Any]]:
        return _record_value(self._inner.list_services())

    def list_agents(self) -> List[Dict[str, Any]]:
        return _record_value(self._inner.list_agents())

    def event_history(self, count: int = 100) -> List[Dict[str, Any]]:
        return _record_value(self._inner.event_history(int(count)))

    def event_capability_report(self) -> Dict[str, Any]:
        return _record_value(self._inner.event_capability_report())

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

    def show_config(self, scope: str = "all") -> Dict[str, Any]:
        config = _record_value(self._inner.show_config())
        if scope in (None, "all"):
            return config
        if scope in ("mcp", "mcpServers"):
            return _record_value({"mcpServers": config.get("mcpServers", {})})
        if scope in ("agents", "agent"):
            return _record_value({"agents": config.get("agents", {})})
        raise ValueError(f"Rust core 当前不支持 show_config scope={scope!r}")

    def show_mcpjson(self) -> Dict[str, Any]:
        return self.show_config("mcp")

    def get_json_config(self) -> Dict[str, Any]:
        return self.show_config()

    def get_data_space_info(self) -> Dict[str, Any]:
        return _record_value(
            {
                "namespace": self.namespace(),
                "backend": self.current_backend(),
                "config_path": self._config_path,
                "source_mode": "db" if self._only_db else "local",
                "service_count": len(self.list_services()),
                "agent_count": len(self.list_agents()),
            }
        )

    async def exportjson(self, path: Optional[str] = None) -> Dict[str, Any]:
        config = self.get_json_config()
        if path:
            with open(path, "w", encoding="utf-8") as file:
                json.dump(config, file, ensure_ascii=False, indent=2)
        return config

    async def export_to_json(self, path: str) -> Dict[str, Any]:
        return await self.exportjson(path)

    async def import_from_json(self, path: str) -> bool:
        with open(path, "r", encoding="utf-8") as file:
            config = json.load(file)
        self.add_service(config)
        return True

    async def cleanup(self) -> bool:
        return self.reset_config()

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

    def switch_cache(self, cache_config: Any) -> bool:
        rust_mod = importlib.import_module("mcpstore._rust")
        backend, redis_url, namespace = self._cache_options(cache_config)
        self._inner = rust_mod.MCPStore.setup_with_options(
            self._config_path,
            "db" if self._only_db else "local",
            backend,
            redis_url,
            namespace,
        )
        self._cache_config = cache_config
        self.load_from_config()
        return True

    def wait_service_ready(self, name: str, timeout: float = 10.0) -> Dict[str, Any]:
        return _record_value(self._inner.wait_service_ready(name, int(timeout)))

    def start_api_server(
        self,
        host: str = "127.0.0.1",
        port: int = 18200,
        url_prefix: str = "",
        show_startup_info: bool = True,
        log_level: Optional[str] = None,
        **_kwargs,
    ) -> int:
        from mcpstore._rust_cli import resolve_rust_cli_binary, resolve_runtime_cwd

        cmd = [
            resolve_rust_cli_binary(),
            "api",
            "--host",
            str(host),
            "--port",
            str(port),
        ]
        if url_prefix:
            cmd.extend(["--url-prefix", url_prefix])
        if self._config_path:
            cmd.extend(["--config-path", self._config_path])
        if self._only_db:
            cmd.extend(["--source", "db"])

        backend, redis_url, namespace = self._cache_options(self._cache_config)
        if backend:
            cmd.extend(["--backend", backend])
        if redis_url:
            cmd.extend(["--redis-url", redis_url])
        if namespace:
            cmd.extend(["--namespace", namespace])

        completed = subprocess.run(cmd, cwd=resolve_runtime_cwd(), check=False)
        return completed.returncode

    def start_mcp_server(
        self,
        *,
        agent_id: Optional[str] = None,
        host: str = "127.0.0.1",
        port: int = 18300,
        path: str = "/mcp",
        block: bool = True,
        **_kwargs,
    ) -> Any:
        from mcpstore._rust_cli import resolve_rust_cli_binary, resolve_runtime_cwd

        cmd = [
            resolve_rust_cli_binary(),
            "mcp-server",
            "--transport",
            "streamable-http",
            "--host",
            str(host),
            "--port",
            str(port),
            "--path",
            str(path),
            "--scope",
            "agent" if agent_id else "store",
        ]
        if agent_id:
            cmd.extend(["--agent", agent_id])
        if self._config_path:
            cmd.extend(["--config-path", self._config_path])
        if self._only_db:
            cmd.extend(["--source", "db"])

        backend, redis_url, namespace = self._cache_options(self._cache_config)
        if backend:
            cmd.extend(["--backend", backend])
        if redis_url:
            cmd.extend(["--redis-url", redis_url])
        if namespace:
            cmd.extend(["--namespace", namespace])

        if block:
            completed = subprocess.run(cmd, cwd=resolve_runtime_cwd(), check=False)
            return completed.returncode
        return subprocess.Popen(cmd, cwd=resolve_runtime_cwd())

    def session_key(self, agent_id: Optional[str]) -> str:
        return agent_id or "global_agent_store"

    def get_session(
        self,
        context: "RustStoreContext",
        session_id: str,
    ) -> "RustSession":
        key = (context.get_id(), session_id)
        session = self._sessions.get(key)
        if session is None or not session.is_active:
            session = RustSession(context, session_id)
            self._sessions[key] = session
        return session

    def find_session(
        self,
        context: "RustStoreContext",
        session_id: Optional[str] = None,
    ) -> Optional["RustSession"]:
        context_id = context.get_id()
        if session_id is None:
            return self._auto_sessions.get(context_id) or self._active_sessions.get(context_id)
        session = self._sessions.get((context_id, session_id))
        if session is not None and session.is_active:
            return session
        return None

    def set_active_session(
        self,
        context: "RustStoreContext",
        session: Optional["RustSession"],
    ) -> None:
        context_id = context.get_id()
        if session is None:
            self._active_sessions.pop(context_id, None)
        else:
            self._active_sessions[context_id] = session

    def active_session(self, context: "RustStoreContext") -> Optional["RustSession"]:
        context_id = context.get_id()
        return self._active_sessions.get(context_id) or self._auto_sessions.get(context_id)

    def enable_auto_session(self, context: "RustStoreContext", session: "RustSession") -> None:
        self._auto_sessions[context.get_id()] = session

    def disable_auto_session(self, context: "RustStoreContext") -> None:
        self._auto_sessions.pop(context.get_id(), None)


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

    @property
    def is_agent_scoped(self) -> bool:
        return self.agent_id is not None

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

    def tools_stats(self) -> Dict[str, Any]:
        tools = self.list_tools()
        return _record_value(
            {
                "service_name": self._service_name,
                "tool_count": len(tools),
                "tools": [
                    {
                        "name": tool.get("name"),
                        "description": tool.get("description"),
                        "tags": tool.get("tags", []) or [],
                    }
                    for tool in tools
                ],
                "source": "rust_metadata",
                "history_available": False,
            }
        )

    def find_tool(self, tool_name: str, service_name: Optional[str] = None) -> "RustToolProxy":
        return RustToolProxy(self._context, tool_name, service_name=service_name or self._service_name)

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

    def connect_service(self) -> bool:
        return self._context.connect_service(self._service_name)

    def disconnect_service(self) -> bool:
        return self._context.disconnect_service(self._service_name)

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

    def usage_stats(self) -> Dict[str, Any]:
        info = self.tool_info()
        return _record_value(
            {
                "tool_name": info.get("name") or self._tool_name,
                "service_name": (
                    self._service_name
                    or info.get("service_name")
                    or info.get("global_service_name")
                ),
                "call_count": None,
                "error_count": None,
                "last_called_at": None,
                "history_available": False,
                "source": "rust_metadata",
            }
        )

    def call_history(self, limit: int = 50) -> List[Dict[str, Any]]:
        return []

    def find_cache(self) -> "RustCacheProxy":
        return RustCacheProxy(self._context, scope="tool", scope_value=self._tool_name)

    def set_redirect(self, enabled: bool = True) -> "RustToolProxy":
        info = self.tool_info()
        service_name = (
            self._service_name
            or info.get("service_name")
            or info.get("global_service_name")
            or ""
        )
        resolved_tool_name = info.get("name") or self._tool_name
        self._context._set_tool_override(
            service_name,
            resolved_tool_name,
            "return_direct",
            bool(enabled),
        )
        return self

    def call_tool(
        self,
        args: Optional[Dict[str, Any]] = None,
        *,
        return_extracted: bool = False,
        **_kwargs,
    ) -> Any:
        if self._service_name:
            info = self.tool_info()
            service_name = self._context._resolve_service_name(self._service_name)
            tool_name = (
                info.get("original_name")
                or info.get("tool_original_name")
                or info.get("name")
                or self._tool_name
            )
            result = self._context._backend.call_tool(service_name, tool_name, args or {})
        else:
            result = self._context.call_tool(self._tool_name, args or {})
        if not return_extracted:
            return result
        return _extract_text_result(result)

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

    def get_scope(self) -> str:
        return self._scope

    def get_backend_type(self) -> Optional[str]:
        inspect = self.inspect()
        return inspect.get("backend")

    def dump_all(self) -> Dict[str, Any]:
        return self.inspect()

    def _read_collection(
        self,
        collection_name: str,
        type_name: Optional[str] = None,
        key: Optional[str] = None,
    ) -> List[Dict[str, Any]]:
        inspect = self.inspect()
        items = inspect.get(collection_name, []) or []
        if type_name is not None:
            items = [item for item in items if item.get("_type") == type_name]
        if key is not None:
            items = [item for item in items if item.get("_key") == key or item.get("key") == key]
        return _record_value(items)

    def read_entity(
        self,
        type_name: Optional[str] = None,
        key: Optional[str] = None,
    ) -> List[Dict[str, Any]]:
        return self._read_collection("entities", type_name, key)

    def read_relation(
        self,
        type_name: Optional[str] = None,
        key: Optional[str] = None,
    ) -> List[Dict[str, Any]]:
        return self._read_collection("relations", type_name, key)

    def read_state(
        self,
        type_name: Optional[str] = None,
        key: Optional[str] = None,
    ) -> List[Dict[str, Any]]:
        return self._read_collection("states", type_name, key)


class RustRegistryFacade:
    def __init__(self, backend: RustStoreBackend):
        self._backend = backend

    async def ping(self) -> bool:
        health = self._backend.cache_health_check()
        return bool(health.get("healthy", True))

    async def clear_all(self) -> bool:
        return self._backend.reset_config()

    async def get_cache_statistics(self) -> Dict[str, Any]:
        inspect = self._backend.cache_inspect()
        entity_count = len(inspect.get("entities", []) or [])
        relation_count = len(inspect.get("relations", []) or [])
        state_count = len(inspect.get("states", []) or [])
        event_count = len(inspect.get("events", []) or [])
        return _record_value(
            {
                "backend": inspect.get("backend"),
                "namespace": inspect.get("namespace"),
                "total_requests": 0,
                "hits": 0,
                "misses": 0,
                "hit_rate": 0.0,
                "avg_latency_ms": 0.0,
                "p50_latency_ms": 0.0,
                "p95_latency_ms": 0.0,
                "p99_latency_ms": 0.0,
                "total_size_bytes": 0,
                "entity_count": entity_count,
                "relation_count": relation_count,
                "state_count": state_count,
                "event_count": event_count,
            }
        )

    async def get_statistics(self) -> Dict[str, Any]:
        return await self.get_cache_statistics()

    async def reset_cache_statistics(self) -> bool:
        return True

    async def switch_backend(self, backend: Any) -> bool:
        self._backend.switch_cache(self._cache_config_from_backend(backend))
        return True

    def _cache_config_from_backend(self, backend: Any) -> Any:
        if hasattr(backend, "cache_type"):
            return backend

        class_name = backend.__class__.__name__.lower()
        if "redis" in class_name or hasattr(backend, "url"):
            from mcpstore.config import RedisConfig

            url = getattr(backend, "url", None) or getattr(backend, "_url", None)
            if url is None:
                host = getattr(backend, "host", None) or "localhost"
                port = getattr(backend, "port", None) or 6379
                db = getattr(backend, "db", None) or 0
                url = f"redis://{host}:{port}/{db}"
            return RedisConfig(
                url=url,
                password=getattr(backend, "password", None),
                namespace=getattr(backend, "namespace", None),
            )

        if "memory" in class_name:
            from mcpstore.config import MemoryConfig

            return MemoryConfig()

        raise ValueError(f"Rust registry facade 无法识别 cache backend: {backend!r}")


class RustSession:
    def __init__(self, context: "RustStoreContext", session_id: str):
        self._context = context
        self._session_id = session_id
        self._is_active = True
        self._bound_services: List[str] = []

    @property
    def session_id(self) -> str:
        return self._session_id

    @property
    def is_active(self) -> bool:
        return self._is_active

    @property
    def service_count(self) -> int:
        return len(self._bound_services) if self._bound_services else len(self.list_services())

    @property
    def tool_count(self) -> int:
        return len(self.list_tools())

    def __enter__(self) -> "RustSession":
        self._is_active = True
        self._context._backend.set_active_session(self._context, self)
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        self._context._backend.set_active_session(self._context, None)

    async def __aenter__(self) -> "RustSession":
        return self.__enter__()

    async def __aexit__(self, exc_type, exc, tb) -> None:
        self.__exit__(exc_type, exc, tb)

    def bind_service(self, service_name: str) -> "RustSession":
        resolved = self._context._resolve_service_name(service_name)
        if resolved not in self._bound_services:
            self._bound_services.append(resolved)
        return self

    async def bind_service_async(self, service_name: str) -> "RustSession":
        return self.bind_service(service_name)

    def list_services(self) -> List[Dict[str, Any]]:
        if not self._bound_services:
            return self._context.list_services()
        services = []
        for name in self._bound_services:
            info = self._context.get_service_info(name)
            if info:
                services.append(info)
        return _record_value(services)

    def list_tools(self, service_name: Optional[str] = None) -> List[Dict[str, Any]]:
        if service_name is not None:
            return self._context._list_tools_direct(service_name=service_name)
        if not self._bound_services:
            return self._context._list_tools_direct()
        tools: List[Dict[str, Any]] = []
        for name in self._bound_services:
            tools.extend(self._context._list_tools_direct(service_name=name))
        return _record_value(tools)

    def use_tool(
        self,
        tool_name: str,
        arguments: Optional[Dict[str, Any]] = None,
        *,
        return_extracted: bool = False,
        **_kwargs,
    ) -> Any:
        result = self._context.call_tool(tool_name, arguments or {})
        if not return_extracted:
            return result
        content = result.get("content", []) if isinstance(result, dict) else []
        text_blocks = [
            item.get("text", "")
            for item in content
            if isinstance(item, dict) and item.get("type") == "text"
        ]
        return "\n".join(text for text in text_blocks if text)

    async def use_tool_async(
        self,
        tool_name: str,
        arguments: Optional[Dict[str, Any]] = None,
        *,
        return_extracted: bool = False,
        **kwargs,
    ) -> Any:
        return self.use_tool(
            tool_name,
            arguments,
            return_extracted=return_extracted,
            **kwargs,
        )

    def session_info(self) -> Dict[str, Any]:
        return _record_value(
            {
                "session_id": self._session_id,
                "is_active": self._is_active,
                "agent_id": self._context.agent_id,
                "services": list(self._bound_services),
                "service_count": self.service_count,
                "tool_count": self.tool_count,
            }
        )

    def connection_status(self) -> Dict[str, Any]:
        return _record_value(
            {
                "session_id": self._session_id,
                "is_active": self._is_active,
                "services": {
                    service: "bound"
                    for service in self._bound_services
                },
            }
        )

    def restart_session(self) -> "RustSession":
        for service_name in list(self._bound_services):
            self._context.restart_service(service_name)
        return self

    def extend_session(self, seconds: int = 3600) -> "RustSession":
        return self

    def clear_cache(self) -> bool:
        return True

    def close_session(self) -> bool:
        self._is_active = False
        self._context._backend.set_active_session(self._context, None)
        return True


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

    async def bridge_execute(self, value: Any) -> Any:
        if hasattr(value, "__await__"):
            return await value
        return value

    def add_service(
        self,
        config: Any = None,
        *,
        json_file: Optional[str] = None,
        headers: Optional[Dict[str, Any]] = None,
    ) -> "RustStoreContext":
        if self._agent_id:
            self._backend.add_service_for_agent(
                self._agent_id,
                config,
                json_file=json_file,
                headers=headers,
            )
        else:
            self._backend.add_service(config, json_file=json_file, headers=headers)
        return self

    @property
    def agent_id(self) -> Optional[str]:
        return self._agent_id

    @property
    def context_type(self) -> str:
        return "agent" if self._agent_id else "store"

    @property
    def active_session(self) -> Optional[RustSession]:
        return self._backend.active_session(self)

    def current_session(self) -> Optional[RustSession]:
        return self.active_session

    def list_services(self) -> List[Dict[str, Any]]:
        return _record_value(self._backend.list_services_scoped(self._agent_id))

    def list_agents(self) -> List[Dict[str, Any]]:
        return _record_value(self._backend.list_agents())

    def _list_tools_direct(
        self,
        service_name: Optional[str] = None,
        *,
        filter: str = "available",
    ) -> List[Dict[str, Any]]:
        tools = _record_value(
            self._backend.list_tools_scoped(self._agent_id, service_name)
        )
        return self._apply_tool_visibility(tools, service_name=service_name, filter=filter)

    def list_tools(self, service_name: Optional[str] = None, *, filter: str = "available") -> List[Dict[str, Any]]:
        active = self.active_session
        if service_name is None and active is not None and active._bound_services:
            return active.list_tools()
        return self._list_tools_direct(service_name, filter=filter)

    def _tool_visibility_for(self, service_name: str) -> Optional[set[str]]:
        return self._backend._tool_visibility.get(self.get_id(), {}).get(service_name)

    def _tool_service_name(self, tool: Dict[str, Any], fallback: Optional[str] = None) -> str:
        value = (
            tool.get("service_name")
            or tool.get("serviceName")
            or tool.get("global_service_name")
            or fallback
            or ""
        )
        return str(value)

    def _tool_candidate_names(self, tool: Dict[str, Any]) -> set[str]:
        return {
            str(name)
            for name in (
                tool.get("name"),
                tool.get("original_name"),
                tool.get("tool_original_name"),
            )
            if name
        }

    def _apply_tool_visibility(
        self,
        tools: List[Dict[str, Any]],
        *,
        service_name: Optional[str] = None,
        filter: str = "available",
    ) -> List[Dict[str, Any]]:
        mode = filter or "available"
        if mode not in {"all", "available", "removed"}:
            raise ValueError(f"Rust facade 当前不支持工具过滤器: {filter}")
        if mode == "all":
            return _record_value(tools)

        selected: List[Dict[str, Any]] = []
        for tool in tools:
            resolved_service = self._tool_service_name(tool, service_name)
            visible = self._tool_visibility_for(resolved_service)
            is_visible = True
            if visible is not None:
                is_visible = bool(self._tool_candidate_names(tool) & visible)
            if (mode == "available" and is_visible) or (mode == "removed" and not is_visible):
                selected.append(tool)
        return _record_value(selected)

    def find_service(self, service_name: str) -> RustServiceProxy:
        return RustServiceProxy(self, service_name)

    def find_tool(self, tool_name: str, service_name: Optional[str] = None) -> RustToolProxy:
        return RustToolProxy(self, tool_name, service_name=service_name)

    def find_agent(self, agent_id: str) -> "RustStoreContext":
        return RustStoreContext(self._backend, agent_id=agent_id)

    def find_cache(self) -> RustCacheProxy:
        scope = "agent" if self._agent_id else "global"
        return RustCacheProxy(self, scope=scope, scope_value=self._agent_id)

    def _service_names_for_tool_scope(self, service: Any) -> List[str]:
        if service == "_all_services":
            return [item.name for item in self.list_services()]
        if isinstance(service, RustServiceProxy):
            if service.agent_id != self._agent_id:
                raise ValueError("不能使用其他 Agent 作用域的 ServiceProxy 修改当前 Agent 工具集")
            return [service.service_name]
        return [str(service)]

    def _tool_names_for_service(self, service_name: str) -> set[str]:
        names: set[str] = set()
        for tool in self._list_tools_direct(service_name=service_name, filter="all"):
            names.update(self._tool_candidate_names(tool))
        return names

    def _set_visible_tools(self, service_name: str, visible: Optional[set[str]]) -> None:
        context_visibility = self._backend._tool_visibility.setdefault(self.get_id(), {})
        if visible is None:
            context_visibility.pop(service_name, None)
        else:
            context_visibility[service_name] = visible

    def remove_tools(self, service: Any, tools: Any) -> "RustStoreContext":
        for service_name in self._service_names_for_tool_scope(service):
            if tools == "_all_tools":
                self._set_visible_tools(service_name, set())
                continue
            current = self._tool_visibility_for(service_name)
            visible = set(current) if current is not None else self._tool_names_for_service(service_name)
            remove = {str(tool) for tool in (tools if isinstance(tools, (list, tuple, set)) else [tools])}
            self._set_visible_tools(service_name, visible - remove)
        return self

    def add_tools(self, service: Any, tools: Any) -> "RustStoreContext":
        for service_name in self._service_names_for_tool_scope(service):
            current = self._tool_visibility_for(service_name)
            if current is None:
                visible = self._tool_names_for_service(service_name)
            else:
                visible = set(current)
            visible.update(str(tool) for tool in (tools if isinstance(tools, (list, tuple, set)) else [tools]))
            self._set_visible_tools(service_name, visible)
        return self

    def reset_tools(self, service: Any) -> "RustStoreContext":
        for service_name in self._service_names_for_tool_scope(service):
            self._set_visible_tools(service_name, None)
        return self

    def get_tool_set_info(self, service: Any) -> Dict[str, Any]:
        service_name = self._service_names_for_tool_scope(service)[0]
        total = len(self._list_tools_direct(service_name=service_name, filter="all"))
        available = len(self._list_tools_direct(service_name=service_name, filter="available"))
        removed = max(total - available, 0)
        return _record_value(
            {
                "agent_id": self._agent_id,
                "service_name": service_name,
                "total_tools": total,
                "available_tools": available,
                "removed_tools": removed,
                "utilization": (available / total) if total else 0.0,
            }
        )

    def get_tool_set_summary(self) -> Dict[str, Any]:
        services = {}
        total_tools = 0
        total_available = 0
        for service in self.list_services():
            info = self.get_tool_set_info(service.name)
            services[service.name] = info
            total_tools += info.total_tools
            total_available += info.available_tools
        return _record_value(
            {
                "agent_id": self._agent_id,
                "total_services": len(services),
                "total_available_tools": total_available,
                "total_original_tools": total_tools,
                "overall_utilization": (total_available / total_tools) if total_tools else 0.0,
                "services": services,
            }
        )

    def create_session(
        self,
        session_id: str,
        user_session_id: Optional[str] = None,
    ) -> RustSession:
        return self._backend.get_session(self, user_session_id or session_id)

    def find_session(
        self,
        session_id: Optional[str] = None,
        is_user_session_id: bool = False,
    ) -> Optional[RustSession]:
        return self._backend.find_session(self, session_id)

    def get_session(self, session_id: str) -> RustSession:
        return self.create_session(session_id)

    def list_sessions(self) -> List[RustSession]:
        context_id = self.get_id()
        return [
            session
            for (cached_context_id, _), session in self._backend._sessions.items()
            if cached_context_id == context_id and session.is_active
        ]

    def with_session(self, session_id: str) -> RustSession:
        return self.create_session(session_id)

    async def with_session_async(self, session_id: str) -> RustSession:
        return self.with_session(session_id)

    def session_auto(
        self,
        session_id: str = "auto_session_default",
        default_timeout: int = 720000,
        auto_cleanup: bool = False,
        session_prefix: str = "auto_",
    ) -> "RustStoreContext":
        session = self.create_session(session_id)
        session._is_active = True
        self._backend.enable_auto_session(self, session)
        return self

    def session_manual(self) -> "RustStoreContext":
        self._backend.disable_auto_session(self)
        return self

    def get_service_info(self, name: str) -> Dict[str, Any]:
        service_name = self._resolve_service_name(name)
        service = self._backend.find_service(service_name)
        return _record_value(service or {})

    def service_info(self, name: str) -> Dict[str, Any]:
        return self.get_service_info(name)

    def get_service_status(self, name: str) -> Dict[str, Any]:
        return self._backend.service_status_scoped(self._agent_id, name)

    def service_status(self, name: str) -> Dict[str, Any]:
        return self.get_service_status(name)

    def check_services(self) -> Dict[str, Any]:
        return self._backend.check_services_scoped(self._agent_id)

    def get_info(self) -> Dict[str, Any]:
        return _record_value(
            {
                "context_type": self.context_type,
                "agent_id": self._agent_id,
                "namespace": self._backend.namespace(),
                "backend": self._backend.current_backend(),
                "service_count": len(self.list_services()),
                "tool_count": len(self.list_tools()),
            }
        )

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

    def disconnect_service(self, name: str) -> bool:
        service_name = self._resolve_service_name(name)
        return self._backend.disconnect_service(service_name)

    def connect_service(self, name: str) -> bool:
        service_name = self._resolve_service_name(name)
        return self._backend.connect_service(service_name)

    def event_history(self, count: int = 100) -> List[Dict[str, Any]]:
        return self._backend.event_history(count)

    def event_capability_report(self) -> Dict[str, Any]:
        return self._backend.event_capability_report()

    def for_langchain(self, response_format: str = "text"):
        from mcpstore.adapters.langchain_adapter import LangChainAdapter

        return LangChainAdapter(self, response_format=response_format)

    def for_langgraph(self, response_format: str = "text"):
        return self.for_langchain(response_format=response_format)

    def for_openai(self):
        from mcpstore.adapters.openai_adapter import OpenAIAdapter

        return OpenAIAdapter(self)

    def for_autogen(self):
        from mcpstore.adapters.autogen_adapter import AutoGenAdapter

        return AutoGenAdapter(self)

    def for_llamaindex(self):
        from mcpstore.adapters.llamaindex_adapter import LlamaIndexAdapter

        return LlamaIndexAdapter(self)

    def for_crewai(self):
        from mcpstore.adapters.crewai_adapter import CrewAIAdapter

        return CrewAIAdapter(self)

    def for_semantic_kernel(self):
        from mcpstore.adapters.semantic_kernel_adapter import SemanticKernelAdapter

        return SemanticKernelAdapter(self)

    def _call_tool_direct(self, tool_name: str, args: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        service_name, original_tool = self._resolve_tool(tool_name)
        return self._backend.call_tool(service_name, original_tool, args or {})

    def call_tool(
        self,
        tool_name: str,
        args: Optional[Dict[str, Any]] = None,
        *,
        return_extracted: bool = False,
        **_kwargs,
    ) -> Any:
        result = self._call_tool_direct(tool_name, args)
        if return_extracted:
            return _extract_text_result(result)
        return result

    def use_tool(
        self,
        tool_name: str,
        args: Optional[Dict[str, Any]] = None,
        *,
        return_extracted: bool = False,
        **kwargs,
    ) -> Any:
        return self.call_tool(
            tool_name,
            args,
            return_extracted=return_extracted,
            **kwargs,
        )

    async def call_tool_async(
        self,
        tool_name: str,
        args: Optional[Dict[str, Any]] = None,
        *,
        return_extracted: bool = False,
        **kwargs,
    ) -> Any:
        return self.call_tool(
            tool_name,
            args,
            return_extracted=return_extracted,
            **kwargs,
        )

    async def use_tool_async(
        self,
        tool_name: str,
        args: Optional[Dict[str, Any]] = None,
        *,
        return_extracted: bool = False,
        **kwargs,
    ) -> Any:
        return await self.call_tool_async(
            tool_name,
            args,
            return_extracted=return_extracted,
            **kwargs,
        )

    def _tool_override_key(self, service_name: str, tool_name: str) -> str:
        return f"{service_name or ''}:{tool_name}"

    def _set_tool_override(
        self,
        service_name: str,
        tool_name: str,
        flag: str,
        value: Any,
    ) -> None:
        key = self._tool_override_key(service_name, tool_name)
        self._backend._tool_overrides.setdefault(key, {})[flag] = value

    def get_tool_override(
        self,
        service_name: str,
        tool_name: str,
        flag: str,
        default: Any = None,
    ) -> Any:
        keys = [
            self._tool_override_key(service_name, tool_name),
            self._tool_override_key("", tool_name),
        ]
        for key in keys:
            overrides = self._backend._tool_overrides.get(key)
            if overrides and flag in overrides:
                return overrides[flag]
        return default

    def show_config(self, scope: str = "all") -> Dict[str, Any]:
        return self._backend.show_config(scope)

    def show_mcpjson(self) -> Dict[str, Any]:
        return self._backend.show_mcpjson()

    def get_json_config(self) -> Dict[str, Any]:
        return self._backend.get_json_config()

    def get_data_space_info(self) -> Dict[str, Any]:
        return self._backend.get_data_space_info()

    def switch_cache(self, cache_config: Any) -> "RustStoreContext":
        self._backend.switch_cache(cache_config)
        return self

    def hub_http(
        self,
        *,
        port: int = 18300,
        host: str = "127.0.0.1",
        path: str = "/mcp",
        block: bool = True,
        **kwargs,
    ) -> Any:
        return self._backend.start_mcp_server(
            agent_id=self._agent_id,
            host=host,
            port=port,
            path=path,
            block=block,
            **kwargs,
        )

    def hub_sse(self, *args, **kwargs) -> Any:
        raise NotImplementedError(
            "Rust core 当前未暴露 SSE Hub；请使用 hub_http(...), 该接口委托 Rust streamable-http MCP server。"
        )

    def hub_stdio(self, *args, **kwargs) -> Any:
        raise NotImplementedError(
            "Rust core 当前未暴露可嵌入的 stdio Hub；请使用 Rust CLI `mcpstore mcp-server --transport stdio`。"
        )

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
        result = self._backend.wait_service_ready(service_name, timeout)
        targets = _normalize_status_targets(status)
        if targets is None:
            return result
        actual = _status_value(result)
        if actual in targets:
            return result
        raise TimeoutError(
            f"Wait for service status timed out: {name} "
            f"(expected={sorted(targets)}, actual={actual or 'unknown'})"
        )

    def wait_services(
        self,
        names: List[str],
        status: Optional[Any] = None,
        timeout: float = 10.0,
    ) -> Dict[str, Any]:
        results = {}
        for name in names:
            results[name] = self.wait_service(name, status=status, timeout=timeout)
        return _record_value(results)

    def _resolve_tool(self, tool_name: str) -> tuple[str, str]:
        active = self.active_session
        if active is not None and active._bound_services:
            for service_name in active._bound_services:
                for tool in self._list_tools_direct(service_name=service_name):
                    names = {
                        tool.get("name"),
                        tool.get("original_name"),
                        tool.get("tool_original_name"),
                    }
                    if tool_name in names:
                        return service_name, tool.get("original_name") or tool.get("name")
        resolution = self._backend.resolve_tool_for_agent(self.get_id(), tool_name)
        return resolution["global_service_name"], resolution["canonical_tool_name"]
