from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from mcpstore.core.store.rust_backend import RustStoreBackend


STORE_INSTANCE_ID = "c81af510-755b-55c7-8487-5668ab36e06e"
AGENT_INSTANCE_ID = "127ce370-1ed6-5b00-9713-e88d01b3010d"


class ScopeBindingIntegrationTests(unittest.TestCase):
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


if __name__ == "__main__":
    unittest.main()
