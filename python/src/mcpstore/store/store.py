"""Public Python store facade; computation stays in Rust or domain modules."""

from __future__ import annotations

from typing import Any, Dict, Optional

from mcpstore.context.store_context import AgentContext, StoreContext
from mcpstore.cache import store as cache_ops
from mcpstore.store import configuration, instances, setup as setup_module
from mcpstore.sessions import store as session_ops


class RustStoreBackend:
    """Thin Python facade over ``mcpstore._rust.MCPStore``."""

    def __init__(self, rust_store: Any):
        self._inner = rust_store
        self._source_mode: Optional[str] = None   # "local" | "db"
        self._node_mode: Optional[str] = None     # "control_plane" | "data_plane"

    # ------------------------------------------------------------------
    # Setup entry point
    # ------------------------------------------------------------------
    @classmethod
    def setup(cls, source: Any, source_mode: str, node_mode: str):
        """Construct the Rust-backed store from resolved source + modes."""
        return setup_module.setup_backend(cls, source, source_mode, node_mode)

    @staticmethod
    def setup_store(source: Any, mode: Optional[str] = None, *,
                    debug: bool | str = False,
                    static_config: Optional[Dict[str, Any]] = None, **kwargs: Any):
        """Public entry point. Delegates to StoreSetupManager."""
        from mcpstore.store.setup import StoreSetupManager
        return StoreSetupManager.setup_store(
            source=source, mode=mode, debug=debug,
            static_config=static_config, **kwargs,
        )

    # ------------------------------------------------------------------
    # Metadata
    # ------------------------------------------------------------------
    @property
    def source_mode(self) -> Optional[str]:
        return self._source_mode

    @property
    def node_mode(self) -> Optional[str]:
        return self._node_mode

    namespace = configuration.namespace
    current_store = configuration.current_store
    load_from_config = configuration.load_from_config
    add_service = configuration.add_service
    declare_service_scope = configuration.declare_service_scope
    remove_service_scope = configuration.remove_service_scope
    patch_service = configuration.patch_service
    update_service = configuration.update_service
    remove_service = configuration.remove_service
    get_definition_config = configuration.get_definition_config
    get_effective_config = configuration.get_effective_config
    show_config = configuration.show_config
    reset_config = configuration.reset_config

    list_instances = instances.list_instances
    list_agents = instances.list_agents
    list_instances_scoped = instances.list_instances_scoped
    find_instance = instances.find_instance
    instance_info = instances.instance_info
    connect_service = instances.connect_service
    disconnect_service = instances.disconnect_service
    restart_service = instances.restart_service
    wait_instance_ready = instances.wait_instance_ready
    check_instances = instances.check_instances
    service_state = instances.service_state
    list_tools = instances.list_tools
    list_tool_entries = instances.list_tool_entries
    list_changed_tools = instances.list_changed_tools
    call_tool = instances.call_tool
    list_resources = instances.list_resources
    list_resource_templates = instances.list_resource_templates
    read_resource = instances.read_resource
    list_prompts = instances.list_prompts
    get_prompt = instances.get_prompt
    export_instance_config = instances.export_instance_config

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
    swap_store = cache_ops.swap_store

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
