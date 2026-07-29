from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from mcpstore import AgentContext, MCPStore, Service, SessionContext, StoreContext, Tool, _rust
from mcpstore.store import RustStoreBackend


STORE_INSTANCE_ID = "c81af510-755b-55c7-8487-5668ab36e06e"
AGENT_INSTANCE_ID = "127ce370-1ed6-5b00-9713-e88d01b3010d"


class ScopeBindingIntegrationTests(unittest.TestCase):
    def test_public_context_exports_are_named_by_domain(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            config_path = Path(tmp) / "mcp.json"
            config_path.write_text('{"mcpServers": {}}', encoding="utf-8")
            store = MCPStore.setup_store(config_path=config_path, cache_mode="local")
            self.assertIsInstance(store.for_store(), StoreContext)
            self.assertIsInstance(store.for_agent("agent-a"), AgentContext)
            self.assertIsInstance(store.create_session("session-a"), SessionContext)
            self.assertFalse(hasattr(store, "reset_scope"))
            self.assertFalse(hasattr(store, "show_scope_config"))
            self.assertTrue(Service)
            self.assertTrue(Tool)

    def test_real_binding_keeps_store_and_agent_instances_isolated(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            config_path = Path(tmp) / "mcp.json"
            config_path.write_text('{"mcpServers": {}}', encoding="utf-8")
            store = RustStoreBackend.setup(str(config_path), cache_config="memory")
            store.add_service(
                "gitodo",
                {"command": "command-that-must-not-run", "args": [], "_mcpstore": {"scopes": {"store": {}, "agents": {"agent1": {}}}}},
            )
            instances = {item["instance_id"]: item for item in store.list_instances()}
            self.assertEqual(set(instances), {STORE_INSTANCE_ID, AGENT_INSTANCE_ID})
            self.assertEqual(instances[STORE_INSTANCE_ID]["scope"], {"type": "store"})
            self.assertEqual(instances[AGENT_INSTANCE_ID]["scope"], {"type": "agent", "agent_id": "agent1"})

    def test_show_config_keeps_session_as_a_separate_facade(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            config_path = Path(tmp) / "mcp.json"
            config_path.write_text('{"mcpServers": {}}', encoding="utf-8")
            store = MCPStore.setup_store(config_path=config_path, cache_mode="local")
            store_context = store.for_store().add_service_config("store-only", {"command": "command-that-must-not-run", "args": []})
            agent_context = store.for_agent("agent-a").add_service_config("agent-only", {"command": "command-that-must-not-run", "args": []})
            self.assertEqual(set(store_context.show_config()["mcpServers"]), {"store-only"})
            self.assertEqual(set(agent_context.show_config()["mcpServers"]), {"agent-only"})
            session = store.create_session("session-a", scope="agent", agent_id="agent-a")
            self.assertEqual(session.show_config(), agent_context.show_config())

    def test_context_mutations_return_the_same_context_domain(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            config_path = Path(tmp) / "mcp.json"
            config_path.write_text('{"mcpServers": {}}', encoding="utf-8")
            store = MCPStore.setup_store(config_path=config_path, cache_mode="local")
            context = store.for_agent("agent-a")
            context = context.add_service_config("svc", {"command": "command-that-must-not-run", "args": []})
            self.assertIsInstance(context, AgentContext)
            self.assertEqual(context.scope, {"type": "agent", "agent_id": "agent-a"})
            context = context.patch_service(service_name="svc", updates={"headers": {"X-Demo": "agent-a"}})
            context = context.update_service(service_name="svc", config={"command": "updated", "args": []})
            self.assertIsInstance(context, AgentContext)
            service = context.find_service(service_name="svc")
            self.assertIsInstance(service, Service)
            self.assertEqual(service.info()["service_name"], "svc")
            self.assertEqual(service.config()["command"], "updated")

    def test_raw_binding_uses_native_context_service_and_bool_results(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            config_path = Path(tmp) / "mcp.json"
            config_path.write_text('{"mcpServers": {}}', encoding="utf-8")
            context = _rust.MCPStore.setup_with_options(str(config_path), backend="memory").for_store()
            context = context.add_service_config("svc", {"command": "command-that-must-not-run", "args": []})
            self.assertEqual(context.scope(), {"type": "store"})
            service = context.find_service(service_name="svc")
            self.assertTrue(service.info()["instance_id"])
            self.assertTrue(service.remove_service())
            self.assertTrue(context.reset_config())

    def test_list_and_find_return_resource_objects_in_current_scope(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            config_path = Path(tmp) / "mcp.json"
            config_path.write_text('{"mcpServers": {}}', encoding="utf-8")
            store = MCPStore.setup_store(config_path=config_path, cache_mode="local")
            store_context = store.for_store().add_service_config("svc", {"command": "command-that-must-not-run", "args": []})
            agent_context = store.for_agent("agent-a").add_service_config("svc", {"command": "command-that-must-not-run", "args": []})
            store_service = store_context.find_service(service_name="svc")
            store_id = store_service.info()["instance_id"]
            agent_service = agent_context.find_service(service_name="svc")
            agent_id = agent_service.info()["instance_id"]
            self.assertIsInstance(store_service, Service)
            self.assertIsInstance(agent_service, Service)
            self.assertNotEqual(store_id, agent_id)
            self.assertEqual(agent_context.list_tools(), [])
            with self.assertRaisesRegex(RuntimeError, "does not belong to scope"):
                agent_context.find_service(instance_id=store_id)

    def test_service_mutations_return_service_and_delete_returns_bool(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            config_path = Path(tmp) / "mcp.json"
            config_path.write_text('{"mcpServers": {}}', encoding="utf-8")
            store = MCPStore.setup_store(config_path=config_path, cache_mode="local")
            context = store.for_agent("agent-a").add_service_config("svc", {"command": "command-that-must-not-run", "args": []})
            service = context.find_service(service_name="svc")
            service = service.patch_service({"headers": {"X-Service": "yes"}})
            self.assertIsInstance(service, Service)
            self.assertEqual(service.config()["headers"]["X-Service"], "yes")
            service = service.update_service({"command": "updated", "args": []})
            self.assertIsInstance(service, Service)
            self.assertEqual(service.config()["command"], "updated")
            self.assertTrue(service.remove_service())
            with self.assertRaisesRegex(RuntimeError, "not found"):
                service.info()

    def test_add_service_accepts_all_supported_inputs_and_returns_context(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            config_path = Path(tmp) / "mcp.json"
            json_path = Path(tmp) / "services.json"
            toml_path = Path(tmp) / "services.toml"
            config_path.write_text('{"mcpServers": {}}', encoding="utf-8")
            json_path.write_text('{"mcpServers": {"from-json-file": {"command": "command-that-must-not-run"}}}', encoding="utf-8")
            toml_path.write_text('[mcpServers.from-toml-file]\ncommand = "command-that-must-not-run"\n', encoding="utf-8")
            context = _rust.MCPStore.setup_with_options(str(config_path), backend="memory").for_store()
            for config in (
                {"mcpServers": {"document": {"command": "command-that-must-not-run"}}},
                {"name": "single", "command": "command-that-must-not-run"},
                [{"name": "list-one", "command": "command-that-must-not-run"}, {"name": "list-two", "command": "command-that-must-not-run"}],
                '{"mcpServers": {"from-json-text": {"command": "command-that-must-not-run"}}}',
                json_path,
                str(toml_path),
            ):
                context = context.add_service(config)
            self.assertEqual(
                {service.info()["service_name"] for service in context.list_services()},
                {"document", "single", "list-one", "list-two", "from-json-text", "from-json-file", "from-toml-file"},
            )

    def test_python_context_wraps_native_scope_facade_without_dict_resources(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            config_path = Path(tmp) / "mcp.json"
            config_path.write_text('{"mcpServers": {}}', encoding="utf-8")
            store = RustStoreBackend.setup(str(config_path), cache_config="memory")
            context = store.for_agent("agent-a").add_service_config("svc", {"command": "command-that-must-not-run", "args": []})
            service = context.list_services()[0]
            self.assertIsInstance(context, AgentContext)
            self.assertIsInstance(service, Service)
            self.assertEqual(service.info()["service_name"], "svc")
            self.assertEqual(store.for_store().list_services(), [])
            self.assertEqual(context.list_tools(), [])


if __name__ == "__main__":
    unittest.main()
