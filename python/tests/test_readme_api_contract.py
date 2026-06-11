import tempfile
import unittest
from pathlib import Path


class ReadmePythonApiContractTest(unittest.TestCase):
    def setUp(self):
        from mcpstore import MCPStore

        self.workdir = Path(tempfile.mkdtemp(prefix="mcpstore-readme-contract-"))
        self.store = MCPStore.setup_store(str(self.workdir / "mcp.json"))

    def test_store_and_agent_chain_entrypoints_remain_stable(self):
        store_context = self.store.for_store()
        agent_context = self.store.for_agent("agent1")

        for context in (store_context, agent_context):
            for name in (
                "add_service",
                "list_services",
                "list_tools",
                "find_service",
                "find_tool",
                "call_tool",
                "use_tool",
                "wait_service",
                "show_config",
                "check_services",
                "service_info",
                "patch_service",
                "update_service",
                "delete_service",
                "restart_service",
                "disconnect_service",
                "list_resources",
                "read_resource",
                "list_prompts",
                "get_prompt",
            ):
                self.assertTrue(callable(getattr(context, name)), name)

        store_context.add_service(
            {
                "name": "demo",
                "command": "python",
                "args": ["-c", "print(1)"],
                "transport": "stdio",
            }
        )
        agent_context.add_service(
            {
                "name": "agent_demo",
                "command": "python",
                "args": ["-c", "print(2)"],
                "transport": "stdio",
            }
        )

        store_names = {service.name for service in store_context.list_services()}
        agent_original_names = {
            service.original_name for service in agent_context.list_services()
        }

        self.assertIn("demo", store_names)
        self.assertIn("agent_demo", agent_original_names)

    def test_readme_adapter_chain_methods_remain_available(self):
        context = self.store.for_store()

        for name in (
            "for_langchain",
            "for_langgraph",
            "for_openai",
            "for_autogen",
            "for_crewai",
            "for_llamaindex",
            "for_semantic_kernel",
        ):
            self.assertTrue(callable(getattr(context, name)), name)

        openai_adapter = context.for_openai()
        self.assertEqual(openai_adapter.list_tools(), [])

    def test_setup_and_config_contract_remain_available(self):
        from mcpstore import MCPStore

        self.assertTrue(callable(MCPStore.setup_store))
        self.assertTrue(callable(MCPStore.setup_store_async))
        self.assertIsInstance(self.store.show_mcpjson(), dict)
        self.assertIsInstance(self.store.get_json_config(), dict)
        self.assertEqual(self.store.get_data_space_info()["backend"], "memory")

    def test_pyo3_binding_exposes_native_methods_not_json_string_bridge(self):
        from mcpstore._rust import MCPStore

        workdir = Path(tempfile.mkdtemp(prefix="mcpstore-pyo3-contract-"))
        rust_store = MCPStore.setup_with_options(
            str(workdir / "mcp.json"),
            "local",
            "memory",
            None,
            "contract",
        )

        for name in (
            "add_service",
            "list_services",
            "list_tools_scoped",
            "call_tool",
            "show_config",
            "wait_service_ready",
        ):
            self.assertTrue(callable(getattr(rust_store, name)), name)

        for name in (
            "add_service_json",
            "list_services_json",
            "list_tools_scoped_json",
            "call_tool_json",
            "show_config_json",
            "wait_service_ready_json",
        ):
            self.assertFalse(hasattr(rust_store, name), name)


if __name__ == "__main__":
    unittest.main()
