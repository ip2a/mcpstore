import tempfile
import unittest
import importlib.util
from pathlib import Path


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

        workdir = Path(tempfile.mkdtemp(prefix="mcpstore-python-chain-"))
        store = MCPStore.setup_store(str(workdir / "mcp.json"))
        context = store.for_store()

        self.assertIsNotNone(context.for_openai())
        self.assertIsNotNone(context.for_autogen())

        try:
            adapter = context.for_langchain()
        except ImportError:
            adapter = None
        self.assertTrue(adapter is None or hasattr(adapter, "list_tools"))

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
        from mcpstore.core.store.rust_backend import RustStoreContext

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

        backend = FakeBackend()
        context = RustStoreContext(backend)
        agent = context.find_agent("agent-a")
        self.assertEqual(agent.agent_id, "agent-a")
        self.assertEqual(context.find_cache().scope, "global")
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
        self.assertEqual(context.list_resources()[0].uri, "memory://doc")
        self.assertEqual(service.list_resource_templates()[0].uriTemplate, "memory://{name}")
        self.assertEqual(service.read_resource("memory://doc").text, "body")
        self.assertEqual(context.list_prompts()[0].name, "summarize")
        self.assertEqual(service.get_prompt("summarize", {"topic": "rust"}).arguments["topic"], "rust")

        self.assertTrue(context.update_service("demo", {"description": "patched"}))
        self.assertTrue(service.restart_service())
        self.assertTrue(service.delete_service())
        self.assertEqual(backend.patches, [("demo", {"description": "patched"})])
        self.assertEqual(backend.restarted, ["demo"])
        self.assertEqual(backend.removed, ["demo"])

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

    def test_deprecated_python_mcp_module_is_removed(self):
        self.assertIsNone(importlib.util.find_spec("mcpstore.mcp"))

    def test_python_adapters_use_rust_schema_field(self):
        from mcpstore.adapters.common import tool_input_schema

        schema = {"type": "object", "properties": {"text": {"type": "string"}}}
        self.assertEqual(tool_input_schema({"input_schema": schema}), schema)
        self.assertEqual(tool_input_schema({"inputSchema": schema}), schema)


if __name__ == "__main__":
    unittest.main()
