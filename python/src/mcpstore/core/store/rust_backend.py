import importlib
import json
import os
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
    _CONFIG_FIELD_ALIASES = {
        "args": "args",
        "description": "description",
        "env": "env",
        "headers": "headers",
        "working_dir": "workingDir",
        "workingDir": "workingDir",
    }

    def __getattr__(self, name: str) -> Any:
        if name == "text_output":
            return _extract_text_result(self)
        key = self._ALIASES.get(name, name)
        if key in self:
            return self[key]
        value = self._config_field_value(name)
        if value is not _MISSING:
            return value
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
            value = self._config_field_value(key)
            if value is not _MISSING:
                return value
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
            value = self._config_field_value(key)
            if value is not _MISSING:
                return value
        return super().get(key, default)

    def __contains__(self, key: object) -> bool:
        if isinstance(key, str):
            if key == "text_output":
                return True
            aliased = self._ALIASES.get(key)
            if aliased and super().__contains__(aliased):
                return True
            if self._config_field_value(key) is not _MISSING:
                return True
        return super().__contains__(key)

    def __hash__(self) -> int:
        return hash(_freeze_record_value(dict(self)))

    def to_dict(self) -> Dict[str, Any]:
        return {key: _plain_record_value(value) for key, value in self.items()}

    def _config_field_value(self, name: str) -> Any:
        config_key = self._CONFIG_FIELD_ALIASES.get(name)
        if not config_key or not super().__contains__("config"):
            return _MISSING
        config = super().__getitem__("config")
        if isinstance(config, dict) and config_key in config:
            return config[config_key]
        return _MISSING


_MISSING = object()
_GLOBAL_AGENT_STORE = "global_agent_store"
_AGENT_SEPARATOR = "_byagent_"


def _record_value(value: Any) -> Any:
    if isinstance(value, dict):
        return RustRecordView({key: _record_value(item) for key, item in value.items()})
    if isinstance(value, list):
        return [_record_value(item) for item in value]
    return value


def _freeze_record_value(value: Any) -> Any:
    if isinstance(value, dict):
        return tuple(sorted((str(key), _freeze_record_value(item)) for key, item in value.items()))
    if isinstance(value, list):
        return tuple(_freeze_record_value(item) for item in value)
    if isinstance(value, set):
        return tuple(sorted(_freeze_record_value(item) for item in value))
    return value


def _plain_record_value(value: Any) -> Any:
    if isinstance(value, dict):
        return {key: _plain_record_value(item) for key, item in value.items()}
    if isinstance(value, list):
        return [_plain_record_value(item) for item in value]
    return value


class AwaitableBool:
    def __init__(self, value: bool):
        self.value = bool(value)

    def __bool__(self) -> bool:
        return self.value

    def __eq__(self, other: Any) -> bool:
        return self.value == other

    def __repr__(self) -> str:
        return repr(self.value)

    def __await__(self):
        async def _value():
            return self.value

        return _value().__await__()


class AwaitableList(list):
    def __await__(self):
        async def _value():
            return list(self)

        return _value().__await__()


def _extract_text_result(result: Any) -> str:
    content = result.get("content", []) if isinstance(result, dict) else []
    text_blocks = [
        item.get("text", "")
        for item in content
        if isinstance(item, dict) and item.get("type") == "text"
    ]
    return "\n".join(text for text in text_blocks if text)


def _tool_result_value(result: Any) -> Any:
    value = _record_value(result)
    if isinstance(value, dict) and "data" not in value:
        value["data"] = None
    return value


def _event_payload(event: Any) -> Dict[str, Any]:
    payload = event.get("payload", {}) if isinstance(event, dict) else getattr(event, "payload", {})
    return payload if isinstance(payload, dict) else {}


def _event_type(event: Any) -> str:
    value = event.get("event_type") if isinstance(event, dict) else getattr(event, "event_type", None)
    return str(value or "")


def _event_timestamp(event: Any) -> Optional[int]:
    value = event.get("timestamp") if isinstance(event, dict) else getattr(event, "timestamp", None)
    return value if isinstance(value, int) else None


def _event_id(event: Any) -> Optional[str]:
    value = event.get("event_id") if isinstance(event, dict) else getattr(event, "event_id", None)
    return str(value) if value else None


def _event_matches_service(event: Any, service_name: str) -> bool:
    payload = _event_payload(event)
    candidates = {
        payload.get("service_name"),
        payload.get("global_service_name"),
        payload.get("serviceName"),
        payload.get("name"),
    }
    return service_name in {str(value) for value in candidates if value}


def _is_tool_call_event(event: Any) -> bool:
    return _event_type(event) in {"TOOL_CALL_COMPLETED", "TOOL_CALL_FAILED"}


def _event_matches_tool(event: Any, service_name: Optional[str], tool_names: set[str]) -> bool:
    if not _is_tool_call_event(event):
        return False
    payload = _event_payload(event)
    if service_name and not _event_matches_service(event, service_name):
        return False
    return str(payload.get("tool_name") or payload.get("toolName") or "") in tool_names


def _tool_call_history_record(event: Any) -> Dict[str, Any]:
    payload = _event_payload(event)
    return _record_value(
        {
            "event_id": _event_id(event),
            "event_type": _event_type(event),
            "timestamp": _event_timestamp(event),
            "service_name": payload.get("service_name") or payload.get("serviceName"),
            "tool_name": payload.get("tool_name") or payload.get("toolName"),
            "arguments": payload.get("arguments", {}),
            "latency_ms": payload.get("latency_ms"),
            "is_error": bool(payload.get("is_error") or _event_type(event) == "TOOL_CALL_FAILED"),
            "status": payload.get("status"),
            "error": payload.get("error"),
        }
    )


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
    if isinstance(status, str):
        return status.lower()
    value = getattr(status, "health_status", None) or getattr(status, "status", None)
    return str(value or "").lower()


def _reject_unsupported_kwargs(context: str, kwargs: Dict[str, Any]) -> None:
    if kwargs:
        unsupported = ", ".join(sorted(kwargs))
        raise ValueError(f"{context} 当前不支持参数: {unsupported}")


