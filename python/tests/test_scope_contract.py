from __future__ import annotations

import unittest
from typing import Any

from mcpstore.adapters.openai_adapter import OpenAIAdapter
from mcpstore.core.models import ScopeDescriptor
from mcpstore.store import RustStoreBackend
from mcpstore.store.setup import StoreSetupManager


class RecordingCore:
    def __init__(self) -> None:
        self.calls: list[tuple[Any, ...]] = []

    def add_service(self, service_name: str, config: dict[str, Any]) -> None:
        self.calls.append(("add_service", service_name, config))

    def declare_service_scope(
        self,
        service_name: str,
        scope: dict[str, Any],
        descriptor: dict[str, Any],
    ) -> str:
        self.calls.append(
            ("declare_service_scope", service_name, scope, descriptor)
        )
        return "instance-agent-a"

    def remove_service_scope(
        self,
        service_name: str,
        scope: dict[str, Any],
    ) -> None:
        self.calls.append(("remove_service_scope", service_name, scope))

    def update_service(self, service_name: str, config: dict[str, Any]) -> None:
        self.calls.append(("update_service", service_name, config))

    def patch_service(self, service_name: str, updates: dict[str, Any]) -> None:
        self.calls.append(("patch_service", service_name, updates))

    def connect_service(self, instance_id: str) -> None:
        self.calls.append(("connect_service", instance_id))

    def list_tools(self, instance_id: str) -> list[dict[str, Any]]:
        self.calls.append(("list_tools", instance_id))
        return [
            {
                "name": "echo",
                "description": "Echo text",
                "input_schema": {
                    "type": "object",
                    "properties": {"text": {"type": "string"}},
                    "required": ["text"],
                },
            }
        ]

    def call_tool(
        self,
        instance_id: str,
        tool_name: str,
        args: dict[str, Any],
    ) -> dict[str, Any]:
        self.calls.append(("call_tool", instance_id, tool_name, args))
        return {"content": [{"type": "text", "text": args.get("text", "")}]}

    def export_instance_config(
        self,
        instance_id: str,
        format: str | None,
    ) -> dict[str, Any]:
        self.calls.append(("export_instance_config", instance_id, format))
        return {"mcpServers": {}}


class ScopeContractTests(unittest.TestCase):
    def setUp(self) -> None:
        self.core = RecordingCore()
        self.store = RustStoreBackend(self.core)

    def test_facade_has_no_agent_add_adapter(self) -> None:
        self.assertFalse(hasattr(self.store, "add_service_for_agent"))

    def test_add_and_scope_operations_are_explicit(self) -> None:
        config = {"command": "demo", "args": ["serve"]}
        self.store.add_service("demo", config)
        instance_id = self.store.declare_service_scope(
            "demo",
            {"type": "agent", "agent_id": "agent-a"},
            ScopeDescriptor(config={"env": {"TOKEN": "agent"}}),
        )
        self.store.remove_service_scope(
            "demo",
            {"type": "agent", "agent_id": "agent-a"},
        )

        self.assertEqual(instance_id, "instance-agent-a")
        self.assertEqual(
            self.core.calls,
            [
                ("add_service", "demo", config),
                (
                    "declare_service_scope",
                    "demo",
                    {"type": "agent", "agent_id": "agent-a"},
                    {
                        "config": {"env": {"TOKEN": "agent"}},
                        "lifecycle": None,
                    },
                ),
                (
                    "remove_service_scope",
                    "demo",
                    {"type": "agent", "agent_id": "agent-a"},
                ),
            ],
        )

    def test_update_and_patch_reject_mcpstore(self) -> None:
        for method, payload in (
            (self.store.update_service, {"_mcpstore": {"scopes": {}}}),
            (self.store.patch_service, {"_mcpstore": None}),
        ):
            with self.subTest(method=method.__name__):
                with self.assertRaises(ValueError):
                    method("demo", payload)
        self.assertEqual(self.core.calls, [])

    def test_update_and_patch_forward_base_fields_only(self) -> None:
        self.store.update_service("demo", {"command": "updated", "args": ["serve"]})
        self.store.patch_service("demo", {"env": {"TOKEN": "changed"}})

        self.assertEqual(
            self.core.calls,
            [
                ("update_service", "demo", {"command": "updated", "args": ["serve"]}),
                ("patch_service", "demo", {"env": {"TOKEN": "changed"}}),
            ],
        )

    def test_runtime_and_export_use_instance_id(self) -> None:
        self.store.connect_service("instance-a")
        tools = self.store.list_tools("instance-a")
        self.store.call_tool("instance-a", "echo", {"text": "hello"})
        self.store.export_instance_config("instance-a", "json")

        self.assertEqual(tools[0]["instance_id"], "instance-a")
        self.assertEqual(
            self.core.calls,
            [
                ("connect_service", "instance-a"),
                ("list_tools", "instance-a"),
                ("call_tool", "instance-a", "echo", {"text": "hello"}),
                ("export_instance_config", "instance-a", "json"),
            ],
        )

    def test_openai_adapter_is_bound_to_one_instance(self) -> None:
        adapter = OpenAIAdapter(self.store, "instance-a")
        self.assertEqual(adapter.list_tools()[0]["function"]["name"], "echo")
        output = adapter.execute_tool_call(
            {"name": "echo", "arguments": {"text": "hello"}}
        )

        self.assertEqual(output, "hello")
        self.assertEqual(
            self.core.calls,
            [
                ("list_tools", "instance-a"),
                ("call_tool", "instance-a", "echo", {"text": "hello"}),
            ],
        )

    def test_static_config_registers_each_definition(self) -> None:
        StoreSetupManager._add_static_config(
            self.store,
            {
                "mcpServers": {
                    "one": {"command": "one"},
                    "two": {"url": "https://example.test/mcp"},
                }
            },
        )
        self.assertEqual(
            self.core.calls,
            [
                ("add_service", "one", {"command": "one"}),
                ("add_service", "two", {"url": "https://example.test/mcp"}),
            ],
        )


if __name__ == "__main__":
    unittest.main()
