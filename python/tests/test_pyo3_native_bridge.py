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
        self.assertNotIn("transport_type", services[0])
        with self.assertRaises(AttributeError):
            getattr(services[0], "name")
        self.assertIsInstance(context.show_config(), dict)

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
        self.assertEqual(tool_input_schema({"inputSchema": schema}), {})


if __name__ == "__main__":
    unittest.main()
