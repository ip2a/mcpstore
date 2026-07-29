from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from mcpstore import (
    AgentContext,
    MCPStore,
    SessionContext,
    StoreContext,
    _rust,
)

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

    def test_real_binding_keeps_store_and_agent_instances_isolated(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            config_path = Path(tmp) / "mcp.json"
            config_path.write_text('{"mcpServers": {}}', encoding="utf-8")
            store = RustStoreBackend.setup(str(config_path), cache_config="memory")

            store.add_service(
                "gitodo",
                {"command": "command-that-must-not-run", "args": []},
            )
            agent_instance_id = store.declare_service_scope(
                "gitodo",
                {"type": "agent", "agent_id": "agent1"},
                {"config": {}},
            )

            instances = {
                instance["instance_id"]: instance
                for instance in store.list_instances()
            }
            self.assertEqual(
                set(instances),
                {STORE_INSTANCE_ID, AGENT_INSTANCE_ID},
            )
            self.assertEqual(agent_instance_id, AGENT_INSTANCE_ID)
            self.assertEqual(
                instances[STORE_INSTANCE_ID]["scope"],
                {"type": "store"},
            )
            self.assertEqual(
                instances[AGENT_INSTANCE_ID]["scope"],
                {"type": "agent", "agent_id": "agent1"},
            )
            self.assertEqual(
                store.get_effective_config("gitodo", {"type": "store"}),
                store.get_effective_config(
                    "gitodo",
                    {"type": "agent", "agent_id": "agent1"},
                ),
            )

            with self.assertRaisesRegex(ValueError, "Invalid instance_id"):
                store.find_instance("gitodo")

            store.remove_service_scope(
                "gitodo",
                {"type": "agent", "agent_id": "agent1"},
            )

            self.assertIsNotNone(store.find_instance(STORE_INSTANCE_ID))
            self.assertIsNone(store.find_instance(AGENT_INSTANCE_ID))
            self.assertEqual(
                [
                    instance["instance_id"]
                    for instance in store.list_instances()
                ],
                [STORE_INSTANCE_ID],
            )

            persisted = json.loads(config_path.read_text(encoding="utf-8"))
            scopes = persisted["mcpServers"]["gitodo"]["_mcpstore"]["scopes"]
            self.assertIn("store", scopes)
            self.assertNotIn("agent1", scopes.get("agents", {}))

    def test_public_mcpstore_exposes_scope_facade(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            config_path = Path(tmp) / "mcp.json"
            config_path.write_text('{"mcpServers": {}}', encoding="utf-8")
            store = MCPStore.setup_store(config_path=config_path, cache_mode="local")
            store_context = store.for_store()
            agent_context = store.for_agent("agent-a")

            store_id = store_context.add_service_config(
                "svc", {"command": "command-that-must-not-run", "args": []}
            )
            agent_id = agent_context.add_service_config(
                "svc", {"command": "command-that-must-not-run", "args": []}
            )
            self.assertEqual(
                [item["instance_id"] for item in store_context.list_services()], [store_id]
            )
            self.assertEqual(
                [item["instance_id"] for item in agent_context.list_services()], [agent_id]
            )

            agent_context.patch_service(
                service_name="svc", updates={"headers": {"X-Agent": "agent-a"}}
            )
            agent_context.update_service(
                instance_id=agent_id, config={"command": "updated", "args": []}
            )
            agent_context.remove_service(instance_id=agent_id)
            self.assertEqual(agent_context.list_services(), [])
            self.assertEqual(
                [item["instance_id"] for item in store_context.list_services()], [store_id]
            )


    def test_scope_facade_binding_keeps_store_and_agent_views_isolated(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            config_path = Path(tmp) / "mcp.json"
            config_path.write_text('{"mcpServers": {}}', encoding="utf-8")
            store = _rust.MCPStore.setup_with_options(
                str(config_path), backend="memory"
            )
            store_context = store.for_store()
            agent_context = store.for_agent("agent-a")

            self.assertEqual(store_context.scope(), {"type": "store"})
            self.assertEqual(
                agent_context.scope(),
                {"type": "agent", "agent_id": "agent-a"},
            )

            instance_id = agent_context.add_service_config(
                "svc",
                {"command": "command-that-must-not-run", "args": []},
            )

            self.assertTrue(instance_id)
            self.assertEqual(
                [
                    service["service_name"]
                    for service in agent_context.list_services()
                ],
                ["svc"],
            )
            self.assertEqual(store_context.list_services(), [])
            self.assertEqual(agent_context.list_tools(), [])
            service = agent_context.list_services()[0]
            self.assertEqual(service["state"]["instance_id"], instance_id)
            self.assertIn("phase", service["state"])

            agent_context.patch_service(
                service_name="svc", updates={"headers": {"X-Demo": "agent-a"}}
            )
            self.assertEqual(
                store.show_config()["mcpServers"]["svc"]["headers"]["X-Demo"],
                "agent-a",
            )
            with self.assertRaisesRegex(RuntimeError, "Scope Store is not declared"):
                store_context.patch_service(
                    service_name="svc", updates={"headers": {"X-Demo": "store"}}
                )

            instance_ids = store_context.add_service(
                {
                    "mcpServers": {
                        "other": {
                            "command": "command-that-must-not-run",
                            "args": [],
                        }
                    }
                }
            )

            self.assertEqual(len(instance_ids), 1)
            self.assertEqual(
                [
                    service["service_name"]
                    for service in store_context.list_services()
                ],
                ["other"],
            )
            self.assertEqual(
                [
                    service["service_name"]
                    for service in agent_context.list_services()
                ],
                ["svc"],
            )


    def test_scope_facade_lifecycle_uses_current_scope(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            config_path = Path(tmp) / "mcp.json"
            config_path.write_text('{"mcpServers": {}}', encoding="utf-8")
            store = _rust.MCPStore.setup_with_options(str(config_path), backend="memory")
            store_context = store.for_store()
            agent_context = store.for_agent("agent-a")
            store_id = store_context.add_service_config(
                "svc", {"command": "command-that-must-not-run", "args": []}
            )
            agent_id = agent_context.add_service_config(
                "svc", {"command": "command-that-must-not-run", "args": []}
            )

            agent_context.patch_service(
                instance_id=agent_id, updates={"headers": {"X-Demo": "agent-a"}}
            )
            agent_context.update_service(
                service_name="svc", config={"command": "updated", "args": []}
            )
            store_context.disconnect_service(service_name="svc")
            with self.assertRaisesRegex(RuntimeError, "does not belong to scope"):
                agent_context.restart_service(instance_id=store_id)
            with self.assertRaisesRegex(TypeError, "exactly one"):
                agent_context.remove_service()
            with self.assertRaisesRegex(RuntimeError, "Scope Agent"):
                agent_context.wait_service(service_name="missing", timeout=0.5)

            agent_context.remove_service(instance_id=agent_id)
            self.assertEqual(agent_context.list_services(), [])
            self.assertEqual(
                [item["instance_id"] for item in store_context.list_services()], [store_id]
            )


    def test_scope_facade_add_service_accepts_all_supported_inputs(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            config_path = Path(tmp) / "mcp.json"
            json_path = Path(tmp) / "services.json"
            toml_path = Path(tmp) / "services.toml"
            config_path.write_text('{"mcpServers": {}}', encoding="utf-8")
            json_path.write_text(
                '{"mcpServers": {"from-json-file": {"command": "command-that-must-not-run"}}}',
                encoding="utf-8",
            )
            toml_path.write_text(
                '[mcpServers.from-toml-file]\ncommand = "command-that-must-not-run"\n',
                encoding="utf-8",
            )
            context = _rust.MCPStore.setup_with_options(
                str(config_path), backend="memory"
            ).for_store()

            context.add_service({"mcpServers": {"document": {"command": "command-that-must-not-run"}}})
            context.add_service({"name": "single", "command": "command-that-must-not-run"})
            context.add_service([
                {"name": "list-one", "command": "command-that-must-not-run"},
                {"name": "list-two", "command": "command-that-must-not-run"},
            ])
            context.add_service('{"mcpServers": {"from-json-text": {"command": "command-that-must-not-run"}}}')
            context.add_service(json_path)
            context.add_service(str(toml_path))

            self.assertEqual(
                {service["service_name"] for service in context.list_services()},
                {
                    "document", "single", "list-one", "list-two",
                    "from-json-text", "from-json-file", "from-toml-file",
                },
            )


    def test_python_context_uses_native_scope_facade(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            config_path = Path(tmp) / "mcp.json"
            config_path.write_text('{"mcpServers": {}}', encoding="utf-8")
            store = RustStoreBackend.setup(str(config_path), cache_config="memory")

            agent_context = store.for_agent("agent-a")
            instance_id = agent_context.add_service_config(
                "svc",
                {"command": "command-that-must-not-run", "args": []},
            )

            self.assertTrue(instance_id)
            self.assertEqual([item["service_name"] for item in agent_context.list_services()], ["svc"])
            self.assertEqual(store.for_store().list_services(), [])
            self.assertEqual(agent_context.list_tools(), [])

if __name__ == "__main__":
    unittest.main()
