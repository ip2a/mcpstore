"""Explicit Python facade for the Rust MCPStore core.

Configuration operations address definitions by service name and scopes by a
structured ``ScopeRef``. Runtime operations address one concrete instance by
``instance_id``. The facade deliberately performs no name parsing or implicit
scope selection.
"""

from __future__ import annotations

import importlib
import os
from typing import Any, Dict, List, Optional
from urllib.parse import quote

from pydantic import BaseModel, TypeAdapter

from mcpstore.core.models import ScopeDescriptor, ScopeRef


_SCOPE_ADAPTER: TypeAdapter[ScopeRef] = TypeAdapter(ScopeRef)


class RustRecordView(dict):
    """Dictionary payload with attribute access and plain-dict conversion."""

    def __getattr__(self, name: str) -> Any:
        if name == "text_output":
            return _extract_text_result(self)
        try:
            return self[name]
        except KeyError as exc:
            raise AttributeError(name) from exc

    def to_dict(self) -> Dict[str, Any]:
        return {key: _plain_record_value(value) for key, value in self.items()}


def _record_value(value: Any) -> Any:
    if isinstance(value, dict):
        return RustRecordView({key: _record_value(item) for key, item in value.items()})
    if isinstance(value, list):
        return [_record_value(item) for item in value]
    return value


def _plain_record_value(value: Any) -> Any:
    if isinstance(value, dict):
        return {key: _plain_record_value(item) for key, item in value.items()}
    if isinstance(value, list):
        return [_plain_record_value(item) for item in value]
    return value


def _extract_text_result(result: Any) -> str:
    content = result.get("content", []) if isinstance(result, dict) else []
    return "\n".join(
        item.get("text", "")
        for item in content
        if isinstance(item, dict) and item.get("type") == "text" and item.get("text")
    )


def _dict_payload(value: Any, context: str) -> Dict[str, Any]:
    if isinstance(value, BaseModel):
        value = value.model_dump(mode="json", exclude_none=False)
    if not isinstance(value, dict):
        raise TypeError(f"{context} must be a dictionary")
    return _plain_record_value(value)


def _base_config_payload(value: Any, context: str) -> Dict[str, Any]:
    payload = _dict_payload(value, context)
    if "_mcpstore" in payload:
        raise ValueError(
            f"{context} only accepts base MCP fields; change scopes with "
            "declare_service_scope() or remove_service_scope()"
        )
    return payload


def _scope_payload(scope: ScopeRef | Dict[str, Any]) -> Dict[str, Any]:
    validated = _SCOPE_ADAPTER.validate_python(scope)
    return _SCOPE_ADAPTER.dump_python(validated, mode="json")


def _descriptor_payload(
    descriptor: ScopeDescriptor | Dict[str, Any],
) -> Dict[str, Any]:
    validated = ScopeDescriptor.model_validate(descriptor)
    return validated.model_dump(mode="json", exclude_none=False)


