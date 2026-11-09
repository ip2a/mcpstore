from __future__ import annotations

import unittest
from typing import Any

from fastapi.testclient import TestClient

from app import create_app


EXPECTED_ROUTES = {
    ("GET", "/health"),
    ("GET", "/agents/list"),
    ("GET", "/agents/{agent_id}"),
    ("GET", "/scopes/list"),
    ("GET", "/scopes/root"),
    ("GET", "/scopes/store"),
    ("GET", "/scopes/agents/{agent_id}"),
    ("GET", "/services/list"),
    ("GET", "/services/{service_name}"),
    ("POST", "/services/{service_name}"),
    ("PUT", "/services/{service_name}"),
    ("DELETE", "/services/{service_name}"),
    ("POST", "/services/{service_name}/connect"),
    ("POST", "/services/{service_name}/disconnect"),
    ("POST", "/services/{service_name}/restart"),
    ("GET", "/services/{service_name}/wait"),
    ("GET", "/services/{service_name}/tools/list"),
    ("GET", "/tools/list"),
    ("POST", "/services/{service_name}/tools/call"),
    ("POST", "/tools/call"),
    ("PUT", "/services/{service_name}/scopes/store"),
    ("DELETE", "/services/{service_name}/scopes/store"),
    ("PUT", "/services/{service_name}/scopes/agents/{agent_id}"),
    ("DELETE", "/services/{service_name}/scopes/agents/{agent_id}"),
}


class FakeService:
    def __init__(self, store: "FakeStore", service_name: str, scope: dict[str, str]):
        self.store = store
        self.service_name = service_name
        self.scope = scope

    def info(self) -> dict[str, Any]:
        self.store.calls.append(("service.info", self.service_name, self.scope))
        if self.service_name == "missing":
            raise RuntimeError("Service not found: missing")
        suffix = (
            "store"
            if self.scope["type"] == "store"
            else f"agent-{self.scope['agent_id']}"
        )
        return {
            "instance_id": f"instance-{self.service_name}-{suffix}",
            "service_name": self.service_name,
            "scope": self.scope,
            "transport": "streamable-http",
            "tools": [],
            "effective_config": {"url": "http://example.test/mcp"},
            "config_revision": {"base_revision": 1, "scope_revision": 1},
            "added_time": 1,
        }


class FakeContext:
    def __init__(self, store: "FakeStore", scope: dict[str, str]):
        self.store = store
        self.scope = scope

    def find_service(self, service_name: str) -> FakeService:
        self.store.calls.append(("find_service", service_name, self.scope))
        return FakeService(self.store, service_name, self.scope)


