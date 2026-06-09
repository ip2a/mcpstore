import tempfile
import unittest
import importlib.util
from pathlib import Path
from unittest.mock import patch


class PyO3NativeBridgeTest(unittest.TestCase):
    def test_rust_binding_uses_native_python_objects(self):
        from mcpstore._rust import MCPStore

        workdir = Path(tempfile.mkdtemp(prefix="mcpstore-pyo3-native-"))
        store = MCPStore.setup_with_options(
            str(workdir / "mcp.json"),
            "local",
            "memory",
            None,
            "smoke",
        )

        self.assertIsInstance(store.list_services(), list)
        self.assertFalse(hasattr(store, "list_services_json"))

        store.add_service(
            "demo",
            {
                "command": "python",
                "args": ["-c", "print(1)"],
                "env": {"A": "B"},
                "headers": {},
                "transport": "stdio",
            },
        )

        services = store.list_services()
        self.assertIsInstance(services, list)
        self.assertIsInstance(services[0], dict)
        self.assertEqual(services[0]["name"], "demo")
        self.assertIsInstance(store.show_config(), dict)

    def test_python_facade_uses_native_bridge(self):
        from mcpstore import MCPStore

        workdir = Path(tempfile.mkdtemp(prefix="mcpstore-python-facade-"))
        store = MCPStore.setup_store(str(workdir / "mcp.json"))
        context = store.for_store()

        context.add_service(
            {
                "name": "demo",
                "command": "python",
                "args": ["-c", "print(1)"],
                "env": {},
                "headers": {},
                "transport": "stdio",
            }
        )

        services = context.list_services()
        self.assertEqual(services[0]["name"], "demo")
        self.assertEqual(services[0]["transport"], "stdio")
        self.assertEqual(services[0].name, "demo")
        self.assertEqual(services[0].transport_type, "stdio")
        self.assertEqual(services[0]["transport_type"], "stdio")
        self.assertIsInstance(context.show_config(), dict)

    def test_python_facade_keeps_chain_adapter_api(self):
        from mcpstore import MCPStore
        import mcpstore

        workdir = Path(tempfile.mkdtemp(prefix="mcpstore-python-chain-"))
        store = MCPStore.setup_store(str(workdir / "mcp.json"))
        context = store.for_store()

        self.assertIsNotNone(context.for_openai())
        self.assertIsNotNone(context.for_autogen())
        self.assertIsNotNone(context.for_llamaindex())
        self.assertIsNotNone(context.for_crewai())
        self.assertIsNotNone(context.for_semantic_kernel())
        self.assertIsNotNone(mcpstore.LlamaIndexAdapter)
        self.assertIsNotNone(mcpstore.CrewAIAdapter)
        self.assertIsNotNone(mcpstore.SemanticKernelAdapter)

        try:
            adapter = context.for_langchain()
        except ImportError:
            adapter = None
        self.assertTrue(adapter is None or hasattr(adapter, "list_tools"))

    def test_additional_adapters_use_rust_context_shape(self):
        from mcpstore.adapters.semantic_kernel_adapter import SemanticKernelAdapter

        class FakeContext:
            def __init__(self):
                self.called = False

            def list_tools(self):
                return [
                    {
                        "name": "echo",
                        "description": "Echo input",
                        "input_schema": {
                            "type": "object",
                            "properties": {"text": {"type": "string"}},
                            "required": ["text"],
                        },
                    }
                ]

            async def list_tools_async(self):
                return self.list_tools()

            def call_tool(self, name, arguments):
                self.called = True
                return {"content": [{"type": "text", "text": arguments["text"]}], "is_error": False}

        context = FakeContext()
        functions = SemanticKernelAdapter(context).get_functions()
        self.assertEqual(functions[0](text="semantic"), "semantic")
        self.assertTrue(context.called)

    def test_rust_binding_exposes_service_management_objects(self):
        from mcpstore._rust import MCPStore

        workdir = Path(tempfile.mkdtemp(prefix="mcpstore-pyo3-service-management-"))
        store = MCPStore.setup_with_options(
            str(workdir / "mcp.json"),
            "local",
            "memory",
            None,
            "smoke",
        )
        self.assertEqual(store.list_resources_scoped(), [])
        self.assertEqual(store.list_resource_templates_scoped(), [])
        self.assertEqual(store.list_prompts_scoped(), [])
        self.assertEqual(store.list_agents(), [])
        self.assertIsInstance(store.cache_health_check(), dict)
        self.assertIsInstance(store.cache_inspect(), dict)

        store.add_service(
            "demo",
            {
                "command": "python",
                "args": ["-c", "print(1)"],
                "env": {},
                "headers": {},
                "transport": "stdio",
            },
        )

        self.assertIsInstance(store.get_service_config("demo"), dict)
        store.patch_service("demo", {"description": "patched"})
        self.assertEqual(store.get_service_config("demo")["description"], "patched")
        self.assertIsInstance(store.check_services_scoped(), dict)

    def test_python_facade_keeps_service_and_tool_proxy_api(self):
        from mcpstore.core.store.rust_backend import RustStoreBackend, RustStoreContext

        schema = {"type": "object", "properties": {"text": {"type": "string"}}}

        class FakeBackend:
            def __init__(self):
                self.removed = []
                self.restarted = []
                self.patches = []

            def list_tools_scoped(self, agent_id=None, service_name=None, *, filter="available"):
                return [
                    {
                        "name": "echo",
                        "description": "Echo input",
                        "input_schema": schema,
                        "service_name": service_name or "demo",
                    }
                ]

            def list_agents(self):
                return [{"agent_id": "agent-a", "services": ["demo"]}]

            def resolve_tool_for_agent(self, agent_id, user_input):
                return {
                    "global_service_name": "demo",
                    "canonical_tool_name": "echo",
                }

            def call_tool(self, service_name, tool_name, args):
                return {
                    "content": [{"type": "text", "text": args["text"]}],
                    "is_error": False,
                }

            def find_service(self, name):
                return {"name": name, "transport": "stdio"}

            def service_status_scoped(self, agent_id, service_name):
                return {"status": "connected"}

            def check_services_scoped(self, agent_id=None):
                return {"demo": "connected"}

            def list_resources_scoped(self, agent_id=None, service_name=None):
                return [{"uri": "memory://doc", "service_name": service_name or "demo"}]

            def list_resource_templates_scoped(self, agent_id=None, service_name=None):
                return [{"uriTemplate": "memory://{name}", "service_name": service_name or "demo"}]

            def read_resource_scoped(self, agent_id, uri, service_name=None):
                return {"uri": uri, "text": "body", "service_name": service_name or "demo"}

            def list_prompts_scoped(self, agent_id=None, service_name=None):
                return [{"name": "summarize", "service_name": service_name or "demo"}]

            def get_prompt_scoped(self, agent_id, prompt_name, arguments, service_name=None):
                return {
                    "name": prompt_name,
                    "arguments": arguments,
                    "service_name": service_name or "demo",
                }

            def cache_health_check(self):
                return {"healthy": True}

            def cache_inspect(self):
                return {"backend": "memory"}

            def patch_service(self, name, updates):
                self.patches.append((name, updates))
                return True

            def remove_service(self, name):
                self.removed.append(name)
                return True

            def restart_service(self, name):
                self.restarted.append(name)
                return True

        inner = FakeBackend()
        backend = RustStoreBackend(inner)
        context = RustStoreContext(backend)
        agent = context.find_agent("agent-a")
        self.assertEqual(agent.agent_id, "agent-a")
        self.assertEqual(context.find_cache().scope, "global")
        self.assertEqual(context.list_agents()[0].agent_id, "agent-a")
        self.assertEqual(agent.find_cache().scope, "agent")
        self.assertEqual(context.find_cache().inspect().backend, "memory")
        self.assertEqual(context.find_cache().health_check().healthy, True)

        service = context.find_service("demo")
        self.assertEqual(service.name, "demo")
        self.assertEqual(service.service_info().name, "demo")
        self.assertEqual(service.check_health()["healthy"], True)
        self.assertEqual(service.find_cache().scope, "service")

        tool = service.find_tool("echo")
        self.assertEqual(tool.name, "echo")
        self.assertEqual(tool.tool_info().inputSchema, schema)
        self.assertEqual(tool.find_cache().scope, "tool")
        self.assertEqual(tool.call_tool({"text": "ok"}, return_extracted=True), "ok")
        self.assertEqual(tool.call_tool({"text": "ok"}).text_output, "ok")
        self.assertEqual(context.use_tool("echo", {"text": "alias"}, return_extracted=True), "alias")
        scoped_tool = context.find_tool("echo", service_name="demo")
        self.assertEqual(scoped_tool.call_tool({"text": "scoped"}, return_extracted=True), "scoped")
        self.assertIs(scoped_tool.set_redirect(True), scoped_tool)
        self.assertTrue(context.get_tool_override("demo", "echo", "return_direct", False))
        self.assertEqual(context.list_resources()[0].uri, "memory://doc")
        self.assertEqual(service.list_resource_templates()[0].uriTemplate, "memory://{name}")
        self.assertEqual(service.read_resource("memory://doc").text, "body")
        self.assertEqual(context.list_prompts()[0].name, "summarize")
        self.assertEqual(service.get_prompt("summarize", {"topic": "rust"}).arguments["topic"], "rust")

        self.assertTrue(context.update_service("demo", {"description": "patched"}))
        self.assertTrue(service.restart_service())
        self.assertTrue(service.delete_service())
        self.assertEqual(inner.patches, [("demo", {"description": "patched"})])
        self.assertEqual(inner.restarted, ["demo"])
        self.assertEqual(inner.removed, ["demo"])

    def test_python_facade_keeps_session_api_shape(self):
        from mcpstore.core.store.rust_backend import RustStoreBackend, RustStoreContext

        class FakeBackend:
            def list_services_scoped(self, agent_id=None):
                return [
                    {"name": "browser", "transport": "stdio"},
                    {"name": "search", "transport": "stdio"},
                ]

            def list_tools_scoped(self, agent_id=None, service_name=None, *, filter="available"):
                if service_name == "browser":
                    return [{"name": "browser_navigate", "service_name": "browser"}]
                if service_name == "search":
                    return [{"name": "web_search", "service_name": "search"}]
                return [
                    {"name": "browser_navigate", "service_name": "browser"},
                    {"name": "web_search", "service_name": "search"},
                ]

            def find_service(self, name):
                return {"name": name, "transport": "stdio"}

            def resolve_tool_for_agent(self, agent_id, user_input):
                if user_input == "browser_navigate":
                    return {
                        "global_service_name": "browser",
                        "canonical_tool_name": "browser_navigate",
                    }
                return {"global_service_name": "search", "canonical_tool_name": "web_search"}

            def call_tool(self, service_name, tool_name, args):
                return {
                    "content": [{"type": "text", "text": f"{service_name}:{tool_name}"}],
                    "is_error": False,
                }

        backend = RustStoreBackend(FakeBackend())
        context = RustStoreContext(backend)
        session = context.create_session("browser_task")
        self.assertEqual(session.session_id, "browser_task")
        self.assertTrue(session.is_active)
        self.assertEqual(context.find_session("browser_task"), session)

        with context.with_session("browser_task") as active:
            active.bind_service("browser")
            self.assertEqual(active.service_count, 1)
            self.assertEqual(active.list_tools()[0].name, "browser_navigate")
            self.assertEqual(active.use_tool("browser_navigate", {}, return_extracted=True), "browser:browser_navigate")

            fresh_context = RustStoreContext(backend)
            self.assertEqual(fresh_context.active_session.session_id, "browser_task")
            self.assertEqual([tool.name for tool in fresh_context.list_tools()], ["browser_navigate"])

        self.assertIsNone(context.active_session)
        context.session_auto("auto_browser")
        self.assertEqual(context.active_session.session_id, "auto_browser")
        context.session_manual()
        self.assertIsNone(context.active_session)

    def test_perspective_resolver_uses_native_python_objects(self):
        from mcpstore._rust import PerspectiveResolver

        parsed = PerspectiveResolver.parse_agent_scoped("svc_byagent_agent-a")
        self.assertIsInstance(parsed, dict)
        self.assertEqual(parsed["agent_id"], "agent-a")
        self.assertEqual(parsed["local_name"], "svc")

        service = PerspectiveResolver.normalize_service_name("agent-a", "svc")
        self.assertIsInstance(service, dict)
        self.assertEqual(service["global_name"], "svc_byagent_agent-a")

        tool = PerspectiveResolver.resolve_tool(
            "agent-a",
            "svc_echo",
            [
                {
                    "name": "svc_echo",
                    "original_name": "echo",
                    "service_name": "svc",
                    "global_service_name": "svc_byagent_agent-a",
                }
            ],
        )
        self.assertIsInstance(tool, dict)
        self.assertEqual(tool["canonical_tool_name"], "echo")

        self.assertFalse(hasattr(PerspectiveResolver, "parse_agent_scoped_json"))
        self.assertFalse(hasattr(PerspectiveResolver, "normalize_service_name_json"))
        self.assertFalse(hasattr(PerspectiveResolver, "resolve_tool_json"))

    def test_sync_adapters_do_not_use_async_bridge(self):
        with self.assertRaises(ImportError):
            from mcpstore.core.bridge import get_bridge_executor

        from mcpstore.adapters.autogen_adapter import AutoGenAdapter
        from mcpstore.adapters.openai_adapter import OpenAIAdapter

        class FakeContext:
            def __init__(self):
                self.list_tools_called = False
                self.call_tool_called = False

            def list_tools(self):
                self.list_tools_called = True
                return [
                    {
                        "name": "echo",
                        "description": "Echo input",
                        "input_schema": {
                            "type": "object",
                            "properties": {"text": {"type": "string"}},
                            "required": ["text"],
                        },
                    }
                ]

            async def list_tools_async(self):
                raise AssertionError("sync adapters must not call list_tools_async")

            def call_tool(self, name, arguments):
                self.call_tool_called = True
                return {"content": [{"type": "text", "text": arguments["text"]}], "is_error": False}

            async def call_tool_async(self, name, arguments):
                raise AssertionError("sync adapters must not call call_tool_async")

        context = FakeContext()
        openai_tools = OpenAIAdapter(context).list_tools()
        self.assertTrue(context.list_tools_called)
        self.assertEqual(openai_tools[0]["function"]["name"], "echo")

        result = OpenAIAdapter(context).execute_tool_call(
            {"name": "echo", "arguments": {"text": "ok"}}
        )
        self.assertTrue(context.call_tool_called)
        self.assertEqual(result, "ok")

        autogen_tools = AutoGenAdapter(context).list_tools()
        self.assertEqual(autogen_tools[0](text="auto"), "auto")

    def test_adapters_preserve_non_text_tool_content(self):
        from mcpstore.adapters.autogen_adapter import AutoGenAdapter
        from mcpstore.adapters.common import call_tool_response_helper
        from mcpstore.adapters.openai_adapter import OpenAIAdapter

        class FakeContext:
            def list_tools(self):
                return [
                    {
                        "name": "snapshot",
                        "description": "Return image",
                        "input_schema": {"type": "object", "properties": {}},
                    }
                ]

            def call_tool(self, name, arguments):
                return {
                    "content": [
                        {
                            "type": "image",
                            "data": "base64-image",
                            "mime_type": "image/png",
                        }
                    ],
                    "is_error": False,
                }

        result = FakeContext().call_tool("snapshot", {})
        view = call_tool_response_helper(result)
        self.assertEqual(view.artifacts[0]["type"], "image")
        self.assertEqual(view.data["artifacts"][0]["mime_type"], "image/png")
        self.assertIn("base64-image", view.text)

        openai_result = OpenAIAdapter(FakeContext()).execute_tool_call(
            {"name": "snapshot", "arguments": {}}
        )
        self.assertIn("base64-image", openai_result)

        autogen_result = AutoGenAdapter(FakeContext()).list_tools()[0]()
        self.assertIn("base64-image", autogen_result)

    def test_deprecated_python_mcp_module_is_removed(self):
        self.assertIsNone(importlib.util.find_spec("mcpstore.mcp"))

    def test_python_adapters_use_rust_schema_field(self):
        from mcpstore.adapters.common import tool_input_schema

        schema = {"type": "object", "properties": {"text": {"type": "string"}}}
        self.assertEqual(tool_input_schema({"input_schema": schema}), schema)
        self.assertEqual(tool_input_schema({"inputSchema": schema}), schema)

    def test_setup_store_normalizes_legacy_cache_options_without_old_core(self):
        from mcpstore.config import CacheType, MemoryConfig, RedisConfig
        from mcpstore.core.store.setup_manager import StoreSetupManager

        cache, only_db = StoreSetupManager._normalize_cache_options(
            cache=None,
            external_db={"cache": {"type": "redis", "url": "redis://localhost:6379/0", "namespace": "team"}},
            cache_mode="shared",
            only_db=False,
        )
        self.assertIsInstance(cache, RedisConfig)
        self.assertEqual(cache.url, "redis://localhost:6379/0")
        self.assertEqual(cache.namespace, "team")
        self.assertTrue(only_db)

        cache, only_db = StoreSetupManager._normalize_cache_options(
            cache=None,
            external_db={"cache": {"type": "memory", "max_size": 12}},
            cache_mode="local",
            only_db=True,
        )
        self.assertIsInstance(cache, MemoryConfig)
        self.assertEqual(cache.max_size, 12)
        self.assertFalse(only_db)

        cache, only_db = StoreSetupManager._normalize_cache_options(
            cache=None,
            external_db={"cache": {"type": "openkeyv_redis", "url": "redis://localhost:6379/0"}},
            cache_mode="auto",
            only_db=False,
        )
        self.assertEqual(cache.cache_type, CacheType.OPENKEYV_REDIS)
        self.assertFalse(only_db)

    def test_setup_store_accepts_mcp_config_file_alias(self):
        from mcpstore.core.store.setup_manager import StoreSetupManager

        with patch.object(StoreSetupManager, "_setup_rust_store", return_value="store") as setup:
            result = StoreSetupManager.setup_store(
                mcp_config_file="alias.json",
                external_db={"cache": {"type": "memory"}},
                cache_mode="hybrid",
            )

        self.assertEqual(result, "store")
        kwargs = setup.call_args.kwargs
        self.assertEqual(kwargs["mcpjson_path"], "alias.json")
        self.assertFalse(kwargs["only_db"])
        self.assertIsNotNone(kwargs["cache"])

    def test_start_api_server_delegates_to_rust_cli(self):
        from mcpstore.config import MemoryConfig
        from mcpstore.core.store.rust_backend import RustStoreBackend

        store = RustStoreBackend(object())
        store._config_path = "mcp.json"
        store._cache_config = MemoryConfig()
        store._only_db = True

        completed = type("Completed", (), {"returncode": 0})()
        with patch("mcpstore._rust_cli.resolve_rust_cli_binary", return_value="/bin/mcpstore"):
            with patch("mcpstore._rust_cli.resolve_runtime_cwd", return_value="/tmp"):
                with patch("mcpstore.core.store.rust_backend.subprocess.run", return_value=completed) as run:
                    code = store.start_api_server(
                        host="0.0.0.0",
                        port=18200,
                        url_prefix="/mcp",
                        show_startup_info=False,
                    )

        self.assertEqual(code, 0)
        cmd = run.call_args.args[0]
        self.assertEqual(cmd[:6], ["/bin/mcpstore", "api", "--host", "0.0.0.0", "--port", "18200"])
        self.assertIn("--url-prefix", cmd)
        self.assertIn("--config-path", cmd)
        self.assertIn("--source", cmd)
        self.assertIn("--backend", cmd)

    def test_hub_http_delegates_to_rust_mcp_server_cli(self):
        from mcpstore.config import MemoryConfig
        from mcpstore.core.store.rust_backend import RustStoreBackend

        store = RustStoreBackend(object())
        store._config_path = "mcp.json"
        store._cache_config = MemoryConfig()
        store._only_db = True

        completed = type("Completed", (), {"returncode": 0})()
        with patch("mcpstore._rust_cli.resolve_rust_cli_binary", return_value="/bin/mcpstore"):
            with patch("mcpstore._rust_cli.resolve_runtime_cwd", return_value="/tmp"):
                with patch("mcpstore.core.store.rust_backend.subprocess.run", return_value=completed) as run:
                    code = store.for_agent("agent-a").hub_http(
                        host="0.0.0.0",
                        port=18080,
                        path="/mcp",
                        block=True,
                    )

        self.assertEqual(code, 0)
        cmd = run.call_args.args[0]
        self.assertEqual(cmd[:4], ["/bin/mcpstore", "mcp-server", "--transport", "streamable-http"])
        self.assertIn("--scope", cmd)
        self.assertIn("agent", cmd)
        self.assertIn("--agent", cmd)
        self.assertIn("agent-a", cmd)
        self.assertIn("--config-path", cmd)
        self.assertIn("--source", cmd)

    def test_rust_backed_public_api_modules_import(self):
        from mcpstore.api.api_dependencies import get_store
        from mcpstore.config import MemoryConfig
        from mcpstore.config.factory import create_kv_store
        from mcpstore.config.namespace import get_namespace
        from mcpstore.core.models import ErrorCode, ResponseBuilder, timed_response

        store = object()
        if importlib.util.find_spec("fastapi") is not None:
            from mcpstore.api.api_pack import api_agent_router, api_main_router, api_set_store

            self.assertIsNotNone(api_main_router)
            self.assertIsNotNone(api_agent_router)
        else:
            from mcpstore.api.api_dependencies import set_store as api_set_store

        api_set_store(store)
        self.assertIs(get_store(), store)
        self.assertEqual(get_namespace(MemoryConfig()), "mcpstore")
        self.assertEqual(create_kv_store(MemoryConfig()).get_backend_type(), "memory")
        self.assertEqual(ErrorCode.MISSING_PARAMETER.value, "missing_parameter")
        self.assertTrue(ResponseBuilder.success()["success"])
        self.assertFalse(ResponseBuilder.error()["success"])

        @timed_response
        def handler():
            return ResponseBuilder.success()

        self.assertIn("elapsed_ms", handler()["meta"])

    def test_rust_context_keeps_bridge_execute_and_cache_read_shape(self):
        import asyncio

        from mcpstore.core.store.rust_backend import RustCacheProxy, RustStoreContext

        class FakeBackend:
            def cache_inspect(self):
                return {
                    "backend": "memory",
                    "entities": [
                        {"_type": "service", "_key": "demo", "name": "demo"},
                        {"_type": "tool", "_key": "echo", "name": "echo"},
                    ],
                    "relations": [{"_type": "binding", "_key": "demo:echo"}],
                    "states": [{"_type": "health", "_key": "demo"}],
                }

        context = RustStoreContext(FakeBackend())
        self.assertEqual(asyncio.run(context.bridge_execute("value")), "value")

        async def compute():
            return "async-value"

        self.assertEqual(asyncio.run(context.bridge_execute(compute())), "async-value")

        cache = RustCacheProxy(context)
        self.assertEqual(cache.get_scope(), "global")
        self.assertEqual(cache.get_backend_type(), "memory")
        self.assertEqual(len(cache.read_entity(None, None)), 2)
        self.assertEqual(cache.read_entity("service", "demo")[0]["name"], "demo")
        self.assertEqual(cache.read_relation("binding", None)[0]["_key"], "demo:echo")
        self.assertEqual(cache.read_state("health", "demo")[0]["_type"], "health")


if __name__ == "__main__":
    unittest.main()