class RustStoreBackend:
    """Thin, explicit wrapper over ``mcpstore._rust.MCPStore``."""

    def __init__(self, rust_store: Any):
        self._inner = rust_store
        self._config_path: Optional[str] = None
        self._cache_config: Any = None
        self._only_db = False

    @classmethod
    def setup(
        cls,
        config_path: Optional[str] = None,
        cache_config: Any = None,
        only_db: bool = False,
    ) -> "RustStoreBackend":
        rust_mod = importlib.import_module("mcpstore._rust")
        normalized_path = os.fspath(config_path) if config_path is not None else None
        normalized_cache = cls._normalize_cache_config(cache_config)
        backend, redis_url, namespace = cls._cache_options(normalized_cache)
        rust_store = rust_mod.MCPStore.setup_with_options(
            normalized_path,
            "db" if only_db else "local",
            backend,
            redis_url,
            namespace,
        )
        store = cls(rust_store)
        store._config_path = normalized_path
        store._cache_config = normalized_cache
        store._only_db = only_db
        store.load_from_config()
        return store

    @staticmethod
    def setup_store(
        mcpjson_path: str | None = None,
        debug: bool | str = False,
        cache: Any = None,
        static_config: Optional[Dict[str, Any]] = None,
        cache_mode: str = "auto",
        only_db: bool = False,
        **kwargs: Any,
    ) -> "RustStoreBackend":
        from mcpstore.core.store.setup_manager import StoreSetupManager

        return StoreSetupManager.setup_store(
            mcpjson_path=mcpjson_path,
            debug=debug,
            cache=cache,
            static_config=static_config,
            cache_mode=cache_mode,
            only_db=only_db,
            **kwargs,
        )

    @staticmethod
    async def setup_store_async(
        mcpjson_path: str | None = None,
        debug: bool | str = False,
        cache: Any = None,
        static_config: Optional[Dict[str, Any]] = None,
        cache_mode: str = "auto",
        only_db: bool = False,
        **kwargs: Any,
    ) -> "RustStoreBackend":
        from mcpstore.core.store.setup_manager import StoreSetupManager

        return await StoreSetupManager.setup_store_async(
            mcpjson_path=mcpjson_path,
            debug=debug,
            cache=cache,
            static_config=static_config,
            cache_mode=cache_mode,
            only_db=only_db,
            **kwargs,
        )

    @classmethod
    def _normalize_cache_config(cls, cache_config: Any) -> Any:
        if cache_config is None or hasattr(cache_config, "cache_type"):
            return cache_config
        if isinstance(cache_config, str):
            cache_type = cache_config.strip().lower()
            if cache_type == "memory":
                from mcpstore.config import MemoryConfig

                return MemoryConfig()
            if cache_type == "openkeyv_memory":
                from mcpstore.config import OpenKeyvMemoryConfig

                return OpenKeyvMemoryConfig()
            raise ValueError(f"Unsupported Rust cache type: {cache_config!r}")
        if isinstance(cache_config, dict):
            raw_type = cache_config.get("type", cache_config.get("cache_type"))
            dict_cache_type = getattr(raw_type, "value", raw_type)
            options = dict(cache_config)
            options.pop("type", None)
            options.pop("cache_type", None)
            if dict_cache_type == "memory":
                from mcpstore.config import MemoryConfig

                return MemoryConfig(**options)
            if dict_cache_type == "openkeyv_memory":
                from mcpstore.config import OpenKeyvMemoryConfig

                return OpenKeyvMemoryConfig(**options)
            if dict_cache_type == "redis":
                from mcpstore.config import RedisConfig

                return RedisConfig(**options)
            if dict_cache_type == "openkeyv_redis":
                from mcpstore.config import OpenKeyvRedisConfig

                return OpenKeyvRedisConfig(**options)
            raise ValueError(f"Unsupported Rust cache type: {dict_cache_type!r}")
        return cache_config

    @staticmethod
    def _redis_url(cache_config: Any) -> Optional[str]:
        redis_url = getattr(cache_config, "url", None)
        if redis_url:
            return str(redis_url)
        host = getattr(cache_config, "host", None)
        if not host:
            return None
        port = getattr(cache_config, "port", None) or 6379
        db = getattr(cache_config, "db", None) or 0
        password = getattr(cache_config, "password", None)
        auth = f":{quote(str(password), safe='')}@" if password else ""
        return f"redis://{auth}{host}:{port}/{db}"

    @classmethod
    def _cache_options(cls, cache_config: Any) -> tuple[Optional[str], Optional[str], Optional[str]]:
        if cache_config is None:
            return None, None, None
        raw_type = getattr(cache_config, "cache_type", None)
        cache_type = getattr(raw_type, "value", raw_type)
        if not isinstance(cache_type, str):
            raise ValueError(f"Unsupported Rust cache type: {cache_type!r}")
        backend = {
            "memory": "memory",
            "openkeyv_memory": "openkeyv-memory",
            "redis": "redis",
            "openkeyv_redis": "openkeyv-redis",
        }.get(cache_type)
        if backend is None:
            raise ValueError(f"Unsupported Rust cache type: {cache_type!r}")
        redis_url = cls._redis_url(cache_config) if "redis" in backend else None
        if "redis" in backend and not redis_url:
            raise ValueError("Redis cache configuration requires url or host")
        namespace = getattr(cache_config, "namespace", None)
        return backend, redis_url, namespace

    def namespace(self) -> str:
        return str(self._inner.namespace())

    def current_backend(self) -> str:
        return str(self._inner.current_backend())

    def load_from_config(self) -> None:
        self._inner.load_from_config()

    # Definition and scope configuration
    def add_service(self, service_name: str, config: Dict[str, Any]) -> None:
        self._inner.add_service(service_name, _dict_payload(config, "Service config"))

    async def add_service_async(self, service_name: str, config: Dict[str, Any]) -> None:
        self.add_service(service_name, config)

    def declare_service_scope(
        self,
        service_name: str,
        scope: ScopeRef | Dict[str, Any],
        descriptor: ScopeDescriptor | Dict[str, Any],
    ) -> str:
        return str(
            self._inner.declare_service_scope(
                service_name,
                _scope_payload(scope),
                _descriptor_payload(descriptor),
            )
        )

    async def declare_service_scope_async(
        self,
        service_name: str,
        scope: ScopeRef | Dict[str, Any],
        descriptor: ScopeDescriptor | Dict[str, Any],
    ) -> str:
        return self.declare_service_scope(service_name, scope, descriptor)

    def remove_service_scope(
        self,
        service_name: str,
        scope: ScopeRef | Dict[str, Any],
    ) -> None:
        self._inner.remove_service_scope(service_name, _scope_payload(scope))

    async def remove_service_scope_async(
        self,
        service_name: str,
        scope: ScopeRef | Dict[str, Any],
    ) -> None:
        self.remove_service_scope(service_name, scope)

    def patch_service(self, service_name: str, base_updates: Dict[str, Any]) -> None:
        self._inner.patch_service(
            service_name,
            _base_config_payload(base_updates, "Service config patch"),
        )

    async def patch_service_async(self, service_name: str, base_updates: Dict[str, Any]) -> None:
        self.patch_service(service_name, base_updates)

    def update_service(self, service_name: str, config: Dict[str, Any]) -> None:
        self._inner.update_service(
            service_name,
            _base_config_payload(config, "Service config update"),
        )

    async def update_service_async(self, service_name: str, config: Dict[str, Any]) -> None:
        self.update_service(service_name, config)

    def remove_service(self, service_name: str) -> None:
        self._inner.remove_service(service_name)

    async def remove_service_async(self, service_name: str) -> None:
        self.remove_service(service_name)

    def get_definition_config(self, service_name: str) -> Optional[Dict[str, Any]]:
        return _record_value(self._inner.get_definition_config(service_name))

    def get_effective_config(
        self,
        service_name: str,
        scope: ScopeRef | Dict[str, Any],
    ) -> Optional[Dict[str, Any]]:
        return _record_value(
            self._inner.get_effective_config(service_name, _scope_payload(scope))
        )

    def show_config(self) -> Dict[str, Any]:
        return _record_value(self._inner.show_config())

    def show_scope_config(self, scope: ScopeRef | Dict[str, Any]) -> Dict[str, Any]:
        return _record_value(self._inner.show_scope_config(_scope_payload(scope)))

    def reset_config(self) -> None:
        self._inner.reset_config()

    def reset_scope(self, scope: ScopeRef | Dict[str, Any]) -> None:
        self._inner.reset_scope(_scope_payload(scope))

    # Instance runtime
    def list_instances(self) -> List[Dict[str, Any]]:
        return _record_value(self._inner.list_instances())

    async def list_instances_async(self) -> List[Dict[str, Any]]:
        return self.list_instances()

    def list_instances_scoped(
        self,
        scope: ScopeRef | Dict[str, Any],
    ) -> List[Dict[str, Any]]:
        return _record_value(self._inner.list_instances_scoped(_scope_payload(scope)))

    def find_instance(self, instance_id: str) -> Optional[Dict[str, Any]]:
        return _record_value(self._inner.find_instance(instance_id))

    def instance_info(self, instance_id: str) -> Dict[str, Any]:
        return _record_value(self._inner.instance_info(instance_id))

    def connect_service(self, instance_id: str) -> None:
        self._inner.connect_service(instance_id)

    async def connect_service_async(self, instance_id: str) -> None:
        self.connect_service(instance_id)

    def disconnect_service(self, instance_id: str) -> None:
        self._inner.disconnect_service(instance_id)

    async def disconnect_service_async(self, instance_id: str) -> None:
        self.disconnect_service(instance_id)

    def restart_service(self, instance_id: str) -> None:
        self._inner.restart_service(instance_id)

    async def restart_service_async(self, instance_id: str) -> None:
        self.restart_service(instance_id)

    def wait_instance_ready(self, instance_id: str, timeout_secs: int = 10) -> Dict[str, Any]:
        return _record_value(self._inner.wait_instance_ready(instance_id, timeout_secs))

    async def wait_instance_ready_async(
        self,
        instance_id: str,
        timeout_secs: int = 10,
    ) -> Dict[str, Any]:
        return self.wait_instance_ready(instance_id, timeout_secs)

    def check_instances(self, instance_ids: List[str]) -> Dict[str, Any]:
        return _record_value(self._inner.check_instances(instance_ids))

    def service_state(self, instance_id: str) -> Dict[str, Any]:
        return _record_value(self._inner.service_state(instance_id))

    def list_tools(self, instance_id: str) -> List[Dict[str, Any]]:
        tools = _record_value(self._inner.list_tools(instance_id))
        for tool in tools:
            tool["instance_id"] = instance_id
        return tools

    async def list_tools_async(self, instance_id: str) -> List[Dict[str, Any]]:
        return self.list_tools(instance_id)

    def list_tool_entries(
        self,
        instance_id: str,
        *,
        filter: str = "all",
    ) -> List[Dict[str, Any]]:
        return _record_value(self._inner.list_tool_entries(instance_id, filter=filter))

    def list_changed_tools(
        self,
        instance_id: str,
        *,
        force_refresh: bool = False,
    ) -> Dict[str, Any]:
        return _record_value(
            self._inner.list_changed_tools(
                instance_id,
                force_refresh=force_refresh,
            )
        )

    def call_tool(
        self,
        instance_id: str,
        tool_name: str,
        args: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        return _record_value(self._inner.call_tool(instance_id, tool_name, args or {}))

    async def call_tool_async(
        self,
        instance_id: str,
        tool_name: str,
        args: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        return self.call_tool(instance_id, tool_name, args)

    def list_resources(self, instance_id: str) -> List[Dict[str, Any]]:
        return _record_value(self._inner.list_resources(instance_id))

    async def list_resources_async(self, instance_id: str) -> List[Dict[str, Any]]:
        return self.list_resources(instance_id)

    def list_resource_templates(self, instance_id: str) -> List[Dict[str, Any]]:
        return _record_value(self._inner.list_resource_templates(instance_id))

    def read_resource(self, instance_id: str, uri: str) -> Dict[str, Any]:
        return _record_value(self._inner.read_resource(instance_id, uri))

    async def read_resource_async(self, instance_id: str, uri: str) -> Dict[str, Any]:
        return self.read_resource(instance_id, uri)

    def list_prompts(self, instance_id: str) -> List[Dict[str, Any]]:
        return _record_value(self._inner.list_prompts(instance_id))

    async def list_prompts_async(self, instance_id: str) -> List[Dict[str, Any]]:
        return self.list_prompts(instance_id)

    def get_prompt(
        self,
        instance_id: str,
        prompt_name: str,
        arguments: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        return _record_value(
            self._inner.get_prompt(instance_id, prompt_name, arguments or {})
        )

    def export_instance_config(
        self,
        instance_id: str,
        format: Optional[str] = None,
    ) -> Dict[str, Any]:
        return _record_value(self._inner.export_instance_config(instance_id, format))

    # Tool transforms are instance-owned.
    def set_tool_transform(
        self,
        instance_id: str,
        tool_name: str,
        transform: Dict[str, Any],
    ) -> Dict[str, Any]:
        return _record_value(
            self._inner.set_tool_transform(instance_id, tool_name, transform)
        )

    def create_llm_friendly_tool_transform(
        self,
        instance_id: str,
        tool_name: str,
        *,
        friendly_name: Optional[str] = None,
        description: Optional[str] = None,
        hide_technical_params: bool = True,
        add_safety_policy: bool = True,
    ) -> Dict[str, Any]:
        return _record_value(
            self._inner.create_llm_friendly_tool_transform(
                instance_id,
                tool_name,
                friendly_name,
                description,
                hide_technical_params,
                add_safety_policy,
            )
        )

    def create_parameter_renamed_tool_transform(
        self,
        instance_id: str,
        tool_name: str,
        parameter_mapping: Dict[str, str],
        *,
        new_tool_name: Optional[str] = None,
    ) -> Dict[str, Any]:
        return _record_value(
            self._inner.create_parameter_renamed_tool_transform(
                instance_id,
                tool_name,
                parameter_mapping,
                new_tool_name,
            )
        )

    def create_validated_tool_transform(
        self,
        instance_id: str,
        tool_name: str,
        validation_rules: Dict[str, Any],
        *,
        new_tool_name: Optional[str] = None,
    ) -> Dict[str, Any]:
        return _record_value(
            self._inner.create_validated_tool_transform(
                instance_id,
                tool_name,
                validation_rules,
                new_tool_name,
            )
        )

    def get_tool_transform(
        self,
        instance_id: str,
        tool_name: str,
    ) -> Optional[Dict[str, Any]]:
        return _record_value(self._inner.get_tool_transform(instance_id, tool_name))

    def list_tool_transforms(self) -> List[Dict[str, Any]]:
        return _record_value(self._inner.list_tool_transforms())

    def delete_tool_transform(self, instance_id: str, tool_name: str) -> None:
        self._inner.delete_tool_transform(instance_id, tool_name)

    # Sessions retain their own explicit instance bindings.
    def create_session(
        self,
        session_id: str,
        *,
        scope: Optional[str] = None,
        agent_id: Optional[str] = None,
        lease_seconds: Optional[int] = None,
        metadata: Optional[Dict[str, Any]] = None,
    ) -> "RustSession":
        entity = self._inner.create_session(
            session_id,
            scope,
            agent_id,
            lease_seconds,
            metadata,
        )
        return RustSession(self, _record_value(entity))

    def get_session(self, session_key: str) -> Optional["RustSession"]:
        entity = self._inner.get_session(session_key)
        return RustSession(self, _record_value(entity)) if entity else None

    def find_session(
        self,
        session_id: str,
        *,
        scope: Optional[str] = None,
        agent_id: Optional[str] = None,
    ) -> Optional["RustSession"]:
        entity = self._inner.find_session(session_id, scope, agent_id)
        return RustSession(self, _record_value(entity)) if entity else None

    def list_sessions(
        self,
        *,
        scope: Optional[str] = None,
        agent_id: Optional[str] = None,
    ) -> List["RustSession"]:
        return [
            RustSession(self, _record_value(entity))
            for entity in self._inner.list_sessions(scope, agent_id)
        ]

    def export_sessions_snapshot(self) -> Dict[str, Any]:
        return _record_value(self._inner.export_sessions_snapshot())

    def import_sessions_snapshot(self, snapshot: Dict[str, Any]) -> Dict[str, Any]:
        return _record_value(self._inner.import_sessions_snapshot(snapshot))

    # Cache and events
    def event_history(self, count: int = 100) -> List[Dict[str, Any]]:
        return _record_value(self._inner.event_history(count))

    def event_capability_report(self) -> Dict[str, Any]:
        return _record_value(self._inner.event_capability_report())

    def cache_health_check(self) -> Dict[str, Any]:
        return _record_value(self._inner.cache_health_check())

    def cache_inspect(self) -> Dict[str, Any]:
        return _record_value(self._inner.cache_inspect())

    def reset_cache_request_metrics(self) -> None:
        self._inner.reset_cache_request_metrics()

    def find_cache(self) -> "RustCacheProxy":
        return RustCacheProxy(self)

    def switch_cache(
        self,
        cache_config: Any,
    ) -> Dict[str, Any]:
        normalized = self._normalize_cache_config(cache_config)
        backend, redis_url, namespace = self._cache_options(normalized)
        if backend is None:
            raise ValueError("cache_config is required")
        result = self._inner.switch_cache_storage(backend, redis_url, namespace)
        self._cache_config = normalized
        return _record_value(result)

    # Scoped views are explicit constructors, not runtime resolution paths.
    def for_store(self) -> "RustStoreContext":
        return RustStoreContext(self, {"type": "store"})

    def for_agent(self, agent_id: str) -> "RustStoreContext":
        return RustStoreContext(self, {"type": "agent", "agent_id": agent_id})

    def for_langchain(self, instance_id: str, response_format: str = "text") -> Any:
        from mcpstore.adapters.langchain_adapter import LangChainAdapter

        return LangChainAdapter(self, instance_id, response_format=response_format)

    def for_langgraph(self, instance_id: str, response_format: str = "text") -> Any:
        from mcpstore.adapters.langgraph_adapter import LangGraphAdapter

        return LangGraphAdapter(self, instance_id, response_format=response_format)

    def for_openai(self, instance_id: str) -> Any:
        from mcpstore.adapters.openai_adapter import OpenAIAdapter

        return OpenAIAdapter(self, instance_id)

    def for_autogen(self, instance_id: str) -> Any:
        from mcpstore.adapters.autogen_adapter import AutoGenAdapter

        return AutoGenAdapter(self, instance_id)

    def for_llamaindex(self, instance_id: str) -> Any:
        from mcpstore.adapters.llamaindex_adapter import LlamaIndexAdapter

        return LlamaIndexAdapter(self, instance_id)

    def for_crewai(self, instance_id: str) -> Any:
        from mcpstore.adapters.crewai_adapter import CrewAIAdapter

        return CrewAIAdapter(self, instance_id)

    def for_semantic_kernel(self, instance_id: str) -> Any:
        from mcpstore.adapters.semantic_kernel_adapter import SemanticKernelAdapter

        return SemanticKernelAdapter(self, instance_id)

    def __repr__(self) -> str:
        return repr(self._inner)


class MCPStore(RustStoreBackend):
    """Public Rust-backed MCPStore entry point."""


class RustStoreContext:
    """View bound to one explicit scope for scoped discovery."""

    def __init__(self, backend: RustStoreBackend, scope: ScopeRef | Dict[str, Any]):
        self._backend = backend
        self.scope = _scope_payload(scope)

    def list_services(self) -> List[Dict[str, Any]]:
        return self._backend.list_instances_scoped(self.scope)

    async def list_services_async(self) -> List[Dict[str, Any]]:
        return self.list_services()

    def show_config(self) -> Dict[str, Any]:
        return self._backend.show_scope_config(self.scope)

    def get_effective_config(self, service_name: str) -> Optional[Dict[str, Any]]:
        return self._backend.get_effective_config(service_name, self.scope)

    def declare_service_scope(
        self,
        service_name: str,
        scope: ScopeRef | Dict[str, Any],
        descriptor: ScopeDescriptor | Dict[str, Any],
    ) -> str:
        return self._backend.declare_service_scope(service_name, scope, descriptor)

    def remove_service_scope(
        self,
        service_name: str,
        scope: ScopeRef | Dict[str, Any],
    ) -> None:
        self._backend.remove_service_scope(service_name, scope)

    def list_instances(self) -> List[Dict[str, Any]]:
        return self._backend.list_instances()

    def find_service(self, instance_id: str) -> "RustServiceProxy":
        if self._backend.find_instance(instance_id) is None:
            raise KeyError(f"Instance not found: {instance_id}")
        return RustServiceProxy(self._backend, instance_id)

    def find_tool(self, instance_id: str, tool_name: str) -> "RustToolProxy":
        return RustToolProxy(self._backend, instance_id, tool_name)

    def connect_service(self, instance_id: str) -> None:
        self._backend.connect_service(instance_id)

    async def connect_service_async(self, instance_id: str) -> None:
        self.connect_service(instance_id)

    def disconnect_service(self, instance_id: str) -> None:
        self._backend.disconnect_service(instance_id)

    async def disconnect_service_async(self, instance_id: str) -> None:
        self.disconnect_service(instance_id)

    def restart_service(self, instance_id: str) -> None:
        self._backend.restart_service(instance_id)

    async def restart_service_async(self, instance_id: str) -> None:
        self.restart_service(instance_id)

    def list_tools(self, instance_id: str) -> List[Dict[str, Any]]:
        return self._backend.list_tools(instance_id)

    async def list_tools_async(self, instance_id: str) -> List[Dict[str, Any]]:
        return self.list_tools(instance_id)

    def call_tool(
        self,
        instance_id: str,
        tool_name: str,
        args: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        return self._backend.call_tool(instance_id, tool_name, args)

    async def call_tool_async(
        self,
        instance_id: str,
        tool_name: str,
        args: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        return self.call_tool(instance_id, tool_name, args)

    def list_resources(self, instance_id: str) -> List[Dict[str, Any]]:
        return self._backend.list_resources(instance_id)

    def read_resource(self, instance_id: str, uri: str) -> Dict[str, Any]:
        return self._backend.read_resource(instance_id, uri)

    def list_prompts(self, instance_id: str) -> List[Dict[str, Any]]:
        return self._backend.list_prompts(instance_id)

    def get_prompt(
        self,
        instance_id: str,
        prompt_name: str,
        arguments: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        return self._backend.get_prompt(instance_id, prompt_name, arguments)

    def export_instance_config(
        self,
        instance_id: str,
        format: Optional[str] = None,
    ) -> Dict[str, Any]:
        return self._backend.export_instance_config(instance_id, format)

    def wait_instance_ready(self, instance_id: str, timeout_secs: int = 10) -> Dict[str, Any]:
        return self._backend.wait_instance_ready(instance_id, timeout_secs)

    def find_cache(self) -> "RustCacheProxy":
        return self._backend.find_cache()

    def for_langchain(self, instance_id: str, response_format: str = "text") -> Any:
        return self._backend.for_langchain(instance_id, response_format)

    def for_langgraph(self, instance_id: str, response_format: str = "text") -> Any:
        return self._backend.for_langgraph(instance_id, response_format)

    def for_openai(self, instance_id: str) -> Any:
        return self._backend.for_openai(instance_id)

    def for_autogen(self, instance_id: str) -> Any:
        return self._backend.for_autogen(instance_id)

    def for_llamaindex(self, instance_id: str) -> Any:
        return self._backend.for_llamaindex(instance_id)

    def for_crewai(self, instance_id: str) -> Any:
        return self._backend.for_crewai(instance_id)

    def for_semantic_kernel(self, instance_id: str) -> Any:
        return self._backend.for_semantic_kernel(instance_id)


class RustServiceProxy:
    """Proxy for one concrete service instance."""

    def __init__(self, backend: RustStoreBackend, instance_id: str):
        self._backend = backend
        self.instance_id = instance_id

    def info(self) -> Dict[str, Any]:
        record = self._backend.find_instance(self.instance_id)
        if record is None:
            raise KeyError(f"Instance not found: {self.instance_id}")
        return record

    @property
    def service_name(self) -> str:
        return str(self.info()["service_name"])

    @property
    def scope(self) -> Dict[str, Any]:
        return _record_value(self.info()["scope"])

    def connect(self) -> "RustServiceProxy":
        self._backend.connect_service(self.instance_id)
        return self

    def disconnect(self) -> "RustServiceProxy":
        self._backend.disconnect_service(self.instance_id)
        return self

    def restart(self) -> "RustServiceProxy":
        self._backend.restart_service(self.instance_id)
        return self

    def wait_ready(self, timeout_secs: int = 10) -> Dict[str, Any]:
        return self._backend.wait_instance_ready(self.instance_id, timeout_secs)

    def list_tools(self) -> List[Dict[str, Any]]:
        return self._backend.list_tools(self.instance_id)

    def find_tool(self, tool_name: str) -> "RustToolProxy":
        return RustToolProxy(self._backend, self.instance_id, tool_name)

    def call_tool(
        self,
        tool_name: str,
        args: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        return self._backend.call_tool(self.instance_id, tool_name, args)

    def export_config(self, format: Optional[str] = None) -> Dict[str, Any]:
        return self._backend.export_instance_config(self.instance_id, format)


class RustToolProxy:
    """Proxy for one tool owned by one concrete instance."""

    def __init__(self, backend: RustStoreBackend, instance_id: str, tool_name: str):
        self._backend = backend
        self.instance_id = instance_id
        self.tool_name = tool_name

    def call(self, args: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        return self._backend.call_tool(self.instance_id, self.tool_name, args)

    async def call_async(self, args: Optional[Dict[str, Any]] = None) -> Dict[str, Any]:
        return self.call(args)


class RustCacheProxy:
    def __init__(self, backend: RustStoreBackend):
        self._backend = backend

    def health_check(self) -> Dict[str, Any]:
        return self._backend.cache_health_check()

    def inspect(self) -> Dict[str, Any]:
        return self._backend.cache_inspect()

    def reset_request_metrics(self) -> None:
        self._backend.reset_cache_request_metrics()


class RustSession:
    """Session facade whose service relationships use instance IDs."""

    def __init__(self, backend: RustStoreBackend, entity: Dict[str, Any]):
        self._backend = backend
        self._entity = entity

    @property
    def session_key(self) -> str:
        return str(self._entity["session_key"])

    @property
    def session_id(self) -> str:
        return str(self._entity["session_id"])

    def to_dict(self) -> Dict[str, Any]:
        return _plain_record_value(self._entity)

    def refresh(self) -> "RustSession":
        entity = self._backend._inner.get_session(self.session_key)
        if entity is None:
            raise KeyError(f"Session not found: {self.session_key}")
        self._entity = _record_value(entity)
        return self

    def bind_service(self, instance_id: str) -> "RustSession":
        self._backend._inner.bind_service_to_session(self.session_key, instance_id)
        return self

    def unbind_service(self, instance_id: str) -> "RustSession":
        self._backend._inner.unbind_service_from_session(self.session_key, instance_id)
        return self

    def list_services(self) -> List[Dict[str, Any]]:
        return _record_value(self._backend._inner.list_session_services(self.session_key))

    def list_tools(self) -> List[Dict[str, Any]]:
        return _record_value(self._backend._inner.list_tools_in_session(self.session_key))

    async def list_tools_async(self) -> List[Dict[str, Any]]:
        return self.list_tools()

    def call_tool(
        self,
        instance_id: str,
        tool_name: str,
        args: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        return _record_value(
            self._backend._inner.call_tool_in_session(
                self.session_key,
                instance_id,
                tool_name,
                args or {},
            )
        )

    async def call_tool_async(
        self,
        instance_id: str,
        tool_name: str,
        args: Optional[Dict[str, Any]] = None,
    ) -> Dict[str, Any]:
        return self.call_tool(instance_id, tool_name, args)

    def close(self, reason: Optional[str] = None) -> Dict[str, Any]:
        return _record_value(self._backend._inner.close_session(self.session_key, reason))

    def for_langchain(self, instance_id: str, response_format: str = "text") -> Any:
        from mcpstore.adapters.langchain_adapter import SessionAwareLangChainAdapter

        return SessionAwareLangChainAdapter(
            self._backend,
            self,
            instance_id,
            response_format=response_format,
        )


__all__ = [
    "MCPStore",
    "RustCacheProxy",
    "RustRecordView",
    "RustServiceProxy",
    "RustSession",
    "RustStoreBackend",
    "RustStoreContext",
    "RustToolProxy",
]