class FakeStore:
    def __init__(self) -> None:
        self.calls: list[tuple[Any, ...]] = []
        self.fail: set[str] = set()

    def _record(self, name: str, *args: Any) -> None:
        self.calls.append((name, *args))
        if name in self.fail:
            raise RuntimeError(f"{name} failed")

    def current_store(self) -> str:
        self._record("current_store")
        return "redis"

    def list_agents(self) -> list[dict[str, Any]]:
        self._record("list_agents")
        return [{"agent_id": "test_agent", "instance_ids": ["instance-agent"]}]

    def find_agent(self, agent_id: str) -> dict[str, Any] | None:
        self._record("find_agent", agent_id)
        if agent_id == "missing":
            return None
        return {"agent_id": agent_id, "instance_ids": ["instance-agent"]}

    def list_scopes(self) -> list[dict[str, Any]]:
        self._record("list_scopes")
        return [{"scope": {"type": "store"}, "service_count": 1}]

    def scope_info(self, view: dict[str, str]) -> dict[str, Any] | None:
        self._record("scope_info", view)
        if view.get("agent_id") == "missing":
            return None
        return {"scope": view, "services": [], "total": 0}

    def list_services_viewed(self, view: dict[str, str]) -> list[dict[str, Any]]:
        self._record("list_services_viewed", view)
        return [
            {
                "instance_id": "instance-demo-store",
                "service_name": "demo",
                "scope": view,
                "transport": "streamable-http",
                "tools": [],
                "effective_config": {},
                "config_revision": {"base_revision": 1, "scope_revision": 1},
                "added_time": 1,
            }
        ]

    def for_store(self) -> FakeContext:
        self._record("for_store")
        return FakeContext(self, {"type": "store"})

    def for_agent(self, agent_id: str) -> FakeContext:
        self._record("for_agent", agent_id)
        return FakeContext(self, {"type": "agent", "agent_id": agent_id})

    def add_service(self, service_name: str, config: dict[str, Any]) -> None:
        self._record("add_service", service_name, config)

    def update_service(self, service_name: str, config: dict[str, Any]) -> None:
        self._record("update_service", service_name, config)

    def remove_service(self, service_name: str) -> None:
        self._record("remove_service", service_name)

    def connect_service(self, instance_id: str) -> None:
        self._record("connect_service", instance_id)

    def disconnect_service(self, instance_id: str) -> None:
        self._record("disconnect_service", instance_id)

    def restart_service(self, instance_id: str) -> None:
        self._record("restart_service", instance_id)

    def wait_instance_ready(self, instance_id: str, timeout: int) -> dict[str, Any]:
        self._record("wait_instance_ready", instance_id, timeout)
        return {"status": "ready"}

    def list_tool_entries(
        self, instance_id: str, *, filter: str
    ) -> list[dict[str, Any]]:
        self._record("list_tool_entries", instance_id, filter)
        return [
            {
                "name": "echo",
                "description": "Echo input",
                "input_schema": {"type": "object"},
            }
        ]

    def call_tool(
        self, instance_id: str, tool_name: str, args: dict[str, Any]
    ) -> dict[str, Any]:
        self._record("call_tool", instance_id, tool_name, args)
        return {"content": [{"type": "text", "text": "ok"}], "is_error": False}

    def declare_service_scope(
        self,
        service_name: str,
        scope: dict[str, str],
        descriptor: dict[str, Any],
    ) -> str:
        self._record("declare_service_scope", service_name, scope, descriptor)
        return "request-id"

    def remove_service_scope(self, service_name: str, scope: dict[str, str]) -> None:
        self._record("remove_service_scope", service_name, scope)


