import tempfile
import unittest
import importlib.util
import json
import tomllib
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
        self.assertIn("mcpServers", store.show_mcpjson())
        self.assertIn("mcpServers", context.show_mcpjson())
        self.assertIsInstance(store.get_json_config(), dict)
        self.assertEqual(store.get_data_space_info()["backend"], "memory")

    def test_top_level_store_methods_are_sync_and_awaitable(self):
        import asyncio

        from mcpstore import MCPStore

        workdir = Path(tempfile.mkdtemp(prefix="mcpstore-python-awaitable-"))
        store = MCPStore.setup_store(str(workdir / "mcp.json"))

        result = store.add_service(
            {"name": "demo", "command": "python", "args": ["-c", "print(1)"], "transport": "stdio"}
        )
        self.assertTrue(result)
        self.assertIsInstance(store.list_services(), list)

        async def run():
            ok = await store.add_service(
                {"name": "demo2", "command": "python", "args": ["-c", "print(2)"], "transport": "stdio"}
            )
            services = await store.list_services()
            return ok, services

        ok, services = asyncio.run(run())
        self.assertTrue(ok)
        self.assertEqual({service.name for service in services}, {"demo", "demo2"})

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

    def test_python_facade_keeps_export_import_cleanup_api(self):
        import asyncio

        from mcpstore import MCPStore

        workdir = Path(tempfile.mkdtemp(prefix="mcpstore-python-export-"))
        store = MCPStore.setup_store(str(workdir / "mcp.json"))
        store.for_store().add_service(
            {"name": "demo", "command": "python", "args": ["-c", "print(1)"], "transport": "stdio"}
        )

        exported = asyncio.run(store.exportjson())
        self.assertIn("mcpServers", exported)
        self.assertIn("demo", exported["mcpServers"])

        backup = workdir / "backup.json"
        written = asyncio.run(store.export_to_json(str(backup)))
        self.assertEqual(written, exported)
        self.assertEqual(json.loads(backup.read_text(encoding="utf-8")), exported)

        output_backup = workdir / "output-backup.json"
        written = asyncio.run(store.export_to_json(output_path=output_backup, include_sessions=True))
        self.assertEqual(written, exported)
        self.assertEqual(json.loads(output_backup.read_text(encoding="utf-8")), exported)

        filepath_backup = workdir / "filepath-backup.json"
        written = asyncio.run(store.exportjson(filepath=filepath_backup))
        self.assertEqual(written, exported)
        self.assertEqual(json.loads(filepath_backup.read_text(encoding="utf-8")), exported)

        with self.assertRaises(ValueError):
            asyncio.run(store.exportjson(filepath=backup, output_path=output_backup))

        asyncio.run(store.cleanup())
        self.assertEqual(store.for_store().list_services(), [])
        self.assertTrue(asyncio.run(store.import_from_json(str(backup))))
        self.assertEqual(store.for_store().list_services()[0].name, "demo")

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
        self.assertIsInstance(store.event_history(10), list)
        self.assertIsInstance(store.event_capability_report(), dict)
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
        import asyncio

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

            def list_services_scoped(self, agent_id=None):
                return [{"name": "demo", "transport": "stdio"}]

            def list_agents(self):
                return [{"agent_id": "agent-a", "services": ["demo"]}]

            def namespace(self):
                return "test"

            def current_backend(self):
                return "memory"

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

            def connect_service(self, name):
                self.connected = name
                return True

            def disconnect_service(self, name):
                self.disconnected = name
                return True

            def event_history(self, count=100):
                return [{"event_type": "TEST", "count": count}]

            def event_capability_report(self):
                return {"event_bus": True}

            def wait_service_ready(self, name, timeout=10.0):
                return {"service_global_name": name, "health_status": "ready"}

            def show_config(self):
                return {
                    "mcpServers": {
                        "demo": {"transport": "stdio"},
                        "agent-demo": {"transport": "stdio"},
                        "other-agent-demo": {"transport": "stdio"},
                    },
                    "agents": {
                        "agent-a": ["agent-demo"],
                        "agent-b": ["other-agent-demo"],
                    },
                    "clients": {"client-a": {"service": "demo"}},
                }

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
        self.assertEqual(context.get_info().context_type, "store")
        self.assertEqual(agent.get_info().agent_id, "agent-a")
        with patch.object(backend, "start_mcp_server", return_value=0) as start_mcp_server:
            self.assertEqual(context.hub_http(), 0)
        start_mcp_server.assert_called_once_with(
            agent_id=None,
            transport="streamable-http",
            host="0.0.0.0",
            port=8000,
            path="/mcp",
            block=False,
        )
        with patch.object(backend, "start_mcp_server", return_value=0) as start_mcp_server:
            self.assertEqual(context.hub_sse(), 0)
        start_mcp_server.assert_called_once_with(
            agent_id=None,
            transport="streamable-http",
            host="0.0.0.0",
            port=8000,
            path="/mcp",
            block=False,
        )
        with patch.object(backend, "start_mcp_server", return_value=0) as start_mcp_server:
            self.assertEqual(context.hub_stdio(), 0)
        start_mcp_server.assert_called_once_with(
            agent_id=None,
            transport="stdio",
            block=False,
        )

        service = context.find_service("demo")
        self.assertEqual(service.name, "demo")
        self.assertEqual(service.context_type, "store")
        self.assertEqual(service.tools_count, 1)
        self.assertTrue(service.is_connected)
        self.assertIn("demo", repr(service))
        self.assertEqual(service.service_info().name, "demo")
        self.assertEqual(service.check_health()["healthy"], True)
        self.assertEqual(asyncio.run(service.check_health_async()).healthy, True)
        self.assertEqual(service.find_cache().scope, "service")
        self.assertEqual(service.tools_stats().tool_count, 1)
        self.assertEqual(service.tools_stats().metadata.total_tools, 1)
        self.assertEqual(service.tools_stats().metadata.tools_by_service.demo, 1)
        self.assertFalse(service.tools_stats().history_available)

        tool = service.find_tool("echo")
        self.assertEqual(tool.name, "echo")
        self.assertEqual(tool.context_type, "store")
        self.assertEqual(tool.scope, "store")
        self.assertEqual(tool.description, "Echo input")
        self.assertTrue(tool.has_schema)
        self.assertTrue(tool.is_available)
        self.assertIn("echo", repr(tool))
        self.assertEqual(tool.tool_info().inputSchema, schema)
        self.assertEqual(asyncio.run(tool.tool_info_async()).inputSchema, schema)
        self.assertEqual(tool.find_cache().scope, "tool")
        self.assertFalse(tool.usage_stats().history_available)
        self.assertEqual(tool.call_history(limit=5), [])
        self.assertEqual(tool.call_tool({"text": "ok"}, return_extracted=True), "ok")
        self.assertEqual(tool.call_tool({"text": "ok"}).text_output, "ok")
        self.assertEqual(tool.test_call({"text": "test"}).text_output, "test")
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
        self.assertTrue(context.connect_service("demo"))
        self.assertTrue(service.disconnect_service())
        self.assertTrue(service.refresh_content())
        self.assertEqual(context.event_history(1)[0].event_type, "TEST")
        self.assertTrue(context.event_capability_report().event_bus)
        self.assertEqual(context.show_config("client").clients["client-a"].service, "demo")
        self.assertEqual(backend.show_config("clients").clients["client-a"].service, "demo")
        self.assertEqual(set(context.show_config("mcp").mcpServers), {"demo", "agent-demo", "other-agent-demo"})
        self.assertEqual(set(agent.show_config("all").mcpServers), {"agent-demo"})
        self.assertEqual(agent.show_config("agent").agents["agent-a"], ["agent-demo"])
        self.assertEqual(context.wait_service("demo", status="healthy").health_status, "ready")
        self.assertEqual(context.wait_service("demo", status=["healthy", "warning"]).health_status, "ready")
        self.assertEqual(context.wait_services(["demo"], status="healthy")["demo"].health_status, "ready")
        with self.assertRaises(TimeoutError):
            context.wait_service("demo", status="degraded")
        self.assertTrue(service.restart_service())
        self.assertTrue(service.delete_service())
        self.assertEqual(inner.patches, [("demo", {"description": "patched"})])
        self.assertEqual(inner.restarted, ["demo", "demo"])
        self.assertEqual(inner.removed, ["demo"])

    def test_add_service_accepts_legacy_config_shapes_through_rust(self):
        from mcpstore.core.store.rust_backend import RustStoreBackend

        class FakeInner:
            def __init__(self):
                self.added = []
                self.agent_added = []

            def add_service(self, name, config):
                self.added.append((name, config))

            def add_service_for_agent(self, agent_id, local_name, config):
                self.agent_added.append((agent_id, local_name, config))
                return f"{local_name}_byagent_{agent_id}"

        inner = FakeInner()
        backend = RustStoreBackend(inner)
        path = Path(tempfile.mkdtemp(prefix="mcpstore-service-json-")) / "mcp.json"
        path.write_text(
            json.dumps({"mcpServers": {"demo": {"url": "https://example.test/mcp"}}}),
            encoding="utf-8",
        )

        backend.add_service(json_file=str(path), headers={"X-Test": "1"})
        self.assertEqual(inner.added[0][0], "demo")
        self.assertEqual(inner.added[0][1]["headers"]["X-Test"], "1")

        backend.for_store().add_service(
            {
                "wide-a": {"url": "https://a.example.test/mcp"},
                "wide-b": {"command": "python", "args": ["-m", "server"]},
            },
            headers={"Authorization": "Bearer test"},
        )
        self.assertEqual(inner.added[1][0], "wide-a")
        self.assertEqual(inner.added[1][1]["headers"]["Authorization"], "Bearer test")
        self.assertEqual(inner.added[2][0], "wide-b")

        context = backend.for_store()
        self.assertIs(
            context.add_service({"name": "chainable", "url": "https://chain.example.test/mcp"}),
            context,
        )

        backend.for_agent("agent-a").add_service(
            json.dumps({"name": "local", "command": "python"}),
        )
        self.assertEqual(inner.agent_added[0][0], "agent-a")
        self.assertEqual(inner.agent_added[0][1], "local")

        agent = backend.for_agent("agent-b")
        self.assertIs(
            agent.add_service({"wide-agent": {"url": "https://agent.example.test/mcp"}}),
            agent,
        )
        self.assertEqual(inner.agent_added[1][0], "agent-b")
        self.assertEqual(inner.agent_added[1][1], "wide-agent")

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
            self.assertIs(context.current_session(), active)
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

    def test_python_facade_keeps_agent_tool_set_management_shape(self):
        from mcpstore.core.store.rust_backend import RustStoreBackend, RustStoreContext

        class FakeBackend:
            def list_services_scoped(self, agent_id=None):
                return [{"name": "svc", "transport": "stdio"}]

            def list_tools_scoped(self, agent_id=None, service_name=None, *, filter="available"):
                service = service_name or "svc"
                return [
                    {"name": "alpha", "original_name": "alpha", "service_name": service},
                    {"name": "beta", "original_name": "beta", "service_name": service},
                ]

        backend = RustStoreBackend(FakeBackend())
        agent = RustStoreContext(backend, agent_id="agent-a")

        self.assertEqual([tool.name for tool in agent.list_tools(filter="all")], ["alpha", "beta"])
        self.assertEqual([tool.name for tool in agent.list_tools(filter="available")], ["alpha", "beta"])

        self.assertIs(agent.remove_tools(service="svc", tools="_all_tools"), agent)
        self.assertEqual(agent.list_tools(filter="available"), [])
        self.assertEqual([tool.name for tool in agent.list_tools(filter="removed")], ["alpha", "beta"])

        self.assertIs(agent.add_tools(service="svc", tools=["alpha"]), agent)
        self.assertEqual([tool.name for tool in agent.list_tools(filter="available")], ["alpha"])
        info = agent.get_tool_set_info(service="svc")
        self.assertEqual(info.total_tools, 2)
        self.assertEqual(info.available_tools, 1)
        self.assertEqual(info.removed_tools, 1)

        summary = agent.get_tool_set_summary()
        self.assertEqual(summary.agent_id, "agent-a")
        self.assertEqual(summary.total_services, 1)

        service_proxy = agent.find_service("svc")
        self.assertTrue(service_proxy.is_agent_scoped)
        self.assertIs(agent.reset_tools(service=service_proxy), agent)
        self.assertEqual([tool.name for tool in agent.list_tools(filter="available")], ["alpha", "beta"])

        other_agent = RustStoreContext(backend, agent_id="agent-b")
        with self.assertRaises(ValueError):
            other_agent.add_tools(service=service_proxy, tools=["alpha"])

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
                            "text": "preview caption",
                        }
                    ],
                    "is_error": False,
                }

        result = FakeContext().call_tool("snapshot", {})
        view = call_tool_response_helper(result)
        self.assertEqual(view.content, result["content"])
        self.assertEqual(view.text, "")
        self.assertEqual(view.artifacts[0]["type"], "image")
        self.assertEqual(view.artifacts[0]["text"], "preview caption")
        self.assertEqual(view.data["artifacts"][0]["mime_type"], "image/png")

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

        alias = Path("alias.json")
        with patch.object(StoreSetupManager, "_setup_rust_store", return_value="store") as setup:
            result = StoreSetupManager.setup_store(
                mcp_config_file=alias,
                external_db={"cache": {"type": "memory"}},
                cache_mode="hybrid",
            )

        self.assertEqual(result, "store")
        kwargs = setup.call_args.kwargs
        self.assertEqual(kwargs["mcpjson_path"], "alias.json")
        self.assertFalse(kwargs["only_db"])
        self.assertIsNotNone(kwargs["cache"])

    def test_setup_store_accepts_config_path_and_cache_config_aliases(self):
        from mcpstore.config import MemoryConfig
        from mcpstore.core.store.setup_manager import StoreSetupManager

        cache = MemoryConfig()
        with patch.object(StoreSetupManager, "_setup_rust_store", return_value="store") as setup:
            result = StoreSetupManager.setup_store(
                config_path=Path("config-alias.json"),
                cache_config=cache,
            )

        self.assertEqual(result, "store")
        kwargs = setup.call_args.kwargs
        self.assertEqual(kwargs["mcpjson_path"], "config-alias.json")
        self.assertIs(kwargs["cache"], cache)

    def test_setup_store_adds_static_config_through_rust_store(self):
        import asyncio

        from mcpstore.core.store.setup_manager import StoreSetupManager

        class FakeStore:
            def __init__(self):
                self.added = []

            def add_service(self, config):
                self.added.append(config)

        static_config = {"mcpServers": {"demo": {"url": "https://example.test/mcp"}}}
        sync_store = FakeStore()
        with patch.object(StoreSetupManager, "_setup_rust_store", return_value=sync_store):
            result = StoreSetupManager.setup_store(static_config=static_config)
        self.assertIs(result, sync_store)
        self.assertEqual(sync_store.added, [static_config])

        async_store = FakeStore()
        with patch.object(StoreSetupManager, "_setup_rust_store", return_value=async_store):
            result = asyncio.run(StoreSetupManager.setup_store_async(static_config=static_config))
        self.assertIs(result, async_store)
        self.assertEqual(async_store.added, [static_config])

    def test_rust_backend_setup_normalizes_pathlike_config_path(self):
        import types

        from mcpstore.core.store.rust_backend import RustStoreBackend

        class FakeRustStore:
            def load_from_config(self):
                pass

        class FakeMCPStore:
            called = None

            @staticmethod
            def setup_with_options(config_path, source_mode, backend, redis_url, namespace):
                FakeMCPStore.called = (config_path, source_mode, backend, redis_url, namespace)
                return FakeRustStore()

        fake_module = types.SimpleNamespace(MCPStore=FakeMCPStore)
        with patch("importlib.import_module", return_value=fake_module):
            RustStoreBackend.setup(config_path=Path("pathlike.json"))

        self.assertEqual(FakeMCPStore.called[0], "pathlike.json")

    def test_redis_client_config_normalizes_to_rust_url(self):
        from mcpstore.config import RedisConfig
        from mcpstore.core.store.rust_backend import RustStoreBackend

        class FakePool:
            connection_kwargs = {
                "host": "redis.local",
                "port": 6380,
                "db": 2,
                "username": "user",
                "password": "p@ss word",
            }

        class FakeRedis:
            connection_pool = FakePool()

        config = RedisConfig(client=FakeRedis(), namespace="team")
        backend, redis_url, namespace = RustStoreBackend._cache_options(config)

        self.assertEqual(backend, "redis")
        self.assertEqual(redis_url, "redis://user:p%40ss%20word@redis.local:6380/2")
        self.assertEqual(namespace, "team")

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

    def test_hub_sse_uses_rust_streamable_http_server_cli(self):
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
                    code = store.for_store().hub_sse(
                        host="0.0.0.0",
                        port=18081,
                        path="/sse",
                        block=True,
                    )

        self.assertEqual(code, 0)
        cmd = run.call_args.args[0]
        self.assertEqual(cmd[:4], ["/bin/mcpstore", "mcp-server", "--transport", "streamable-http"])
        self.assertIn("--host", cmd)
        self.assertIn("0.0.0.0", cmd)
        self.assertIn("--port", cmd)
        self.assertIn("18081", cmd)
        self.assertIn("--path", cmd)
        self.assertIn("/sse", cmd)
        self.assertIn("--scope", cmd)
        self.assertIn("store", cmd)

    def test_hub_stdio_delegates_to_rust_mcp_server_cli(self):
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
                    code = store.for_store().hub_stdio(block=True)

        self.assertEqual(code, 0)
        cmd = run.call_args.args[0]
        self.assertEqual(cmd[:4], ["/bin/mcpstore", "mcp-server", "--transport", "stdio"])
        self.assertIn("--scope", cmd)
        self.assertIn("store", cmd)
        self.assertIn("--config-path", cmd)
        self.assertIn("--source", cmd)
        self.assertNotIn("--host", cmd)
        self.assertNotIn("--port", cmd)
        self.assertNotIn("--path", cmd)

    def test_switch_cache_rebuilds_rust_store(self):
        from mcpstore.config import RedisConfig
        from mcpstore.core.store.rust_backend import RustStoreBackend

        class FakeRustInner:
            def __init__(self):
                self.loaded = False

            def load_from_config(self):
                self.loaded = True

        class FakeRustStore:
            called = None

            @staticmethod
            def setup_with_options(config_path, source_mode, backend, redis_url, namespace):
                FakeRustStore.called = (config_path, source_mode, backend, redis_url, namespace)
                return FakeRustInner()

        fake_module = type("FakeRustModule", (), {"MCPStore": FakeRustStore})
        store = RustStoreBackend(object())
        store._config_path = "mcp.json"
        store._only_db = True

        with patch("mcpstore.core.store.rust_backend.importlib.import_module", return_value=fake_module):
            context = store.for_store()
            self.assertIs(
                context.switch_cache(RedisConfig(url="redis://localhost:6379/0", namespace="switched")),
                context,
            )

        self.assertEqual(
            FakeRustStore.called,
            ("mcp.json", "db", "redis", "redis://localhost:6379/0", "switched"),
        )
        self.assertTrue(store._inner.loaded)

    def test_registry_facade_delegates_to_rust_cache_surface(self):
        import asyncio

        from mcpstore.config import MemoryConfig
        from mcpstore.core.store.rust_backend import RustStoreBackend

        class FakeInner:
            def cache_health_check(self):
                return {"healthy": True, "backend": "memory"}

            def cache_inspect(self):
                return {
                    "backend": "memory",
                    "namespace": "test",
                    "entities": [{"name": "svc"}],
                    "relations": [],
                    "states": [{"status": "ready"}],
                    "events": [],
                }

            def reset_config(self):
                self.reset = True

        store = RustStoreBackend(FakeInner())
        switched = []
        store.switch_cache = lambda config: switched.append(config) or True

        self.assertTrue(asyncio.run(store.registry.ping()))
        stats = asyncio.run(store.registry.get_cache_statistics())
        self.assertEqual(stats.backend, "memory")
        self.assertEqual(stats.entity_count, 1)
        self.assertEqual(stats.state_count, 1)
        self.assertTrue(asyncio.run(store.registry.clear_all()))
        self.assertTrue(store._inner.reset)
        self.assertTrue(asyncio.run(store.registry.reset_cache_statistics()))
        self.assertTrue(asyncio.run(store.registry.switch_backend(MemoryConfig())))
        self.assertEqual(switched[0].cache_type.value, "memory")

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
            paths = {route.path for route in api_main_router.routes}
            self.assertTrue(
                {
                    "/for_store/show_mcpjson",
                    "/for_store/setup_config",
                    "/for_store/reset_config",
                    "/for_store/sync_status",
                    "/for_store/list_agents",
                    "/for_store/tool_records",
                    "/for_store/service_status/{service_name}",
                    "/for_store/service_info/{service_name}",
                    "/for_store/update_config/{service_name}",
                    "/for_store/patch_service/{service_name}",
                    "/for_store/delete_config/{service_name}",
                    "/for_store/wait_service",
                    "/for_agent/{agent_id}/wait_service",
                    "/for_agent/{agent_id}/patch_service/{service_name}",
                    "/for_agent/{agent_id}/restart_service",
                    "/for_agent/{agent_id}/disconnect_service",
                    "/for_agent/{agent_id}/service_status/{service_name}",
                    "/for_agent/{agent_id}/service_info/{service_name}",
                    "/for_agent/{agent_id}/service/{service_name}",
                }.issubset(paths)
            )
        else:
            from mcpstore.api.api_dependencies import set_store as api_set_store

        api_set_store(store)
        self.assertIs(get_store(), store)
        self.assertEqual(get_namespace(MemoryConfig()), "mcpstore")
        kv_store = create_kv_store(MemoryConfig())
        self.assertEqual(kv_store.get_backend_type(), "memory")
        self.assertEqual(kv_store.get_scope(), "mcpstore")
        self.assertEqual(kv_store.read_entity(), [])
        self.assertEqual(kv_store.dump_all()["entities"], [])
        self.assertTrue(kv_store.health_check()["healthy"])
        self.assertEqual(ErrorCode.MISSING_PARAMETER.value, "missing_parameter")
        self.assertTrue(ResponseBuilder.success()["success"])
        self.assertFalse(ResponseBuilder.error()["success"])

        @timed_response
        def handler():
            return ResponseBuilder.success()

        self.assertIn("elapsed_ms", handler()["meta"])

    def test_python_package_declares_api_extra(self):
        project = tomllib.loads(Path("python/pyproject.toml").read_text(encoding="utf-8"))
        extras = project["project"]["optional-dependencies"]
        self.assertIn("api", extras)
        self.assertTrue(any(dep.startswith("fastapi") for dep in extras["api"]))
        self.assertTrue(any(dep.startswith("uvicorn") for dep in extras["api"]))

    def test_python_and_rust_versions_match(self):
        import mcpstore

        python_project = tomllib.loads(Path("python/pyproject.toml").read_text(encoding="utf-8"))
        rust_workspace = tomllib.loads(Path("rust/Cargo.toml").read_text(encoding="utf-8"))
        rust_crate = tomllib.loads(Path("rust/crates/mcpstore/Cargo.toml").read_text(encoding="utf-8"))
        rust_binding = tomllib.loads(Path("rust/bindings/python/Cargo.toml").read_text(encoding="utf-8"))

        version = python_project["project"]["version"]
        self.assertEqual(mcpstore.__version__, version)
        self.assertEqual(rust_workspace["workspace"]["package"]["version"], version)
        self.assertEqual(rust_crate["package"]["version"], version)
        self.assertTrue(rust_binding["package"]["version"]["workspace"])

    def test_api_wait_service_passes_status_to_rust_context(self):
        if importlib.util.find_spec("fastapi") is None:
            self.skipTest("fastapi is not installed")

        import asyncio

        from mcpstore.api.api_pack import api_set_store, agent_wait_service, store_wait_service

        class FakeContext:
            def __init__(self):
                self.wait_call = None

            async def wait_service_async(self, service_name, status=None, timeout=10.0):
                self.wait_call = (service_name, status, timeout)
                return {"service_global_name": service_name, "health_status": "ready"}

        class FakeStore:
            def __init__(self):
                self.context = FakeContext()

            def for_store(self):
                return self.context

        store = FakeStore()
        api_set_store(store)
        result = asyncio.run(
            store_wait_service(
                {"service_name": "demo", "status": ["healthy", "warning"], "timeout": 3}
            )
        )

        self.assertTrue(result["success"])
        self.assertEqual(store.context.wait_call, ("demo", ["healthy", "warning"], 3))

        class FakeAgentStore:
            def __init__(self):
                self.contexts = {}

            def for_agent(self, agent_id):
                context = FakeContext()
                self.contexts[agent_id] = context
                return context

        agent_store = FakeAgentStore()
        api_set_store(agent_store)
        result = asyncio.run(
            agent_wait_service(
                "agent-a",
                {"name": "demo", "status": "healthy", "timeout": 5},
            )
        )

        self.assertTrue(result["success"])
        self.assertEqual(agent_store.contexts["agent-a"].wait_call, ("demo", "healthy", 5))

    def test_api_service_management_routes_delegate_to_context(self):
        if importlib.util.find_spec("fastapi") is None:
            self.skipTest("fastapi is not installed")

        import asyncio

        from mcpstore.api.api_pack import (
            agent_disconnect_service,
            agent_patch_service,
            agent_restart_service,
            api_set_store,
            store_patch_service,
        )

        class FakeContext:
            def __init__(self):
                self.calls = []

            async def patch_service_async(self, service_name, payload):
                self.calls.append(("patch", service_name, payload))
                return True

            async def restart_service_async(self, service_name):
                self.calls.append(("restart", service_name))
                return True

            async def disconnect_service_async(self, service_name):
                self.calls.append(("disconnect", service_name))
                return True

        class FakeStore:
            def __init__(self):
                self.store_context = FakeContext()
                self.agent_contexts = {}

            def for_store(self):
                return self.store_context

            def for_agent(self, agent_id):
                context = FakeContext()
                self.agent_contexts[agent_id] = context
                return context

        store = FakeStore()
        api_set_store(store)

        result = asyncio.run(store_patch_service("demo", {"timeout": 3}))
        self.assertTrue(result["success"])
        self.assertEqual(store.store_context.calls, [("patch", "demo", {"timeout": 3})])

        result = asyncio.run(agent_patch_service("agent-a", "demo", {"timeout": 5}))
        self.assertTrue(result["success"])
        self.assertEqual(store.agent_contexts["agent-a"].calls, [("patch", "demo", {"timeout": 5})])

        result = asyncio.run(agent_restart_service("agent-b", {"service_name": "demo"}))
        self.assertTrue(result["success"])
        self.assertEqual(store.agent_contexts["agent-b"].calls, [("restart", "demo")])

        result = asyncio.run(agent_disconnect_service("agent-c", {"name": "demo"}))
        self.assertTrue(result["success"])
        self.assertEqual(store.agent_contexts["agent-c"].calls, [("disconnect", "demo")])

    def test_rust_context_keeps_cache_read_shape(self):
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
        cache = RustCacheProxy(context)
        self.assertEqual(cache.get_scope(), "global")
        self.assertEqual(cache.get_backend_type(), "memory")
        self.assertEqual(len(cache.read_entity(None, None)), 2)
        self.assertEqual(cache.read_entity("service", "demo")[0]["name"], "demo")
        self.assertEqual(cache.read_relation("binding", None)[0]["_key"], "demo:echo")
        self.assertEqual(cache.read_state("health", "demo")[0]["_type"], "health")


if __name__ == "__main__":
    unittest.main()