def _tool_error_result(
    service_name: str,
    tool_name: str,
    args: Optional[Dict[str, Any]],
    error: Exception,
) -> Dict[str, Any]:
    message = f"MCP 工具调用失败: {error}"
    arguments = dict(args) if isinstance(args, dict) else {}
    payload = {
        "ok": False,
        "is_error": True,
        "error_type": "mcp_tool_call_failed",
        "service_name": service_name,
        "tool_name": tool_name,
        "arguments": arguments,
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
    """Compatibility name for the Rust-backed Python store facade."""

    def __init__(self, rust_store):
        self._inner = rust_store
        self._sessions: Dict[tuple[str, str], "RustSession"] = {}
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
        config_path = os.fspath(config_path) if config_path is not None else None
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

        return None

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

    @classmethod
    def _normalize_config_dict(cls, value: Any, context: str) -> Dict[str, Any]:
        try:
            return cls._validate_dict(value)
        except ValueError as error:
            raise ValueError(f"{context} 必须是 dict；Python SDK 不再接受 JSON 字符串配置") from error

    @classmethod
    def _normalize_optional_dict(cls, value: Any, context: str) -> Dict[str, Any]:
        if value is None:
            return {}
        try:
            return cls._validate_dict(value)
        except ValueError as error:
            raise ValueError(f"{context} 必须是 dict；Python SDK 不再接受 JSON 字符串参数") from error

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

        if config is None:
            raise ValueError("服务配置缺少 config 或 json_file")

        configs = config if isinstance(config, list) else [config]
        normalized: List[Dict[str, Any]] = []
        for item in configs:
            try:
                item = cls._validate_dict(item)
            except ValueError as error:
                raise ValueError("服务配置必须是 dict/list；Python SDK 不再接受 JSON 字符串配置") from error
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
    ) -> AwaitableBool:
        configs = self._normalize_service_config(config, json_file=json_file, headers=headers)
        for config in configs:
            self._add_service_one(config)
        return AwaitableBool(True)

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

    def patch_service(self, name: str, updates: Any) -> bool:
        self._inner.patch_service(name, self._normalize_config_dict(updates, "服务补丁配置"))
        return True

    def update_service(self, name: str, config: Any) -> bool:
        self._inner.update_service(name, self._normalize_config_dict(config, "服务更新配置"))
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
        return AwaitableList(_record_value(self._inner.list_services()))

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
                self._normalize_optional_dict(arguments, "Prompt arguments"),
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
        return _record_value(
            self._inner.list_tools_scoped(agent_id, service_name, filter=filter)
        )

    def get_context_tool_visibility(
        self,
        agent_id: Optional[str],
        service_name: str,
    ) -> Optional[Dict[str, Any]]:
        return _record_value(
            self._inner.get_context_tool_visibility(agent_id, service_name)
        )

    def set_context_tool_visibility(
        self,
        agent_id: Optional[str],
        service_name: str,
        tool_names: List[str],
    ) -> Dict[str, Any]:
        return _record_value(
            self._inner.set_context_tool_visibility(agent_id, service_name, tool_names)
        )

    def clear_context_tool_visibility(
        self,
        agent_id: Optional[str],
        service_name: str,
    ) -> None:
        self._inner.clear_context_tool_visibility(agent_id, service_name)

    def get_tool_preference(
        self,
        agent_id: Optional[str],
        service_name: str,
        tool_name: str,
        key: str,
        default: Any = None,
    ) -> Any:
        return _record_value(
            self._inner.get_tool_preference(
                agent_id,
                service_name,
                tool_name,
                key,
                default,
            )
        )

    def get_tool_preferences(
        self,
        agent_id: Optional[str],
        service_name: str,
        tool_name: str,
    ) -> Optional[Dict[str, Any]]:
        return _record_value(
            self._inner.get_tool_preferences(agent_id, service_name, tool_name)
        )

    def set_tool_preference(
        self,
        agent_id: Optional[str],
        service_name: str,
        tool_name: str,
        key: str,
        value: Any,
    ) -> Dict[str, Any]:
        return _record_value(
            self._inner.set_tool_preference(
                agent_id,
                service_name,
                tool_name,
                key,
                value,
            )
        )

    def clear_tool_preference(
        self,
        agent_id: Optional[str],
        service_name: str,
        tool_name: str,
        key: str,
    ) -> Optional[Dict[str, Any]]:
        return _record_value(
            self._inner.clear_tool_preference(agent_id, service_name, tool_name, key)
        )

    def list_changed_tools_scoped(
        self,
        agent_id: Optional[str] = None,
        service_name: Optional[str] = None,
        *,
        force_refresh: bool = False,
    ) -> Dict[str, Any]:
        return _record_value(
            self._inner.list_changed_tools_scoped(
                agent_id,
                service_name,
                force_refresh=bool(force_refresh),
            )
        )

    def import_openapi_service(
        self,
        name: str,
        spec_url: str,
        *,
        headers: Optional[Dict[str, str]] = None,
        auth: Optional[Dict[str, Any]] = None,
        ref_cache: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        return _record_value(
            self._inner.import_openapi_service(
                name,
                spec_url,
                self._openapi_import_options(headers, auth, ref_cache),
            )
        )

    def import_openapi_service_from_spec(
        self,
        name: str,
        spec_url: str,
        spec: Any,
        *,
        headers: Optional[Dict[str, str]] = None,
        auth: Optional[Dict[str, Any]] = None,
        ref_cache: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        options = self._openapi_import_options(headers, auth, ref_cache)
        if isinstance(spec, str):
            return _record_value(
                self._inner.import_openapi_service_from_spec_text(
                    name,
                    spec_url,
                    spec,
                    options,
                )
            )
        return _record_value(
            self._inner.import_openapi_service_from_spec(
                name,
                spec_url,
                self._normalize_config_dict(spec, "OpenAPI spec"),
                options,
            )
        )

    def _openapi_import_options(
        self,
        headers: Optional[Dict[str, str]],
        auth: Optional[Dict[str, Any]],
        ref_cache: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        return {
            "headers": dict(headers or {}),
            "auth": dict(auth or {}),
            "ref_cache": dict(ref_cache or {}),
        }

    def _openapi_bundle_options(
        self,
        ref_cache: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        return {"ref_cache": dict(ref_cache or {})}

    def bundle_openapi_spec(
        self,
        spec_url: str,
        *,
        ref_cache: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        if ref_cache is None:
            return _record_value(self._inner.bundle_openapi_spec(spec_url))
        return _record_value(
            self._inner.bundle_openapi_spec(
                spec_url,
                self._openapi_bundle_options(ref_cache),
            )
        )

    def bundle_openapi_spec_from_spec(
        self,
        spec_url: str,
        spec: Any,
        *,
        ref_cache: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        options = self._openapi_bundle_options(ref_cache) if ref_cache is not None else None
        if isinstance(spec, str):
            if options is None:
                return _record_value(self._inner.bundle_openapi_spec_from_spec(spec_url, spec))
            return _record_value(self._inner.bundle_openapi_spec_from_spec(spec_url, spec, options))
        normalized = self._normalize_config_dict(spec, "OpenAPI spec")
        if options is None:
            return _record_value(self._inner.bundle_openapi_spec_from_spec(spec_url, normalized))
        return _record_value(
            self._inner.bundle_openapi_spec_from_spec(
                spec_url,
                normalized,
                options,
            )
        )

    def bundle_openapi_artifact(
        self,
        spec_url: str,
        *,
        ref_cache: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        if ref_cache is None:
            return _record_value(self._inner.bundle_openapi_artifact(spec_url))
        return _record_value(
            self._inner.bundle_openapi_artifact(
                spec_url,
                self._openapi_bundle_options(ref_cache),
            )
        )

    def bundle_openapi_artifact_from_spec(
        self,
        spec_url: str,
        spec: Any,
        *,
        ref_cache: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        options = self._openapi_bundle_options(ref_cache) if ref_cache is not None else None
        if isinstance(spec, str):
            if options is None:
                return _record_value(self._inner.bundle_openapi_artifact_from_spec(spec_url, spec))
            return _record_value(self._inner.bundle_openapi_artifact_from_spec(spec_url, spec, options))
        normalized = self._normalize_config_dict(spec, "OpenAPI spec")
        if options is None:
            return _record_value(self._inner.bundle_openapi_artifact_from_spec(spec_url, normalized))
        return _record_value(
            self._inner.bundle_openapi_artifact_from_spec(
                spec_url,
                normalized,
                options,
            )
        )

    def get_openapi_import(self, name: str) -> Optional[Dict[str, Any]]:
        return _record_value(self._inner.get_openapi_import(name))

    def list_openapi_imports(self) -> List[Dict[str, Any]]:
        return _record_value(self._inner.list_openapi_imports())

    def last_openapi_import(self) -> Optional[Dict[str, Any]]:
        return _record_value(self._inner.last_openapi_import())

    def call_tool(
        self,
        service_name: str,
        tool_name: str,
        args: Dict[str, Any],
    ) -> Dict[str, Any]:
        try:
            return _tool_result_value(
                self._inner.call_tool(
                    service_name,
                    tool_name,
                    self._normalize_optional_dict(args, "Tool arguments"),
                )
            )
        except Exception as error:
            return _tool_error_result(service_name, tool_name, args, error)

    def resolve_tool_for_agent(self, agent_id: str, user_input: str) -> Dict[str, Any]:
        return _record_value(self._inner.resolve_tool_for_agent(agent_id, user_input))

    def set_tool_transform(
        self,
        service_name: str,
        tool_name: str,
        transform: Dict[str, Any],
    ) -> Dict[str, Any]:
        return _record_value(
            self._inner.set_tool_transform(
                service_name,
                tool_name,
                self._normalize_config_dict(transform, "Tool transform"),
            )
        )

    def create_llm_friendly_tool_transform(
        self,
        service_name: str,
        tool_name: str,
        friendly_name: Optional[str] = None,
        description: Optional[str] = None,
        hide_technical_params: bool = True,
        add_safety_policy: bool = True,
    ) -> Dict[str, Any]:
        return _record_value(
            self._inner.create_llm_friendly_tool_transform(
                service_name,
                tool_name,
                friendly_name,
                description,
                hide_technical_params,
                add_safety_policy,
            )
        )

    def create_parameter_renamed_tool_transform(
        self,
        service_name: str,
        tool_name: str,
        parameter_mapping: Dict[str, str],
        new_tool_name: Optional[str] = None,
    ) -> Dict[str, Any]:
        return _record_value(
            self._inner.create_parameter_renamed_tool_transform(
                service_name,
                tool_name,
                self._normalize_config_dict(parameter_mapping, "Parameter mapping"),
                new_tool_name,
            )
        )

    def create_validated_tool_transform(
        self,
        service_name: str,
        tool_name: str,
        validation_rules: Dict[str, Any],
        new_tool_name: Optional[str] = None,
    ) -> Dict[str, Any]:
        return _record_value(
            self._inner.create_validated_tool_transform(
                service_name,
                tool_name,
                self._normalize_config_dict(validation_rules, "Validation rules"),
                new_tool_name,
            )
        )

    def get_tool_transform(self, service_name: str, tool_name: str) -> Optional[Dict[str, Any]]:
        return _record_value(self._inner.get_tool_transform(service_name, tool_name))

    def list_tool_transforms(self) -> List[Dict[str, Any]]:
        return _record_value(self._inner.list_tool_transforms())

    def delete_tool_transform(self, service_name: str, tool_name: str) -> bool:
        self._inner.delete_tool_transform(service_name, tool_name)
        return True

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
        if scope in ("clients", "client"):
            return _record_value({"clients": config.get("clients", {})})
        raise ValueError(f"Rust core 当前不支持 show_config scope={scope!r}")

    def show_config_scoped(
        self,
        agent_id: Optional[str] = None,
        scope: str = "all",
    ) -> Dict[str, Any]:
        if agent_id is None:
            return self.show_config(scope)
        config = _record_value(self._inner.show_config_scoped(agent_id))
        return self._filter_config_scope(config, scope)

    def _filter_config_scope(self, config: Dict[str, Any], scope: str = "all") -> Dict[str, Any]:
        if scope in (None, "all"):
            return config
        if scope in ("mcp", "mcpServers"):
            return _record_value({"mcpServers": config.get("mcpServers", {})})
        if scope in ("agents", "agent"):
            return _record_value({"agents": config.get("agents", {})})
        if scope in ("clients", "client"):
            return _record_value({"clients": config.get("clients", {})})
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

    async def exportjson(
        self,
        filepath: Optional[str] = None,
        *,
        include_sessions: bool = False,
    ) -> Dict[str, Any]:
        path = filepath
        path = os.fspath(path) if path is not None else None
        config = self.get_json_config()
        if include_sessions:
            config = dict(config)
            config["sessions"] = _record_value(self._inner.export_sessions_snapshot())
        if path:
            with open(path, "w", encoding="utf-8") as file:
                json.dump(config, file, ensure_ascii=False, indent=2)
        return config

    async def export_to_json(
        self,
        output_path: Optional[str] = None,
        *,
        include_sessions: bool = False,
    ) -> Dict[str, Any]:
        return await self.exportjson(
            filepath=output_path,
            include_sessions=include_sessions,
        )

    async def import_from_json(self, path: str) -> bool:
        with open(path, "r", encoding="utf-8") as file:
            config = json.load(file)
        self.add_service(config)
        sessions = config.get("sessions")
        if sessions is not None:
            self._inner.import_sessions_snapshot(sessions)
        return True

    async def cleanup(self) -> bool:
        return self.reset_config()

    def cache_health_check(self) -> Dict[str, Any]:
        return _record_value(self._inner.cache_health_check())

    def cache_inspect(self) -> Dict[str, Any]:
        return _record_value(self._inner.cache_inspect())

    def reset_cache_request_metrics(self) -> bool:
        self._inner.reset_cache_request_metrics()
        return True

    def reset_config(self) -> bool:
        self._inner.reset_config()
        return True

    def reset_agent_config(self, agent_id: str) -> bool:
        self._inner.reset_agent_config(agent_id)
        return True

    def reset_mcp_json_scope(self, scope: Optional[str] = None) -> bool:
        self._inner.reset_mcp_json_scope(scope)
        return True

    def load_from_config(self) -> bool:
        self._inner.load_from_config()
        return True

    def switch_cache(self, cache_config: Any) -> bool:
        backend, redis_url, namespace = self._cache_options(cache_config)
        if backend is None:
            backend = "memory"
        self._inner.switch_cache_storage(backend, redis_url, namespace)
        self._cache_config = cache_config
        self._sessions.clear()
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
        **kwargs,
    ) -> int:
        from mcpstore._rust_cli import resolve_rust_cli_binary, resolve_runtime_cwd

        if kwargs:
            unsupported = ", ".join(sorted(kwargs))
            raise ValueError(f"Rust API server 当前不支持参数: {unsupported}")
        if show_startup_info is not True:
            raise ValueError("Rust API server 当前不支持 show_startup_info=False")
        if log_level is not None:
            raise ValueError("Rust API server 当前不支持 log_level 参数")

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
        service_name: Optional[str] = None,
        transport: str = "streamable-http",
        host: str = "0.0.0.0",
        port: int = 8000,
        path: str = "/mcp",
        block: bool = False,
        **kwargs,
    ) -> Any:
        from mcpstore._rust_cli import resolve_rust_cli_binary, resolve_runtime_cwd

        if kwargs:
            unsupported = ", ".join(sorted(kwargs))
            raise ValueError(f"Rust MCP server 当前不支持参数: {unsupported}")

        cmd = [
            resolve_rust_cli_binary(),
            "mcp-server",
            "--transport",
            transport,
        ]
        if transport != "stdio":
            cmd.extend(
                [
                    "--host",
                    str(host),
                    "--port",
                    str(port),
                    "--path",
                    str(path),
                ]
            )
        cmd.extend(
            [
                "--scope",
                "agent" if agent_id else "store",
            ]
        )
        if agent_id:
            cmd.extend(["--agent", agent_id])
        if service_name:
            cmd.extend(["--service", service_name])
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

    @staticmethod
    def _session_scope(context: "RustStoreContext") -> str:
        return "agent" if context.agent_id else "store"

    @staticmethod
    def _session_agent_id(context: "RustStoreContext") -> Optional[str]:
        return context.agent_id

    def _wrap_session(
        self,
        context: "RustStoreContext",
        entity: Dict[str, Any],
    ) -> "RustSession":
        session_id = str(entity["session_id"])
        key = (context.get_id(), session_id)
        session = self._sessions.get(key)
        if session is None:
            session = RustSession(context, entity)
            self._sessions[key] = session
        else:
            session._refresh_entity(entity)
        return session

    def get_session(
        self,
        context: "RustStoreContext",
        session_id: str,
        *,
        lease_seconds: Optional[int] = None,
        metadata: Optional[Dict[str, Any]] = None,
    ) -> "RustSession":
        scope = self._session_scope(context)
        agent_id = self._session_agent_id(context)
        entity = self._inner.find_session(session_id, scope, agent_id)
        if entity is None:
            entity = self._inner.create_session(
                session_id,
                scope,
                agent_id,
                lease_seconds,
                metadata or {},
            )
        return self._wrap_session(context, _record_value(entity))

    def find_session(
        self,
        context: "RustStoreContext",
        session_id: Optional[str] = None,
    ) -> Optional["RustSession"]:
        if session_id is None:
            return self.active_session(context)
        entity = self._inner.find_session(
            session_id,
            self._session_scope(context),
            self._session_agent_id(context),
        )
        if entity is None:
            return None
        return self._wrap_session(context, _record_value(entity))

    def find_session_by_user_session_id(
        self,
        context: "RustStoreContext",
        user_session_id: str,
    ) -> Optional["RustSession"]:
        entity = self._inner.find_session_by_user_session_id(user_session_id)
        if entity is None:
            return None
        return self._wrap_session(context, _record_value(entity))

    def update_session_metadata(
        self,
        session: "RustSession",
        metadata: Dict[str, Any],
    ) -> Dict[str, Any]:
        entity = self._inner.update_session_metadata(
            session.session_key,
            self._normalize_config_dict(metadata, "Session metadata"),
        )
        session._refresh_entity(_record_value(entity))
        return _record_value(entity)

    def list_sessions(self, context: "RustStoreContext") -> List["RustSession"]:
        sessions = self._inner.list_sessions(
            self._session_scope(context),
            self._session_agent_id(context),
        )
        return [self._wrap_session(context, _record_value(entity)) for entity in sessions]

    def get_session_status(self, session: "RustSession") -> Dict[str, Any]:
        status = self._inner.get_session_status(session.session_key)
        return _record_value(status or {})

    def get_session_entity(self, session: "RustSession") -> Dict[str, Any]:
        entity = self._inner.get_session(session.session_key)
        if entity:
            session._refresh_entity(_record_value(entity))
        return _record_value(entity or {})

    def extend_session(self, session: "RustSession", seconds: int) -> Dict[str, Any]:
        entity = self._inner.extend_session(session.session_key, int(seconds))
        session._refresh_entity(_record_value(entity))
        return _record_value(entity)

    def extend_session_with_retry(
        self,
        session: "RustSession",
        seconds: int,
        *,
        max_attempts: int = 3,
        delay_millis: int = 0,
    ) -> Dict[str, Any]:
        entity = self._inner.extend_session_with_retry(
            session.session_key,
            int(seconds),
            int(max_attempts),
            int(delay_millis),
        )
        session._refresh_entity(_record_value(entity))
        return _record_value(entity)

    def close_session(self, session: "RustSession", reason: Optional[str] = None) -> Dict[str, Any]:
        status = self._inner.close_session(session.session_key, reason)
        return _record_value(status)

    def close_sessions(self, context: "RustStoreContext", reason: Optional[str] = None) -> List[Dict[str, Any]]:
        statuses = self._inner.close_sessions(
            self._session_scope(context),
            self._session_agent_id(context),
            reason,
        )
        return _record_value(statuses)

    def cleanup_sessions(self, context: "RustStoreContext") -> Dict[str, Any]:
        return _record_value(
            self._inner.cleanup_sessions(
                self._session_scope(context),
                self._session_agent_id(context),
            )
        )

    def restart_sessions(self, context: "RustStoreContext") -> Dict[str, Any]:
        return _record_value(
            self._inner.restart_sessions(
                self._session_scope(context),
                self._session_agent_id(context),
            )
        )

    def bind_service_to_session(self, session: "RustSession", service_name: str) -> Dict[str, Any]:
        relation = self._inner.bind_service_to_session(session.session_key, service_name)
        return _record_value(relation)

    def bind_service_to_session_with_retry(
        self,
        session: "RustSession",
        service_name: str,
        *,
        max_attempts: int = 3,
        delay_millis: int = 0,
    ) -> Dict[str, Any]:
        relation = self._inner.bind_service_to_session_with_retry(
            session.session_key,
            service_name,
            int(max_attempts),
            int(delay_millis),
        )
        return _record_value(relation)

    def unbind_service_from_session(self, session: "RustSession", service_name: str) -> Dict[str, Any]:
        relation = self._inner.unbind_service_from_session(session.session_key, service_name)
        return _record_value(relation)

    def unbind_service_from_session_with_retry(
        self,
        session: "RustSession",
        service_name: str,
        *,
        max_attempts: int = 3,
        delay_millis: int = 0,
    ) -> Dict[str, Any]:
        relation = self._inner.unbind_service_from_session_with_retry(
            session.session_key,
            service_name,
            int(max_attempts),
            int(delay_millis),
        )
        return _record_value(relation)

    def list_session_services(self, session: "RustSession") -> List[Dict[str, Any]]:
        return _record_value(self._inner.list_session_services(session.session_key))

    def list_tools_in_session(self, session: "RustSession") -> List[Dict[str, Any]]:
        return _record_value(self._inner.list_tools_in_session(session.session_key))

    def get_session_state_value(self, session: "RustSession", key: str) -> Any:
        return _record_value(self._inner.get_session_state_value(session.session_key, key))

    def list_session_state(self, session: "RustSession") -> Dict[str, Any]:
        return _record_value(self._inner.list_session_state(session.session_key))

    def set_session_state(self, session: "RustSession", key: str, value: Any) -> Dict[str, Any]:
        return _record_value(self._inner.set_session_state(session.session_key, key, value))

    def set_session_state_with_retry(
        self,
        session: "RustSession",
        key: str,
        value: Any,
        *,
        max_attempts: int = 3,
        delay_millis: int = 0,
    ) -> Dict[str, Any]:
        return _record_value(
            self._inner.set_session_state_with_retry(
                session.session_key,
                key,
                value,
                int(max_attempts),
                int(delay_millis),
            )
        )

    def delete_session_state(self, session: "RustSession", key: str) -> Dict[str, Any]:
        return _record_value(self._inner.delete_session_state(session.session_key, key))

    def delete_session_state_with_retry(
        self,
        session: "RustSession",
        key: str,
        *,
        max_attempts: int = 3,
        delay_millis: int = 0,
    ) -> Dict[str, Any]:
        return _record_value(
            self._inner.delete_session_state_with_retry(
                session.session_key,
                key,
                int(max_attempts),
                int(delay_millis),
            )
        )

    def clear_session_state(self, session: "RustSession") -> Dict[str, Any]:
        return _record_value(self._inner.clear_session_state(session.session_key))

    def call_tool_in_session(
        self,
        session: "RustSession",
        tool_name: str,
        args: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        try:
            return _tool_result_value(
                self._inner.call_tool_in_session(
                    session.session_key,
                    tool_name,
                    self._normalize_optional_dict(args, "Tool arguments"),
                )
            )
        except Exception as error:
            return _tool_error_result(session.session_key, tool_name, args or {}, error)

    def set_active_session(
        self,
        context: "RustStoreContext",
        session: Optional["RustSession"],
    ) -> None:
        self._inner.set_active_session_for_context(
            session.session_key if session is not None else None,
            self._session_scope(context),
            self._session_agent_id(context),
        )

    def active_session(self, context: "RustStoreContext") -> Optional["RustSession"]:
        entity = self._inner.get_active_session_for_context(
            self._session_scope(context),
            self._session_agent_id(context),
        )
        if entity is None:
            return None
        return self._wrap_session(context, _record_value(entity))

    def enable_auto_session(self, context: "RustStoreContext", session: "RustSession") -> None:
        self._inner.enable_auto_session_for_context(
            session.session_key,
            self._session_scope(context),
            self._session_agent_id(context),
        )

    def disable_auto_session(self, context: "RustStoreContext") -> None:
        self._inner.disable_auto_session_for_context(
            self._session_scope(context),
            self._session_agent_id(context),
        )

    def is_auto_session_enabled(self, context: "RustStoreContext") -> bool:
        return bool(
            self._inner.is_auto_session_enabled_for_context(
                self._session_scope(context),
                self._session_agent_id(context),
            )
        )


class MCPStore(RustStoreBackend):
    """Public Python MCPStore facade backed by the PyO3 Rust runtime."""


class RustServiceProxy:
    def __init__(self, context: "RustStoreContext", service_name: str):
        self._context = context
        self._service_name = service_name

    def __getattr__(self, name: str) -> Any:
        if name.endswith("_async"):
            sync_name = name[:-6]
            if hasattr(self, sync_name):
                sync_method = getattr(self, sync_name)

                async def _async_wrapper(*args, **kwargs):
                    return sync_method(*args, **kwargs)

                return _async_wrapper
        raise AttributeError(name)

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

    @property
    def context_type(self) -> str:
        return self._context.context_type

    @property
    def tools_count(self) -> int:
        return len(self.list_tools())

    @property
    def is_connected(self) -> bool:
        status = self.service_status()
        health_status = status.get("health_status", status.get("status", "unknown"))
        return health_status in ("healthy", "connected", "ok", "ready")

    def __repr__(self) -> str:
        return f"RustServiceProxy(service_name={self._service_name!r}, context_type={self.context_type!r})"

    __str__ = __repr__

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
        history = self._context._tool_call_history(
            service_name=self._service_name,
            limit=1000,
        )
        error_count = sum(1 for item in history if item.get("is_error"))
        return _record_value(
            {
                "service_name": self._service_name,
                "tool_count": len(tools),
                "call_count": len(history),
                "error_count": error_count,
                "last_called_at": history[0].get("timestamp") if history else None,
                "tools": [
                    {
                        "name": tool.get("name"),
                        "description": tool.get("description"),
                        "tags": tool.get("tags", []) or [],
                    }
                    for tool in tools
                ],
                "metadata": {
                    "total_tools": len(tools),
                    "services_count": 1,
                    "tools_by_service": {self._service_name: len(tools)},
                },
                "source": "rust_event_history",
                "history_available": True,
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
            arguments,
            service_name=self._service_name,
        )

    def update_service(self, config: Any) -> bool:
        return self._context.update_service(self._service_name, config)

    def update_config(self, config: Any) -> bool:
        return self.update_service(config)

    def patch_service(self, updates: Any) -> bool:
        return self._context.patch_service(self._service_name, updates)

    def patch_config(self, updates: Any) -> bool:
        return self.patch_service(updates)

    def restart_service(self) -> bool:
        return self._context.restart_service(self._service_name)

    def refresh_content(self) -> bool:
        # Historical examples use refresh_content() as a "reload this service surface" action.
        # Restarting is the closest supported Rust-backed operation and refreshes exposed metadata.
        return self.restart_service()

    def connect_service(self) -> bool:
        return self._context.connect_service(self._service_name)

    def disconnect_service(self) -> bool:
        return self._context.disconnect_service(self._service_name)

    def delete_service(self) -> bool:
        return self._context.delete_service(self._service_name)

    def remove_service(self) -> bool:
        return self.delete_service()

    def call_tool(
        self,
        tool_name: str,
        args: Optional[Dict[str, Any]] = None,
        *,
        return_extracted: bool = False,
        **kwargs,
    ) -> Any:
        return self.find_tool(tool_name).call_tool(
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

    def hub_http(
        self,
        *,
        port: int = 8000,
        host: str = "0.0.0.0",
        path: str = "/mcp",
        block: bool = False,
        **kwargs,
    ) -> Any:
        return self._context._backend.start_mcp_server(
            agent_id=self._context.agent_id,
            service_name=self._service_name,
            transport="streamable-http",
            host=host,
            port=port,
            path=path,
            block=block,
            **kwargs,
        )

    def hub_sse(self, *args, **kwargs) -> Any:
        raise NotImplementedError(
            "Rust mcp-server does not expose SSE hub transport; use hub_http() for streamable-http"
        )

    def hub_stdio(
        self,
        *,
        block: bool = False,
        **kwargs,
    ) -> Any:
        return self._context._backend.start_mcp_server(
            agent_id=self._context.agent_id,
            service_name=self._service_name,
            transport="stdio",
            block=block,
            **kwargs,
        )


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

    def __getattr__(self, name: str) -> Any:
        if name.endswith("_async"):
            sync_name = name[:-6]
            if hasattr(self, sync_name):
                sync_method = getattr(self, sync_name)

                async def _async_wrapper(*args, **kwargs):
                    return sync_method(*args, **kwargs)

                return _async_wrapper
        raise AttributeError(name)

    @property
    def name(self) -> str:
        return self._tool_name

    @property
    def tool_name(self) -> str:
        return self._tool_name

    @property
    def service_name(self) -> Optional[str]:
        return self._service_name

    @property
    def context_type(self) -> str:
        return self._context.context_type

    @property
    def scope(self) -> str:
        return "agent" if self._context.agent_id else "store"

    @property
    def description(self) -> Optional[str]:
        return self.tool_info().get("description")

    @property
    def has_schema(self) -> bool:
        return bool(self.tool_schema())

    @property
    def is_available(self) -> bool:
        return bool(self.tool_info().get("name"))

    def __repr__(self) -> str:
        return (
            f"RustToolProxy(tool_name={self._tool_name!r}, "
            f"service_name={self._service_name!r}, scope={self.scope!r})"
        )

    __str__ = __repr__

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

    def mcp_type2tool(self):
        from mcp import types as mcp_types

        info = self.tool_info()
        name = (
            info.get("tool_original_name")
            or info.get("original_name")
            or info.get("name")
            or self._tool_name
        )
        service_name = (
            info.get("service_name")
            or info.get("global_service_name")
            or info.get("service_global_name")
            or self._service_name
        )
        meta = {
            "service_name": service_name,
            "service_global_name": (
                info.get("service_global_name") or info.get("global_service_name")
            ),
            "client_id": info.get("client_id"),
        }
        return mcp_types.Tool(
            name=str(name),
            title=info.get("title") or str(info.get("name") or self._tool_name),
            description=info.get("description"),
            inputSchema=info.get("inputSchema") or info.get("input_schema") or {},
            outputSchema=info.get("outputSchema") or info.get("output_schema"),
            icons=info.get("icons"),
            annotations=info.get("annotations"),
            _meta=meta,
        )

    def usage_stats(self) -> Dict[str, Any]:
        info = self.tool_info()
        history = self.call_history(limit=1000)
        error_count = sum(1 for item in history if item.get("is_error"))
        return _record_value(
            {
                "tool_name": info.get("name") or self._tool_name,
                "service_name": (
                    self._service_name
                    or info.get("service_name")
                    or info.get("global_service_name")
                ),
                "call_count": len(history),
                "error_count": error_count,
                "last_called_at": history[0].get("timestamp") if history else None,
                "history_available": True,
                "source": "rust_event_history",
            }
        )

    def call_history(self, limit: int = 50) -> List[Dict[str, Any]]:
        info = self.tool_info()
        service_name = (
            self._service_name
            or info.get("service_name")
            or info.get("global_service_name")
        )
        tool_names = {
            str(name)
            for name in (
                self._tool_name,
                info.get("name"),
                info.get("original_name"),
                info.get("tool_original_name"),
            )
            if name
        }
        return self._context._tool_call_history(
            service_name=service_name,
            tool_names=tool_names,
            limit=limit,
        )

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

    def transform(
        self,
        *,
        display_name: Optional[str] = None,
        description: Optional[str] = None,
        arguments: Optional[List[Dict[str, Any]]] = None,
        safety_policy: Optional[Dict[str, Any]] = None,
        tags: Optional[List[str]] = None,
        enabled: bool = True,
        **kwargs,
    ) -> Dict[str, Any]:
        _reject_unsupported_kwargs("RustToolProxy.transform", kwargs)
        info = self.tool_info()
        service_name = (
            self._service_name
            or info.get("service_name")
            or info.get("global_service_name")
            or info.get("service_global_name")
        )
        if not service_name:
            raise ValueError("Tool transform requires a service_name")
        tool_name = info.get("original_name") or info.get("tool_original_name") or self._tool_name
        return self._context.set_tool_transform(
            str(service_name),
            str(tool_name),
            display_name=display_name,
            description=description,
            arguments=arguments,
            safety_policy=safety_policy,
            tags=tags,
            enabled=enabled,
        )

    def call_tool(
        self,
        args: Optional[Dict[str, Any]] = None,
        *,
        return_extracted: bool = False,
        **kwargs,
    ) -> Any:
        _reject_unsupported_kwargs("RustToolProxy.call_tool", kwargs)
        if self._service_name:
            info = self.tool_info()
            service_name = self._context._resolve_service_name(self._service_name)
            tool_name = info.get("name") or self._tool_name
            result = self._context._backend.call_tool(service_name, tool_name, args)
        else:
            result = self._context.call_tool(self._tool_name, args)
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

    def test_call(
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

    async def dump_all_async(self) -> Dict[str, Any]:
        return self.dump_all()

    async def inspect_async(self) -> Dict[str, Any]:
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

    async def read_entity_async(
        self,
        type_name: Optional[str] = None,
        key: Optional[str] = None,
    ) -> List[Dict[str, Any]]:
        return self.read_entity(type_name, key)

    def read_relation(
        self,
        type_name: Optional[str] = None,
        key: Optional[str] = None,
    ) -> List[Dict[str, Any]]:
        return self._read_collection("relations", type_name, key)

    async def read_relation_async(
        self,
        type_name: Optional[str] = None,
        key: Optional[str] = None,
    ) -> List[Dict[str, Any]]:
        return self.read_relation(type_name, key)

    def read_state(
        self,
        type_name: Optional[str] = None,
        key: Optional[str] = None,
    ) -> List[Dict[str, Any]]:
        return self._read_collection("states", type_name, key)

    async def read_state_async(
        self,
        type_name: Optional[str] = None,
        key: Optional[str] = None,
    ) -> List[Dict[str, Any]]:
        return self.read_state(type_name, key)

    def read_event(
        self,
        type_name: Optional[str] = None,
        key: Optional[str] = None,
    ) -> List[Dict[str, Any]]:
        return self._read_collection("events", type_name, key)

    async def read_event_async(
        self,
        type_name: Optional[str] = None,
        key: Optional[str] = None,
    ) -> List[Dict[str, Any]]:
        return self.read_event(type_name, key)


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
        metrics = inspect.get("request_metrics") or {}
        return _record_value(
            {
                "backend": inspect.get("backend"),
                "namespace": inspect.get("namespace"),
                "request_metrics_available": bool(metrics.get("available")),
                "request_metrics_scope": metrics.get("scope"),
                "total_requests": metrics.get("total_requests"),
                "hits": metrics.get("hits"),
                "misses": metrics.get("misses"),
                "errors": metrics.get("errors"),
                "hit_rate": metrics.get("hit_rate"),
                "avg_latency_ms": metrics.get("avg_latency_ms"),
                "p50_latency_ms": metrics.get("p50_latency_ms"),
                "p95_latency_ms": metrics.get("p95_latency_ms"),
                "p99_latency_ms": metrics.get("p99_latency_ms"),
                "total_size_bytes": None,
                "entity_count": entity_count,
                "relation_count": relation_count,
                "state_count": state_count,
                "event_count": event_count,
            }
        )

    async def get_statistics(self) -> Dict[str, Any]:
        return await self.get_cache_statistics()

    async def reset_cache_statistics(self) -> bool:
        return self._backend.reset_cache_request_metrics()

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
            if url:
                return RedisConfig(
                    url=url,
                    password=getattr(backend, "password", None),
                    namespace=getattr(backend, "namespace", None),
                )
            host = getattr(backend, "host", None)
            if not host:
                raise ValueError("Redis cache backend 缺少 url 或 host，Rust core 不会使用隐式默认 Redis 地址")
            return RedisConfig(
                host=host,
                port=getattr(backend, "port", None),
                db=getattr(backend, "db", None),
                password=getattr(backend, "password", None),
                namespace=getattr(backend, "namespace", None),
            )

        if "memory" in class_name:
            from mcpstore.config import MemoryConfig

            return MemoryConfig()

        raise ValueError(f"Rust registry facade 无法识别 cache backend: {backend!r}")


class RustSession:
    def __init__(self, context: "RustStoreContext", entity: Dict[str, Any]):
        self._context = context
        self._entity: Dict[str, Any] = {}
        self._refresh_entity(entity)

    def _refresh_entity(self, entity: Dict[str, Any]) -> None:
        self._entity = _record_value(entity)

    @property
    def session_id(self) -> str:
        return str(self._entity["session_id"])

    @property
    def session_key(self) -> str:
        return str(self._entity["session_key"])

    @property
    def metadata(self) -> Dict[str, Any]:
        self._context._backend.get_session_entity(self)
        return _record_value(self._entity.get("metadata") or {})

    @property
    def status(self) -> str:
        return str(self._context._backend.get_session_status(self).get("status") or "active")

    @property
    def is_active(self) -> bool:
        return self.status == "active"

    def _bound_services(self) -> List[Dict[str, Any]]:
        return self._context._backend.list_session_services(self)

    @property
    def service_count(self) -> int:
        return len(self.list_services())

    @property
    def tool_count(self) -> int:
        return len(self.list_tools())

    def __enter__(self) -> "RustSession":
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
        self._context._backend.bind_service_to_session(self, resolved)
        return self

    def bind_service_with_retry(
        self,
        service_name: str,
        *,
        max_attempts: int = 3,
        delay_millis: int = 0,
    ) -> "RustSession":
        resolved = self._context._resolve_service_name(service_name)
        self._context._backend.bind_service_to_session_with_retry(
            self,
            resolved,
            max_attempts=max_attempts,
            delay_millis=delay_millis,
        )
        return self

    def unbind_service(self, service_name: str) -> "RustSession":
        resolved = self._context._resolve_service_name(service_name)
        self._context._backend.unbind_service_from_session(self, resolved)
        return self

    def unbind_service_with_retry(
        self,
        service_name: str,
        *,
        max_attempts: int = 3,
        delay_millis: int = 0,
    ) -> "RustSession":
        resolved = self._context._resolve_service_name(service_name)
        self._context._backend.unbind_service_from_session_with_retry(
            self,
            resolved,
            max_attempts=max_attempts,
            delay_millis=delay_millis,
        )
        return self

    async def bind_service_async(self, service_name: str) -> "RustSession":
        return self.bind_service(service_name)

    async def unbind_service_async(self, service_name: str) -> "RustSession":
        return self.unbind_service(service_name)

    def list_services(self) -> List[Dict[str, Any]]:
        bindings = self._bound_services()
        if not bindings:
            return self._context.list_services()
        services = []
        for binding in bindings:
            name = binding.get("service_global_name") or binding.get("service_original_name")
            info = self._context.get_service_info(str(name)) if name else None
            if info:
                services.append(info)
        return _record_value(services)

    def list_tools(self, service_name: Optional[str] = None) -> List[Dict[str, Any]]:
        if service_name is not None:
            resolved = self._context._resolve_service_name(service_name)
            return [
                tool
                for tool in self._context._backend.list_tools_in_session(self)
                if tool.get("service_name") == resolved
                or tool.get("global_service_name") == resolved
                or tool.get("service_global_name") == resolved
            ]
        return _record_value(self._context._backend.list_tools_in_session(self))

    def use_tool(
        self,
        tool_name: str,
        arguments: Optional[Dict[str, Any]] = None,
        *,
        return_extracted: bool = False,
        **kwargs,
    ) -> Any:
        _reject_unsupported_kwargs("RustSession.use_tool", kwargs)
        result = self._context._backend.call_tool_in_session(self, tool_name, arguments)
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
        self._context._backend.get_session_entity(self)
        status = self._context._backend.get_session_status(self)
        services = self._bound_services()
        return _record_value(
            {
                **dict(self._entity),
                "status": status.get("status"),
                "is_active": status.get("status") == "active",
                "agent_id": self._context.agent_id,
                "services": services,
                "service_count": self.service_count,
                "tool_count": self.tool_count,
            }
        )

    def connection_status(self) -> Dict[str, Any]:
        services = self._bound_services()
        return _record_value(
            {
                "session_id": self.session_id,
                "session_key": self.session_key,
                "is_active": self.is_active,
                "services": {
                    service.get("service_global_name"): "bound"
                    for service in services
                },
            }
        )

    def set_state(self, key: str, value: Any) -> "RustSession":
        self._context._backend.set_session_state(self, key, value)
        return self

    def set_state_with_retry(
        self,
        key: str,
        value: Any,
        *,
        max_attempts: int = 3,
        delay_millis: int = 0,
    ) -> "RustSession":
        self._context._backend.set_session_state_with_retry(
            self,
            key,
            value,
            max_attempts=max_attempts,
            delay_millis=delay_millis,
        )
        return self

    def get_state(self, key: str, default: Any = None) -> Any:
        value = self._context._backend.get_session_state_value(self, key)
        return default if value is None else value

    def list_state(self) -> Dict[str, Any]:
        return self._context._backend.list_session_state(self)

    def state_values(self) -> Dict[str, Any]:
        return _record_value(self.list_state().get("values") or {})

    def delete_state(self, key: str) -> "RustSession":
        self._context._backend.delete_session_state(self, key)
        return self

    def delete_state_with_retry(
        self,
        key: str,
        *,
        max_attempts: int = 3,
        delay_millis: int = 0,
    ) -> "RustSession":
        self._context._backend.delete_session_state_with_retry(
            self,
            key,
            max_attempts=max_attempts,
            delay_millis=delay_millis,
        )
        return self

    def restart_session(self) -> "RustSession":
        for service in self._bound_services():
            service_name = service.get("service_global_name")
            if service_name:
                self._context.restart_service(str(service_name))
        return self

    def extend_session(self, seconds: int = 3600) -> "RustSession":
        self._context._backend.extend_session(self, seconds)
        return self

    def extend_session_with_retry(
        self,
        seconds: int = 3600,
        *,
        max_attempts: int = 3,
        delay_millis: int = 0,
    ) -> "RustSession":
        self._context._backend.extend_session_with_retry(
            self,
            seconds,
            max_attempts=max_attempts,
            delay_millis=delay_millis,
        )
        return self

    def clear_cache(self) -> bool:
        self._context._backend.clear_session_state(self)
        return True

    def close_session(self) -> bool:
        self._context._backend.close_session(self)
        return True

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

    def add_service_with_details(self, config: Any = None) -> Dict[str, Any]:
        before_names = {str(service.get("name") or "") for service in self.list_services()}
        self.add_service(config)
        services = self.list_services()
        added_services = [
            service
            for service in services
            if str(service.get("name") or "") not in before_names
        ]
        tools = self._list_tools_direct()
        return _record_value(
            {
                "success": True,
                "added_services": [service.get("name") for service in added_services],
                "failed_services": [],
                "service_details": {
                    str(service.get("name")): service
                    for service in added_services
                    if service.get("name")
                },
                "total_services": len(services),
                "total_tools": len(tools),
                "services": services,
            }
        )

    def batch_add_services(self, services: Any) -> Dict[str, Any]:
        for service in services:
            self.add_service(service)
        return _record_value({"success": True, "count": len(services)})

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

    def is_session_auto(self) -> bool:
        return self._backend.is_auto_session_enabled(self)

    def close_all_sessions(self) -> "RustStoreContext":
        self._backend.close_sessions(self)
        return self

    def cleanup_sessions(self) -> "RustStoreContext":
        self._backend.cleanup_sessions(self)
        return self

    def restart_sessions(self) -> "RustStoreContext":
        self._backend.restart_sessions(self)
        return self

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
        return _record_value(
            self._backend.list_tools_scoped(self._agent_id, service_name, filter=filter)
        )

    def list_tools(self, service_name: Optional[str] = None, *, filter: str = "available") -> List[Dict[str, Any]]:
        active = self.active_session
        if service_name is None and active is not None:
            return active.list_tools()
        return self._list_tools_direct(service_name, filter=filter)

    def _tool_visibility_for(self, service_name: str) -> Optional[set[str]]:
        visibility = self._backend.get_context_tool_visibility(self._agent_id, service_name)
        if not visibility:
            return None
        return {
            str(name)
            for tool in visibility.get("tools", [])
            for name in (
                tool.get("tool_original_name"),
                tool.get("tool_global_name"),
            )
            if name
        }

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
        if visible is None:
            self._backend.clear_context_tool_visibility(self._agent_id, service_name)
        else:
            self._backend.set_context_tool_visibility(
                self._agent_id,
                service_name,
                sorted(str(tool) for tool in visible),
            )

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

    def get_tools_with_stats(self) -> Dict[str, Any]:
        tools = self.list_tools()
        tools_by_service: Dict[str, int] = {}
        for tool in tools:
            service_name = str(
                tool.get("service_name")
                or tool.get("serviceName")
                or tool.get("global_service_name")
                or ""
            )
            if not service_name:
                continue
            tools_by_service[service_name] = tools_by_service.get(service_name, 0) + 1
        return _record_value(
            {
                "tools": tools,
                "metadata": {
                    "total_tools": len(tools),
                    "services_count": len(tools_by_service),
                    "tools_by_service": tools_by_service,
                },
            }
        )

    def get_system_stats(self) -> Dict[str, Any]:
        return self.get_stats()

    def create_session(
        self,
        session_id: str,
        *,
        user_session_id: Optional[str] = None,
        lease_seconds: Optional[int] = None,
        metadata: Optional[Dict[str, Any]] = None,
    ) -> RustSession:
        normalized_metadata = self._backend._normalize_optional_dict(
            metadata,
            "Session metadata",
        )
        if user_session_id is not None:
            existing_user_session_id = normalized_metadata.get("user_session_id")
            if (
                existing_user_session_id is not None
                and existing_user_session_id != user_session_id
            ):
                raise ValueError(
                    "Session metadata user_session_id conflicts with create_session(user_session_id)"
                )
            normalized_metadata = dict(normalized_metadata)
            normalized_metadata["user_session_id"] = user_session_id
        return self._backend.get_session(
            self,
            session_id,
            lease_seconds=lease_seconds,
            metadata=normalized_metadata,
        )

    def find_session(
        self,
        session_id: Optional[str] = None,
        *,
        is_user_session_id: bool = False,
    ) -> Optional[RustSession]:
        if is_user_session_id:
            if session_id is None:
                raise ValueError("find_session(is_user_session_id=True) requires session_id")
            return self.find_user_session(session_id)
        return self._backend.find_session(self, session_id)

    def find_user_session(self, user_session_id: str) -> Optional[RustSession]:
        return self._backend.find_session_by_user_session_id(self, user_session_id)

    def register_session_globally(self, session_id: str, global_id: str) -> bool:
        session = self.find_session(session_id)
        if session is None:
            return False
        metadata = dict(session.metadata)
        metadata["user_session_id"] = global_id
        self._backend.update_session_metadata(session, metadata)
        return True

    def get_session(self, session_id: str) -> RustSession:
        return self.create_session(session_id)

    def list_sessions(self) -> List[RustSession]:
        return self._backend.list_sessions(self)

    def list_agent_sessions(self) -> List[RustSession]:
        return self.list_sessions()

    def get_session_statistics(self) -> Dict[str, Any]:
        sessions = self.list_sessions()
        active = [session for session in sessions if session.is_active]
        auto_session_enabled = self.is_session_auto()
        return _record_value(
            {
                "total_sessions": len(sessions),
                "active_sessions": len(active),
                "context_sessions": len(sessions),
                "auto_session_enabled": auto_session_enabled,
                "cached_session_objects": 0,
                "context_info": {
                    "context_type": self.context_type,
                    "agent_id": self.get_id(),
                    "context_sessions": len(sessions),
                    "auto_session_enabled": auto_session_enabled,
                    "cached_session_objects": 0,
                },
                "sessions": sessions,
            }
        )

    def with_session(self, session_id: str) -> RustSession:
        return self.create_session(session_id)

    async def with_session_async(self, session_id: str) -> RustSession:
        return self.with_session(session_id)

    def create_shared_session(self, session_id: str, shared_id: str) -> RustSession:
        return self.create_session(
            session_id,
            metadata={"user_session_id": shared_id},
        )

    def for_langchain_with_session(
        self,
        session_id: str,
        create_if_not_exists: bool = True,
    ):
        session = self.find_session(session_id)
        if session is None:
            if not create_if_not_exists:
                raise ValueError(f"Session not found: {session_id}")
            session = self.with_session(session_id)
        return session.for_langchain()

    def for_langchain_with_auto_session(self):
        session = self.current_session()
        if session is None:
            self.session_auto()
            session = self.current_session()
        return session.for_langchain()

    def for_langchain_with_shared_session(self, shared_id: str):
        session = self.find_user_session(shared_id)
        if session is None:
            session = self.create_shared_session(shared_id, shared_id)
        return session.for_langchain()

    def session_auto(
        self,
        session_id: Optional[str] = None,
        default_timeout: int = 720000,
        auto_cleanup: bool = False,
        session_prefix: str = "auto_",
    ) -> "RustStoreContext":
        resolved_session_id = session_id if session_id is not None else f"{session_prefix}session_default"
        metadata = {
            "created_by": "python_session_auto",
            "auto_cleanup": bool(auto_cleanup),
            "session_prefix": session_prefix,
        }
        session = self.create_session(
            resolved_session_id,
            lease_seconds=int(default_timeout),
            metadata=metadata,
        )
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

    def get_stats(self) -> Dict[str, Any]:
        services = self.list_services()
        tools = self.list_tools()
        service_health = self.check_services()

        healthy_services = 0
        unhealthy_services = 0
        if isinstance(service_health, dict):
            for value in service_health.values():
                if _status_value(value) in {"healthy", "ready", "connected", "ok"}:
                    healthy_services += 1
                else:
                    unhealthy_services += 1

        history = self._tool_call_history(limit=1000)
        events = self.event_history(1000)
        last_activity = None
        for event in events:
            timestamp = _event_timestamp(event)
            if timestamp is not None:
                last_activity = timestamp
                break

        return _record_value(
            {
                "agent_id": self._agent_id,
                "service_count": len(services),
                "tool_count": len(tools),
                "healthy_services": healthy_services,
                "unhealthy_services": unhealthy_services,
                "total_tool_executions": len(history),
                "is_active": bool(services) or self.active_session is not None or bool(self.list_sessions()),
                "last_activity": last_activity,
            }
        )

    def get_agents_summary(self) -> Dict[str, Any]:
        agents = []
        total_services = 0
        total_tools = 0
        active_agents = 0
        for item in self.list_agents():
            agent_id = item.get("agent_id") or item.get("id")
            if not agent_id:
                continue
            stats = self.find_agent(str(agent_id)).get_stats()
            agents.append(stats)
            total_services += int(stats.get("service_count") or 0)
            total_tools += int(stats.get("tool_count") or 0)
            if stats.get("is_active"):
                active_agents += 1
        store_stats = self.get_stats()
        return _record_value(
            {
                "total_agents": len(agents),
                "active_agents": active_agents,
                "total_services": total_services,
                "total_tools": total_tools,
                "store_services": store_stats.get("service_count", 0),
                "store_tools": store_stats.get("tool_count", 0),
                "agents": agents,
            }
        )

    def list_resources(self, service_name: Optional[str] = None) -> List[Dict[str, Any]]:
        return _record_value(
            self._backend.list_resources_scoped(self._agent_id, service_name)
        )

    def list_changed_tools(
        self,
        service_name: Optional[str] = None,
        force_refresh: bool = False,
    ) -> Dict[str, Any]:
        return _record_value(
            self._backend.list_changed_tools_scoped(
                self._agent_id,
                service_name,
                force_refresh=force_refresh,
            )
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
                arguments,
                service_name,
            )
        )

    def patch_service(self, name: str, updates: Any) -> bool:
        service_name = self._resolve_service_name(name)
        return self._backend.patch_service(service_name, updates)

    def update_service(self, name: str, config: Any) -> bool:
        service_name = self._resolve_service_name(name)
        return self._backend.update_service(service_name, config)

    def update_config(self, name: str, config: Any) -> bool:
        return self.update_service(name, config)

    def delete_service(self, name: str) -> bool:
        service_name = self._resolve_service_name(name)
        return self._backend.remove_service(service_name)

    def remove_service(self, name: str) -> bool:
        return self.delete_service(name)

    def delete_config(self, name: str) -> bool:
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

    def _tool_call_history(
        self,
        *,
        service_name: Optional[str] = None,
        tool_names: Optional[set[str]] = None,
        limit: int = 50,
    ) -> List[Dict[str, Any]]:
        records = []
        tool_names = {str(name) for name in tool_names or set() if name}
        for event in self.event_history(1000):
            if tool_names:
                if not _event_matches_tool(event, service_name, tool_names):
                    continue
            elif not _is_tool_call_event(event):
                continue
            elif service_name and not _event_matches_service(event, service_name):
                continue
            records.append(_tool_call_history_record(event))
            if len(records) >= limit:
                break
        return _record_value(records)

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
        return self._backend.call_tool(service_name, original_tool, args)

    def set_tool_transform(
        self,
        service_name: str,
        tool_name: str,
        *,
        display_name: Optional[str] = None,
        description: Optional[str] = None,
        arguments: Optional[List[Dict[str, Any]]] = None,
        safety_policy: Optional[Dict[str, Any]] = None,
        tags: Optional[List[str]] = None,
        enabled: bool = True,
    ) -> Dict[str, Any]:
        resolved_service = self._resolve_service_name(service_name)
        payload = {
            "display_name": display_name,
            "description": description,
            "arguments": arguments or [],
            "safety_policy": safety_policy,
            "tags": tags or [],
            "enabled": enabled,
        }
        return self._backend.set_tool_transform(resolved_service, tool_name, payload)

    def create_llm_friendly_tool_transform(
        self,
        service_name: str,
        tool_name: str,
        friendly_name: Optional[str] = None,
        description: Optional[str] = None,
        hide_technical_params: bool = True,
        add_safety_policy: bool = True,
    ) -> Dict[str, Any]:
        return self._backend.create_llm_friendly_tool_transform(
            self._resolve_service_name(service_name),
            tool_name,
            friendly_name,
            description,
            hide_technical_params,
            add_safety_policy,
        )

    def create_parameter_renamed_tool_transform(
        self,
        service_name: str,
        tool_name: str,
        parameter_mapping: Dict[str, str],
        new_tool_name: Optional[str] = None,
    ) -> Dict[str, Any]:
        return self._backend.create_parameter_renamed_tool_transform(
            self._resolve_service_name(service_name),
            tool_name,
            parameter_mapping,
            new_tool_name,
        )

    def create_validated_tool_transform(
        self,
        service_name: str,
        tool_name: str,
        validation_rules: Dict[str, Any],
        new_tool_name: Optional[str] = None,
    ) -> Dict[str, Any]:
        return self._backend.create_validated_tool_transform(
            self._resolve_service_name(service_name),
            tool_name,
            validation_rules,
            new_tool_name,
        )

    def get_tool_transform(self, service_name: str, tool_name: str) -> Optional[Dict[str, Any]]:
        return self._backend.get_tool_transform(self._resolve_service_name(service_name), tool_name)

    def list_tool_transforms(self) -> List[Dict[str, Any]]:
        return self._backend.list_tool_transforms()

    def delete_tool_transform(self, service_name: str, tool_name: str) -> bool:
        return self._backend.delete_tool_transform(self._resolve_service_name(service_name), tool_name)

    def import_openapi_service(
        self,
        name: str,
        spec_url: str,
        *,
        headers: Optional[Dict[str, str]] = None,
        auth: Optional[Dict[str, Any]] = None,
        ref_cache: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        result = self._backend.import_openapi_service(
            name,
            spec_url,
            headers=headers,
            auth=auth,
            ref_cache=ref_cache,
        )
        return result

    def import_api(
        self,
        api_url: str,
        api_name: Optional[str] = None,
        *,
        headers: Optional[Dict[str, str]] = None,
        auth: Optional[Dict[str, Any]] = None,
        ref_cache: Optional[Dict[str, Any]] = None,
    ) -> "RustStoreContext":
        import time

        name = api_name or f"api_{int(time.time())}"
        self.import_openapi_service(
            name,
            api_url,
            headers=headers,
            auth=auth,
            ref_cache=ref_cache,
        )
        return self

    def import_openapi_service_from_spec(
        self,
        name: str,
        spec_url: str,
        spec: Any,
        *,
        headers: Optional[Dict[str, str]] = None,
        auth: Optional[Dict[str, Any]] = None,
        ref_cache: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        result = self._backend.import_openapi_service_from_spec(
            name,
            spec_url,
            spec,
            headers=headers,
            auth=auth,
            ref_cache=ref_cache,
        )
        return result

    def import_api_from_spec(
        self,
        spec: Any,
        api_name: str,
        spec_url: str = "memory://openapi",
        *,
        headers: Optional[Dict[str, str]] = None,
        auth: Optional[Dict[str, Any]] = None,
        ref_cache: Optional[Dict[str, Any]] = None,
    ) -> "RustStoreContext":
        self.import_openapi_service_from_spec(
            api_name,
            spec_url,
            spec,
            headers=headers,
            auth=auth,
            ref_cache=ref_cache,
        )
        return self

    def get_openapi_import(self, name: str) -> Optional[Dict[str, Any]]:
        return self._backend.get_openapi_import(name)

    def list_openapi_imports(self) -> List[Dict[str, Any]]:
        return self._backend.list_openapi_imports()

    def last_openapi_import(self) -> Optional[Dict[str, Any]]:
        return self._backend.last_openapi_import()

    def bundle_openapi_spec(
        self,
        spec_url: str,
        *,
        ref_cache: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        return self._backend.bundle_openapi_spec(spec_url, ref_cache=ref_cache)

    def bundle_openapi_spec_from_spec(
        self,
        spec_url: str,
        spec: Any,
        *,
        ref_cache: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        return self._backend.bundle_openapi_spec_from_spec(spec_url, spec, ref_cache=ref_cache)

    def bundle_openapi_artifact(
        self,
        spec_url: str,
        *,
        ref_cache: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        return self._backend.bundle_openapi_artifact(spec_url, ref_cache=ref_cache)

    def bundle_openapi_artifact_from_spec(
        self,
        spec_url: str,
        spec: Any,
        *,
        ref_cache: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        return self._backend.bundle_openapi_artifact_from_spec(
            spec_url,
            spec,
            ref_cache=ref_cache,
        )

    def call_tool(
        self,
        tool_name: str,
        args: Optional[Dict[str, Any]] = None,
        *,
        return_extracted: bool = False,
        **kwargs,
    ) -> Any:
        _reject_unsupported_kwargs("RustStoreContext.call_tool", kwargs)
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

    def _set_tool_override(
        self,
        service_name: str,
        tool_name: str,
        flag: str,
        value: Any,
    ) -> None:
        self._backend.set_tool_preference(
            self._agent_id,
            service_name,
            tool_name,
            flag,
            value,
        )

    def get_tool_override(
        self,
        service_name: str,
        tool_name: str,
        flag: str,
        default: Any = None,
    ) -> Any:
        keys = [
            (service_name, tool_name),
            ("", tool_name),
        ]
        for service, tool in keys:
            try:
                state = self._backend.get_tool_preferences(
                    self._agent_id,
                    service,
                    tool,
                )
            except Exception:
                state = None
            preferences = state.get("preferences", {}) if state else {}
            if flag in preferences:
                return preferences[flag]
        return default

    def show_config(self, scope: str = "all") -> Dict[str, Any]:
        if self._agent_id:
            return self._backend.show_config_scoped(self._agent_id, scope)
        return self._backend.show_config(scope)

    def show_mcpjson(self) -> Dict[str, Any]:
        return self._backend.show_mcpjson()

    def show_mcpconfig(self) -> Dict[str, Any]:
        return self.show_mcpjson()

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
        port: int = 8000,
        host: str = "0.0.0.0",
        path: str = "/mcp",
        block: bool = False,
        **kwargs,
    ) -> Any:
        return self._backend.start_mcp_server(
            agent_id=self._agent_id,
            transport="streamable-http",
            host=host,
            port=port,
            path=path,
            block=block,
            **kwargs,
        )

    def hub_sse(
        self,
        *,
        port: int = 8000,
        host: str = "0.0.0.0",
        path: str = "/mcp",
        block: bool = False,
        **kwargs,
    ) -> Any:
        raise NotImplementedError(
            "Rust mcp-server does not expose SSE hub transport; use hub_http() for streamable-http"
        )

    def hub_stdio(
        self,
        *,
        block: bool = False,
        **kwargs,
    ) -> Any:
        return self._backend.start_mcp_server(
            agent_id=self._agent_id,
            transport="stdio",
            block=block,
            **kwargs,
        )

    def reset_config(self) -> bool:
        if self._agent_id:
            return self._backend.reset_agent_config(self._agent_id)
        return self._backend.reset_config()

    def reset_mcp_json_scope(self, scope: Optional[str] = None) -> bool:
        return self._backend.reset_mcp_json_scope(scope)

    def reset_mcp_json_file(self) -> bool:
        return self.reset_mcp_json_scope("all")

    async def reset_mcp_json_file_async(self, scope: str = "all") -> bool:
        return self.reset_mcp_json_scope(scope)

    def get_id(self) -> str:
        return self._agent_id or "global_agent_store"

    def map_global(self, name: str) -> str:
        value = str(name)
        if not self._agent_id or self._agent_id == _GLOBAL_AGENT_STORE:
            return value
        if _AGENT_SEPARATOR in value:
            return value
        if ":" in value:
            parsed_agent, local_name = value.split(":", 1)
            if parsed_agent and local_name and parsed_agent == self._agent_id:
                return f"{local_name}{_AGENT_SEPARATOR}{self._agent_id}"
            return value
        return f"{value}{_AGENT_SEPARATOR}{self._agent_id}"

    def map_local(self, name: str) -> str:
        value = str(name)
        if not self._agent_id or self._agent_id == _GLOBAL_AGENT_STORE:
            return value
        suffix = f"{_AGENT_SEPARATOR}{self._agent_id}"
        if value.endswith(suffix):
            return value[: -len(suffix)]
        if ":" in value:
            parsed_agent, local_name = value.split(":", 1)
            if parsed_agent == self._agent_id and local_name:
                return local_name
        for service in self.list_services():
            if service.get("global_name") == value or service.get("client_id") == value:
                return service.get("name") or service.get("original_name") or value
        return value

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

    def init_service(
        self,
        name: Optional[str] = None,
        *,
        client_id: Optional[str] = None,
        service_name: Optional[str] = None,
    ) -> Dict[str, Any]:
        target = service_name or client_id or name
        if not target:
            raise ValueError("init_service requires a service name")
        return self.wait_service(target)

    def _resolve_tool(self, tool_name: str) -> tuple[str, str]:
        active = self.active_session
        if active is not None:
            for service in active.list_services():
                service_name = str(service.get("name") or service.get("service_global_name") or "")
                if not service_name:
                    continue
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
