"""Public Python store facade; computation stays in Rust or domain modules."""

from __future__ import annotations

from typing import Any, Dict, Optional

from mcpstore.context.store_context import AgentContext, StoreContext
from mcpstore.cache import store as cache_ops
from mcpstore.store import configuration, instances, setup as setup_module, transforms
from mcpstore.sessions import store as session_ops


class RustStoreBackend:
    """Thin Python facade over ``mcpstore._rust.MCPStore``."""

    def __init__(self, rust_store: Any):
        self._inner = rust_store
        self._config_path: Optional[str] = None
        self._cache_config: Any = None
        self._only_db = False

    @classmethod
    def setup(cls, config_path: Optional[str] = None, cache_config: Any = None, only_db: bool = False):
        return setup_module.setup_backend(cls, config_path, cache_config, only_db)

    @staticmethod
    def setup_store(mcpjson_path: str | None = None, debug: bool | str = False,
                    cache: Any = None, static_config: Optional[Dict[str, Any]] = None,
                    cache_mode: str = "auto", only_db: bool = False, **kwargs: Any):
        from mcpstore.store.setup import StoreSetupManager
        return StoreSetupManager.setup_store(mcpjson_path, debug, cache, static_config, cache_mode, only_db, **kwargs)

    @staticmethod
    async def setup_store_async(mcpjson_path: str | None = None, debug: bool | str = False,
                                cache: Any = None, static_config: Optional[Dict[str, Any]] = None,
                                cache_mode: str = "auto", only_db: bool = False, **kwargs: Any):
        from mcpstore.store.setup import StoreSetupManager
        return await StoreSetupManager.setup_store_async(mcpjson_path, debug, cache, static_config, cache_mode, only_db, **kwargs)

    _normalize_cache_config = staticmethod(setup_module.normalize_cache_config)
    _redis_url = staticmethod(setup_module.redis_url)
    _cache_options = staticmethod(setup_module.cache_options)

    namespace = configuration.namespace
    current_backend = configuration.current_backend
    load_from_config = configuration.load_from_config
    add_service = configuration.add_service
    add_service_async = configuration.add_service_async
    declare_service_scope = configuration.declare_service_scope
    declare_service_scope_async = configuration.declare_service_scope_async
    remove_service_scope = configuration.remove_service_scope
    remove_service_scope_async = configuration.remove_service_scope_async
    patch_service = configuration.patch_service
    patch_service_async = configuration.patch_service_async
    update_service = configuration.update_service
    update_service_async = configuration.update_service_async
    remove_service = configuration.remove_service
    remove_service_async = configuration.remove_service_async
    get_definition_config = configuration.get_definition_config
    get_effective_config = configuration.get_effective_config
    show_config = configuration.show_config
    reset_config = configuration.reset_config

    list_instances = instances.list_instances
    list_instances_async = instances.list_instances_async
    list_instances_scoped = instances.list_instances_scoped
    find_instance = instances.find_instance
    instance_info = instances.instance_info
    connect_service = instances.connect_service
    connect_service_async = instances.connect_service_async
    disconnect_service = instances.disconnect_service
    disconnect_service_async = instances.disconnect_service_async
    restart_service = instances.restart_service
    restart_service_async = instances.restart_service_async
    wait_instance_ready = instances.wait_instance_ready
    wait_instance_ready_async = instances.wait_instance_ready_async
    check_instances = instances.check_instances
    service_state = instances.service_state
    list_tools = instances.list_tools
    list_tools_async = instances.list_tools_async
    list_tool_entries = instances.list_tool_entries
    list_changed_tools = instances.list_changed_tools
    call_tool = instances.call_tool
    call_tool_async = instances.call_tool_async
    list_resources = instances.list_resources
    list_resources_async = instances.list_resources_async
    list_resource_templates = instances.list_resource_templates
    read_resource = instances.read_resource
    read_resource_async = instances.read_resource_async
    list_prompts = instances.list_prompts
    list_prompts_async = instances.list_prompts_async
    get_prompt = instances.get_prompt
    export_instance_config = instances.export_instance_config

    set_tool_transform = transforms.set_tool_transform
    create_llm_friendly_tool_transform = transforms.create_llm_friendly_tool_transform
    create_parameter_renamed_tool_transform = transforms.create_parameter_renamed_tool_transform
    create_validated_tool_transform = transforms.create_validated_tool_transform
    get_tool_transform = transforms.get_tool_transform
    list_tool_transforms = transforms.list_tool_transforms
    delete_tool_transform = transforms.delete_tool_transform

    create_session = session_ops.create_session
    get_session = session_ops.get_session
    find_session = session_ops.find_session
    list_sessions = session_ops.list_sessions
    export_sessions_snapshot = session_ops.export_sessions_snapshot
    import_sessions_snapshot = session_ops.import_sessions_snapshot

    event_history = cache_ops.event_history
    event_capability_report = cache_ops.event_capability_report
    cache_health_check = cache_ops.cache_health_check
    cache_inspect = cache_ops.cache_inspect
    reset_cache_request_metrics = cache_ops.reset_cache_request_metrics
    find_cache = cache_ops.find_cache
    switch_cache = cache_ops.switch_cache

    def for_store(self) -> StoreContext:
        return StoreContext(self._inner.for_store())

    def for_agent(self, agent_id: str) -> AgentContext:
        return AgentContext(self._inner.for_agent(agent_id))

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


__all__ = ["MCPStore", "RustStoreBackend"]