class AppContractTests(unittest.TestCase):
    def setUp(self) -> None:
        self.store = FakeStore()
        self.app = create_app(self.store)
        self.client_context = TestClient(self.app)
        self.client = self.client_context.__enter__()

    def tearDown(self) -> None:
        self.client_context.__exit__(None, None, None)

    def assert_envelope(self, response, *, success: bool = True) -> dict[str, Any]:
        payload = response.json()
        self.assertEqual(
            set(payload),
            {"success", "message", "data", "errors", "meta", "pagination"},
        )
        self.assertEqual(payload["success"], success)
        self.assertEqual(
            set(payload["meta"]),
            {"timestamp", "request_id", "execution_time_ms", "api_version"},
        )
        self.assertEqual(payload["meta"]["execution_time_ms"], 0)
        self.assertEqual(payload["meta"]["api_version"], "1.0.0")
        self.assertTrue(payload["meta"]["timestamp"].endswith("Z"))
        self.assertTrue(payload["meta"]["request_id"].startswith("req_"))
        return payload

    def assert_error(self, response, status: int, code: str, field: str | None = None):
        self.assertEqual(response.status_code, status)
        payload = self.assert_envelope(response, success=False)
        self.assertIsNone(payload["data"])
        self.assertEqual(payload["errors"][0]["code"], code)
        self.assertEqual(payload["errors"][0]["field"], field)
        return payload

    def test_complete_official_route_set(self) -> None:
        actual = {
            (method, route.path)
            for route in self.app.routes
            if route.path
            not in {"/openapi.json", "/docs", "/docs/oauth2-redirect", "/redoc"}
            for method in route.methods
        }
        self.assertEqual(actual, EXPECTED_ROUTES)

    def test_health_and_success_envelope(self) -> None:
        health = self.client.get("/health")
        self.assertEqual(health.status_code, 200)
        self.assertEqual(health.json(), {"status": "ok", "store": "redis"})

        response = self.client.get("/agents/list")
        self.assertEqual(response.status_code, 200)
        payload = self.assert_envelope(response)
        self.assertIsNone(payload["errors"])
        self.assertIsNone(payload["pagination"])
        self.assertEqual(payload["data"]["total"], 1)

    def test_static_routes_precede_dynamic_routes(self) -> None:
        response = self.client.get("/services/list")
        self.assertEqual(response.status_code, 200)
        payload = self.assert_envelope(response)
        self.assertEqual(payload["data"]["services"][0]["service_name"], "demo")
        self.assertIn(("list_services_viewed", {"type": "store"}), self.store.calls)

        response = self.client.get("/agents/list")
        self.assertEqual(response.status_code, 200)
        self.assertIn(("list_agents",), self.store.calls)

    def test_scope_defaults_and_validation(self) -> None:
        self.client.get("/services/list?scope=root")
        self.assertIn(("list_services_viewed", {"type": "root"}), self.store.calls)

        response = self.client.get("/services/list?scope=agent")
        self.assert_error(response, 400, "MISSING_PARAMETER", "agent_id")

        response = self.client.get("/services/list?scope=invalid")
        self.assert_error(response, 400, "INVALID_PARAMETER", "scope")

        response = self.client.get("/services/demo?scope=root")
        self.assert_error(response, 400, "INVALID_PARAMETER", "scope")

        response = self.client.get("/services/demo/tools/list?filter=hidden")
        self.assert_error(response, 400, "INVALID_PARAMETER", "filter")

        response = self.client.get("/tools/list")
        self.assert_error(response, 400, "MISSING_PARAMETER", "service_name")

    def test_read_facade_routes(self) -> None:
        self.assertEqual(self.client.get("/agents/test_agent").status_code, 200)
        self.assertEqual(self.client.get("/scopes/list").status_code, 200)
        self.assertEqual(self.client.get("/scopes/root").status_code, 200)
        self.assertEqual(self.client.get("/scopes/store").status_code, 200)
        self.assertEqual(self.client.get("/scopes/agents/test_agent").status_code, 200)
        self.assertEqual(
            self.client.get("/services/demo?scope=agent&agent_id=test_agent").status_code,
            200,
        )
        self.assertIn(("find_agent", "test_agent"), self.store.calls)
        self.assertIn(("scope_info", {"type": "root"}), self.store.calls)
        self.assertIn(("scope_info", {"type": "store"}), self.store.calls)
        self.assertIn(
            ("scope_info", {"type": "agent", "agent_id": "test_agent"}),
            self.store.calls,
        )
        self.assertIn(("for_agent", "test_agent"), self.store.calls)

    def test_mutation_lifecycle_tool_and_scope_core_calls(self) -> None:
        add_config = {
            "url": "http://example.test/mcp",
            "_mcpstore": {"scopes": {"store": {}}},
        }
        self.assertEqual(
            self.client.post("/services/demo", json=add_config).status_code, 200
        )
        self.assertEqual(
            self.client.put(
                "/services/demo", json={"url": "http://example.test/new"}
            ).status_code,
            200,
        )
        self.assertEqual(self.client.delete("/services/demo").status_code, 200)
        self.assertEqual(self.client.post("/services/demo/connect").status_code, 200)
        self.assertEqual(
            self.client.post(
                "/services/demo/disconnect?scope=agent&agent_id=test_agent"
            ).status_code,
            200,
        )
        self.assertEqual(self.client.post("/services/demo/restart").status_code, 200)
        self.assertEqual(
            self.client.get("/services/demo/wait?timeout=20").status_code, 200
        )

        nested_tools = self.client.get(
            "/services/demo/tools/list?scope=agent&agent_id=test_agent&filter=all"
        )
        self.assertEqual(nested_tools.status_code, 200)
        self.assertEqual(nested_tools.json()["data"]["filter"], "all")
        top_tools = self.client.get("/tools/list?service_name=demo&filter=removed")
        self.assertEqual(top_tools.status_code, 200)
        self.assertEqual(top_tools.json()["data"]["service_name"], "demo")

        self.assertEqual(
            self.client.post(
                "/services/demo/tools/call", json={"tool_name": "echo"}
            ).status_code,
            200,
        )
        self.assertEqual(
            self.client.post(
                "/tools/call",
                json={
                    "service_name": "demo",
                    "scope": "agent",
                    "agent_id": "test_agent",
                    "tool_name": "echo",
                    "args": None,
                },
            ).status_code,
            200,
        )

        descriptor = {"config": {"headers": {"X-Test": "1"}}}
        self.assertEqual(
            self.client.put(
                "/services/demo/scopes/store", json=descriptor
            ).status_code,
            200,
        )
        self.assertEqual(
            self.client.delete("/services/demo/scopes/store").status_code, 200
        )
        self.assertEqual(
            self.client.put(
                "/services/demo/scopes/agents/test_agent", json={}
            ).status_code,
            200,
        )
        self.assertEqual(
            self.client.delete(
                "/services/demo/scopes/agents/test_agent"
            ).status_code,
            200,
        )

        expected = [
            ("add_service", "demo", add_config),
            ("update_service", "demo", {"url": "http://example.test/new"}),
            ("remove_service", "demo"),
            ("connect_service", "instance-demo-store"),
            ("disconnect_service", "instance-demo-agent-test_agent"),
            ("restart_service", "instance-demo-store"),
            ("wait_instance_ready", "instance-demo-store", 20),
            ("list_tool_entries", "instance-demo-agent-test_agent", "all"),
            ("list_tool_entries", "instance-demo-store", "removed"),
            ("call_tool", "instance-demo-store", "echo", {}),
            ("call_tool", "instance-demo-agent-test_agent", "echo", {}),
            ("declare_service_scope", "demo", {"type": "store"}, descriptor),
            ("remove_service_scope", "demo", {"type": "store"}),
            (
                "declare_service_scope",
                "demo",
                {"type": "agent", "agent_id": "test_agent"},
                {},
            ),
            (
                "remove_service_scope",
                "demo",
                {"type": "agent", "agent_id": "test_agent"},
            ),
        ]
        for call in expected:
            self.assertIn(call, self.store.calls)

    def test_non_object_args_and_update_scope_metadata(self) -> None:
        response = self.client.post(
            "/services/demo/tools/call",
            json={"tool_name": "echo", "args": ["invalid"]},
        )
        self.assert_error(response, 400, "INVALID_PARAMETER", "args")

        response = self.client.put(
            "/services/demo",
            json={"url": "http://example.test/mcp", "_mcpstore": {}},
        )
        self.assert_error(response, 400, "INVALID_REQUEST")

    def test_stable_error_mapping(self) -> None:
        response = self.client.get("/services/missing")
        self.assert_error(response, 404, "SERVICE_NOT_FOUND", "service_name")

        self.store.fail.add("restart_service")
        response = self.client.post("/services/demo/restart")
        self.assert_error(response, 502, "SERVICE_OPERATION_FAILED")

        self.store.fail.add("list_agents")
        response = self.client.get("/agents/list")
        self.assert_error(response, 500, "INTERNAL_ERROR")

        response = self.client.get("/agents/missing")
        self.assert_error(response, 404, "AGENT_NOT_FOUND", "agent_id")

    def test_old_routes_are_absent(self) -> None:
        for method, path in [
            ("get", "/for_store/list_services"),
            ("get", "/for_agent/test_agent/list_services"),
            ("get", "/services"),
            ("post", "/services"),
            ("get", "/tools/echo"),
            ("post", "/tools/echo/call"),
        ]:
            with self.subTest(method=method, path=path):
                response = getattr(self.client, method)(path)
                self.assertEqual(response.status_code, 404)


if __name__ == "__main__":
    unittest.main()
