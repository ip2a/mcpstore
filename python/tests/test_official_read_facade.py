from __future__ import annotations

import unittest
from typing import Any

from mcpstore.core.models import RootScope
from mcpstore.store import RustStoreBackend


class RecordingCore:
    def __init__(self) -> None:
        self.calls: list[tuple[Any, ...]] = []

    def list_scopes(self) -> list[dict[str, Any]]:
        self.calls.append(("list_scopes",))
        return [{"scope": {"type": "root"}, "service_count": 1}]

    def scope_info(self, view: dict[str, Any]) -> dict[str, Any] | None:
        self.calls.append(("scope_info", view))
        return {"scope": view, "service_count": 1}

    def find_agent(self, agent_id: str) -> dict[str, Any] | None:
        self.calls.append(("find_agent", agent_id))
        return {"agent_id": agent_id, "instance_ids": ["instance-a"]}

    def list_services_viewed(self, view: dict[str, Any]) -> list[dict[str, Any]]:
        self.calls.append(("list_services_viewed", view))
        return [{"service_name": "demo", "scope": {"type": "store"}}]

    def show_scope_config(self, scope: dict[str, Any]) -> dict[str, Any]:
        self.calls.append(("show_scope_config", scope))
        return {"scope": scope}

    def reset_scope(self, scope: dict[str, Any]) -> None:
        self.calls.append(("reset_scope", scope))


class OfficialReadFacadeTests(unittest.TestCase):
    def test_official_reads_forward_scope_views_without_aggregation(self) -> None:
        core = RecordingCore()
        store = RustStoreBackend(core)

        self.assertEqual(
            store.list_scopes(),
            [{"scope": {"type": "root"}, "service_count": 1}],
        )
        self.assertEqual(
            store.scope_info(RootScope()),
            {"scope": {"type": "root"}, "service_count": 1},
        )
        self.assertEqual(
            store.find_agent("agent-a"),
            {"agent_id": "agent-a", "instance_ids": ["instance-a"]},
        )
        self.assertEqual(
            store.list_services_viewed({"type": "store"}),
            [{"service_name": "demo", "scope": {"type": "store"}}],
        )
        self.assertEqual(
            store.show_scope_config({"type": "store"}),
            {"scope": {"type": "store"}},
        )
        store.reset_scope({"type": "store"})
        self.assertEqual(
            core.calls,
            [
                ("list_scopes",),
                ("scope_info", {"type": "root"}),
                ("find_agent", "agent-a"),
                ("list_services_viewed", {"type": "store"}),
                ("show_scope_config", {"type": "store"}),
                ("reset_scope", {"type": "store"}),
            ],
        )

    def test_scope_view_rejects_agent_without_id(self) -> None:
        store = RustStoreBackend(RecordingCore())

        with self.assertRaises(ValueError):
            store.scope_info({"type": "agent"})


if __name__ == "__main__":
    unittest.main()
