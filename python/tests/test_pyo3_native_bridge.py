import tempfile
import unittest
import importlib.util
import json
import os
import subprocess
import sys
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

    def test_rust_binding_imports_openapi_spec_as_shared_analysis(self):
        from mcpstore._rust import MCPStore

        workdir = Path(tempfile.mkdtemp(prefix="mcpstore-openapi-native-"))
        store = MCPStore.setup_with_options(
            str(workdir / "mcp.json"),
            "local",
            "memory",
            None,
            "openapi-native",
        )
        spec = {
            "openapi": "3.0.0",
            "info": {"title": "Inventory", "version": "2026.1"},
            "servers": [{"url": "https://inventory.example.test"}],
            "paths": {
                "/items": {
                    "get": {"operationId": "listItems", "summary": "List items"},
                    "post": {"operationId": "createItem", "requestBody": {"required": True}},
                },
                "/items/{sku}": {
                    "get": {
                        "parameters": [
                            {
                                "name": "sku",
                                "in": "path",
                                "required": True,
                                "schema": {"type": "string"},
                            }
                        ]
                    }
                },
            },
        }

        result = store.import_openapi_service_from_spec("inventory", "memory://inventory", spec)

        self.assertEqual(result["service_name"], "inventory")
        self.assertEqual(result["total_endpoints"], 3)
        self.assertEqual(result["component_types"]["tools"], 1)
        self.assertEqual(result["component_types"]["resources"], 1)
        self.assertEqual(result["component_types"]["resource_templates"], 1)
        self.assertTrue(result["runtime_executable"])
        self.assertEqual(store.get_openapi_import("inventory")["spec_info"]["title"], "Inventory")
        self.assertEqual(len(store.list_openapi_imports()), 1)

    def test_python_facade_imports_openapi_yaml_text_via_rust(self):
        from mcpstore import MCPStore

        workdir = Path(tempfile.mkdtemp(prefix="mcpstore-openapi-yaml-"))
        store = MCPStore.setup_store(str(workdir / "mcp.json"))
        context = store.for_store()
        spec_text = """
openapi: 3.0.0
info:
  title: YAML Facade
  version: '2026.1'
servers:
  - url: https://yaml.example.test
paths:
  /items:
    get:
      operationId: listYamlItems
      responses:
        '200':
          description: ok
"""

        result = context.import_openapi_service_from_spec(
            "yaml-facade",
            "memory://yaml-facade",
            spec_text,
        )

        self.assertEqual(result["spec_info"]["title"], "YAML Facade")
        self.assertEqual(result["component_types"]["resources"], 1)

    def test_python_facade_bundles_openapi_file_refs_via_rust(self):
        from mcpstore import MCPStore

        workdir = Path(tempfile.mkdtemp(prefix="mcpstore-openapi-bundle-"))
        components_dir = workdir / "components"
        components_dir.mkdir()
        (components_dir / "shared.yaml").write_text(
            """
components:
  schemas:
    ItemId:
      type: string
      description: bundled local item id
    Item:
      type: object
      properties:
        id:
          $ref: '#/components/schemas/ItemId'
""".strip(),
            encoding="utf-8",
        )
        spec_path = workdir / "openapi.yaml"
        spec_path.write_text(
            """
openapi: 3.0.0
info:
  title: Bundle Facade
  version: '2026.1'
servers:
  - url: https://bundle.example.test
paths:
  /items:
    get:
      operationId: listBundledItems
      responses:
        '200':
          description: ok
          content:
            application/json:
              schema:
                $ref: components/shared.yaml#/components/schemas/Item
""".strip(),
            encoding="utf-8",
        )

        store = MCPStore.setup_store(str(workdir / "mcp.json"))
        bundled = store.for_store().bundle_openapi_spec(spec_path.as_posix())

        schema = bundled["paths"]["/items"]["get"]["responses"]["200"]["content"][
            "application/json"
        ]["schema"]
        self.assertEqual(bundled["info"]["title"], "Bundle Facade")
        self.assertEqual(
            schema["properties"]["id"],
            {"type": "string", "description": "bundled local item id"},
        )

        artifact = store.for_store().bundle_openapi_artifact(spec_path.as_posix())
        artifact_schema = artifact["bundle"]["paths"]["/items"]["get"]["responses"]["200"][
            "content"
        ]["application/json"]["schema"]
        self.assertEqual(artifact_schema, schema)
        self.assertEqual(artifact["spec_url"], spec_path.as_posix())
        self.assertEqual(artifact["diagnostics"], [])
        self.assertTrue(
            any(
                document["role"] == "root" and document["url"].endswith("/openapi.yaml")
                for document in artifact["documents"]
            )
        )
        for document in artifact["documents"]:
            self.assertTrue(document["content_hash"].startswith("blake3:"))
            self.assertGreater(document["content_length"], 0)
        self.assertTrue(
            any(
                dependency["source_ref"] == "components/shared.yaml#/components/schemas/Item"
                and dependency["target_document"].endswith("/components/shared.yaml")
                and dependency["pointer"] == "/components/schemas/Item"
                for dependency in artifact["dependencies"]
            )
        )

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
        self.assertEqual(services[0].client_id, "demo")
        self.assertEqual(services[0]["client_id"], "demo")
        self.assertEqual(services[0].transport_type, "stdio")
        self.assertEqual(services[0]["transport_type"], "stdio")
        self.assertIsInstance(context.show_config(), dict)
        self.assertIn("mcpServers", store.show_mcpjson())
        self.assertIn("mcpServers", context.show_mcpjson())
        self.assertIsInstance(store.get_json_config(), dict)
        self.assertEqual(store.get_data_space_info()["backend"], "memory")

    def test_record_view_reads_service_config_fields_without_fake_defaults(self):
        from mcpstore.core.store.rust_backend import RustRecordView

        record = RustRecordView(
            {
                "name": "demo",
                "config": {
                    "args": ["-c", "print(1)"],
                    "env": {"A": "B"},
                    "headers": {},
                    "workingDir": "/tmp/demo",
                },
            }
        )

        self.assertEqual(record.args, ["-c", "print(1)"])
        self.assertEqual(record["args"], ["-c", "print(1)"])
        self.assertEqual(record.env, {"A": "B"})
        self.assertEqual(record.working_dir, "/tmp/demo")
        self.assertEqual(record.workingDir, "/tmp/demo")
        self.assertIn("args", record)
        self.assertEqual(record.to_dict()["config"]["args"], ["-c", "print(1)"])

        missing = RustRecordView({"name": "demo"})
        self.assertNotIn("args", missing)
        self.assertNotIn("data", missing)
        with self.assertRaises(AttributeError):
            _ = missing.args
        with self.assertRaises(KeyError):
            _ = missing["data"]

        comparable = RustRecordView(
            {
                "name": "demo",
                "config": {"args": ["-c", "print(1)"]},
            }
        )
        self.assertEqual(len({record, comparable}), 2)

    def test_rust_store_backend_has_no_python_fallback_without_pyo3(self):
        import importlib

        from mcpstore.core.store.rust_backend import RustStoreBackend

        original_import_module = importlib.import_module

        def fail_rust_extension(name, *args, **kwargs):
            if name == "mcpstore._rust":
                raise ImportError("missing PyO3 extension")
            return original_import_module(name, *args, **kwargs)

        with patch("importlib.import_module", side_effect=fail_rust_extension):
            with self.assertRaisesRegex(ImportError, "missing PyO3 extension"):
                RustStoreBackend.setup()

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

        session = store.for_store().create_session("export-session")
        session.bind_service("demo")
        output_backup = workdir / "output-backup.json"
        session_export = asyncio.run(
            store.export_to_json(output_path=output_backup, include_sessions=True)
        )
        self.assertIn("sessions", session_export)
        self.assertIn("store:global:export-session", session_export["sessions"]["entities"])
        self.assertIn(
            "store:global:export-session",
            session_export["sessions"]["relations"]["session_services"],
        )
        self.assertIn(
            "store:global:export-session",
            session_export["sessions"]["states"]["session_status"],
        )
        self.assertTrue(session_export["sessions"]["events"])
        self.assertEqual(json.loads(output_backup.read_text(encoding="utf-8")), session_export)

        restored = MCPStore.setup_store(str(workdir / "restored-mcp.json"))
        self.assertTrue(asyncio.run(restored.import_from_json(str(output_backup))))
        restored_sessions = restored.for_store().list_sessions()
        self.assertEqual(len(restored_sessions), 1)
        self.assertEqual(restored_sessions[0].session_id, "export-session")
        self.assertEqual(restored_sessions[0].status, "active")
        self.assertEqual(restored_sessions[0].list_services()[0].name, "demo")

        filepath_backup = workdir / "filepath-backup.json"
        written = asyncio.run(store.exportjson(filepath=filepath_backup))
        self.assertEqual(written, exported)
        self.assertEqual(json.loads(filepath_backup.read_text(encoding="utf-8")), exported)

        with self.assertRaisesRegex(TypeError, "output_path"):
            store.exportjson(output_path=output_backup)
        with self.assertRaisesRegex(TypeError, "filepath"):
            store.export_to_json(filepath=filepath_backup)

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
                self.updates = []

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

            def update_service(self, name, config):
                self.updates.append((name, config))
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
                return [
                    {
                        "event_type": "TOOL_CALL_COMPLETED",
                        "event_id": "evt-tool-1",
                        "timestamp": 20,
                        "payload": {
                            "service_name": "demo",
                            "tool_name": "echo",
                            "arguments": {"text": "history"},
                            "latency_ms": 1.5,
                            "is_error": False,
                            "status": "success",
                        },
                    },
                    {"event_type": "TEST", "count": count, "timestamp": 10, "payload": {"ok": True}},
                ][:count]

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
        with self.assertRaisesRegex(NotImplementedError, "hub_http"):
            context.hub_sse()
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
        self.assertTrue(service.tools_stats().history_available)
        self.assertTrue(service.update_service({"url": "https://updated.example.test/mcp"}))
        self.assertTrue(service.patch_service({"headers": {"X-Test": "1"}}))
        self.assertFalse(hasattr(service, "update_config"))
        self.assertFalse(hasattr(service, "patch_config"))
        self.assertEqual(service.tools_stats().call_count, 1)
        self.assertEqual(service.tools_stats().error_count, 0)
        self.assertEqual(service.tools_stats().last_called_at, 20)

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
        self.assertTrue(tool.usage_stats().history_available)
        self.assertEqual(tool.usage_stats().call_count, 1)
        self.assertEqual(tool.usage_stats().error_count, 0)
        self.assertEqual(tool.usage_stats().last_called_at, 20)
        self.assertEqual(tool.call_history(limit=5)[0].arguments.text, "history")
        self.assertEqual(tool.call_tool({"text": "ok"}, return_extracted=True), "ok")
        self.assertEqual(tool.call_tool({"text": "ok"}).text_output, "ok")
        self.assertEqual(tool.call_tool({"text": "ok"}).to_dict()["content"][0]["text"], "ok")
        with self.assertRaisesRegex(ValueError, "timeout"):
            tool.call_tool({"text": "ok"}, timeout=10)
        self.assertEqual(tool.test_call({"text": "probe"}).text_output, "probe")
        self.assertEqual(context.use_tool("echo", {"text": "alias"}, return_extracted=True), "alias")
        with self.assertRaisesRegex(ValueError, "timeout"):
            context.call_tool("echo", {"text": "alias"}, timeout=10)
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
        self.assertTrue(service.restart_service())
        self.assertTrue(service.remove_service())
        self.assertEqual(context.event_history(2)[1].event_type, "TEST")
        self.assertTrue(context.event_capability_report().event_bus)
        self.assertEqual(context.show_config("client").clients["client-a"].service, "demo")
        self.assertEqual(backend.show_config("clients").clients["client-a"].service, "demo")
        self.assertEqual(set(context.show_config("mcp").mcpServers), {"demo", "agent-demo", "other-agent-demo"})
        self.assertEqual(set(agent.show_config("all").mcpServers), {"agent-demo"})
        self.assertEqual(agent.show_config("agent").agents["agent-a"], ["agent-demo"])
        self.assertEqual(agent.get_stats().service_count, 1)
        self.assertEqual(asyncio.run(agent.get_stats_async()).healthy_services, 1)
        self.assertEqual(agent.map_global("svc"), "svc_byagent_agent-a")
        self.assertEqual(agent.map_local("svc_byagent_agent-a"), "svc")
        self.assertEqual(context.wait_service("demo", status="healthy").health_status, "ready")
        self.assertEqual(context.wait_service("demo", status=["healthy", "warning"]).health_status, "ready")
        self.assertEqual(context.wait_services(["demo"], status="healthy")["demo"].health_status, "ready")
        with self.assertRaises(TimeoutError):
            context.wait_service("demo", status="degraded")
        self.assertTrue(service.restart_service())
        self.assertTrue(service.delete_service())
        self.assertEqual(
            inner.patches,
            [
                ("demo", {"headers": {"X-Test": "1"}}),
            ],
        )
        self.assertEqual(
            inner.updates,
            [
                ("demo", {"url": "https://updated.example.test/mcp"}),
                ("demo", {"description": "patched"}),
            ],
        )
        self.assertEqual(inner.restarted, ["demo", "demo", "demo"])
        self.assertEqual(inner.removed, ["demo", "demo"])

    def test_add_service_accepts_public_config_shapes_through_rust(self):
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

        backend.for_agent("agent-a").add_service({"name": "local", "command": "python"})
        self.assertEqual(inner.agent_added[0][0], "agent-a")
        self.assertEqual(inner.agent_added[0][1], "local")

        with self.assertRaisesRegex(ValueError, "不再接受 JSON 字符串配置"):
            backend.for_agent("agent-a").add_service(
                json.dumps({"name": "legacy", "command": "python"})
            )

        agent = backend.for_agent("agent-b")
        self.assertIs(
            agent.add_service({"wide-agent": {"url": "https://agent.example.test/mcp"}}),
            agent,
        )
        self.assertEqual(inner.agent_added[1][0], "agent-b")
        self.assertEqual(inner.agent_added[1][1], "wide-agent")

    def test_service_config_mutations_require_python_dicts(self):
        from mcpstore.core.store.rust_backend import RustStoreBackend

        class FakeInner:
            def __init__(self):
                self.patches = []
                self.updates = []

            def patch_service(self, name, updates):
                self.patches.append((name, updates, type(updates)))

            def update_service(self, name, config):
                self.updates.append((name, config, type(config)))

        inner = FakeInner()
        backend = RustStoreBackend(inner)
        context = backend.for_store()

        self.assertTrue(backend.patch_service("demo", {"description": "patched"}))
        self.assertTrue(backend.update_service("demo", {"url": "https://example.test/mcp"}))
        self.assertTrue(
            context.update_service(
                "demo",
                {"url": "https://example.test/mcp", "headers": {"X-Test": "1"}},
            )
        )
        self.assertTrue(context.update_service("demo", {"command": "python"}))

        self.assertEqual(inner.patches[0], ("demo", {"description": "patched"}, dict))
        self.assertEqual(inner.updates[0], ("demo", {"url": "https://example.test/mcp"}, dict))
        self.assertEqual(
            inner.updates[1],
            ("demo", {"url": "https://example.test/mcp", "headers": {"X-Test": "1"}}, dict),
        )
        self.assertEqual(inner.updates[2], ("demo", {"command": "python"}, dict))

        with self.assertRaisesRegex(ValueError, "不再接受 JSON 字符串配置"):
            backend.patch_service("demo", '{"description": "legacy"}')
        with self.assertRaisesRegex(ValueError, "不再接受 JSON 字符串配置"):
            context.update_service("demo", '{"command": "python"}')

    def test_rust_context_exposes_documented_async_facade_methods(self):
        import asyncio

        from mcpstore.core.store.rust_backend import RustStoreBackend

        class FakeInner:
            def __init__(self):
                self.added = []

            def add_service(self, name, config):
                self.added.append((name, config))

            def list_services_scoped(self, agent_id=None):
                return [{"name": "demo"}]

            def list_tools_scoped(self, agent_id=None, service_name=None):
                return [
                    {
                        "name": "demo_echo",
                        "original_name": "echo",
                        "service_name": "demo",
                    }
                ]

            def wait_service_ready(self, name, timeout):
                return {"service_global_name": name, "health_status": "ready"}

            def show_config(self):
                return {"mcpServers": {"demo": {}}, "agents": {}, "clients": {}}

        backend = RustStoreBackend(FakeInner())
        context = backend.for_store()

        async def run():
            added_context = await context.add_service_async(
                {"name": "demo", "url": "https://example.test/mcp"}
            )
            self.assertIs(added_context, context)
            self.assertEqual((await context.list_services_async())[0].name, "demo")
            self.assertEqual((await context.list_tools_async())[0].name, "demo_echo")
            self.assertEqual((await context.find_tool_async("echo")).name, "echo")
            self.assertEqual(
                (await context.wait_service_async("demo", status="healthy")).health_status,
                "ready",
            )
            self.assertEqual(set((await context.show_config_async("mcp")).mcpServers), {"demo"})

        asyncio.run(run())

    def test_call_tool_does_not_parse_json_string_arguments_in_python_facade(self):
        from mcpstore.core.store.rust_backend import RustStoreBackend

        class FakeInner:
            def __init__(self):
                self.calls = []
                self.sessions = {}

            def find_session(self, session_id, scope="store", agent_id=None):
                return self.sessions.get(session_id)

            def create_session(self, session_id, scope="store", agent_id=None, lease_seconds=None, metadata=None):
                session = {
                    "session_key": f"{scope}:{agent_id or 'global'}:{session_id}",
                    "session_id": session_id,
                    "scope": scope,
                    "agent_id": agent_id,
                    "metadata": metadata or {},
                }
                self.sessions[session_id] = session
                return session

            def get_session_status(self, session_key):
                return {"session_key": session_key, "status": "active"}

            def list_session_services(self, session_key):
                return []

            def list_tools_in_session(self, session_key):
                return self.list_tools_scoped()

            def call_tool_in_session(self, session_key, tool_name, args):
                return self.call_tool("demo", tool_name, args)

            def list_tools_scoped(self, agent_id=None, service_name=None):
                return [{"name": "echo", "service_name": service_name or "demo"}]

            def resolve_tool_for_agent(self, agent_id, user_input):
                return {"global_service_name": "demo", "canonical_tool_name": "echo"}

            def call_tool(self, service_name, tool_name, args):
                self.calls.append((service_name, tool_name, args))
                return {"content": [{"type": "text", "text": "ok"}], "is_error": False}

        inner = FakeInner()
        backend = RustStoreBackend(inner)
        result = backend.call_tool("demo", "echo", '{"text": "ok"}')

        self.assertEqual(inner.calls, [])
        self.assertTrue(result.is_error)
        self.assertEqual(result.data.arguments, {})
        self.assertIn("不再接受 JSON 字符串参数", result.error)

        self.assertTrue(backend.call_tool("demo", "echo", "").is_error)
        self.assertEqual(inner.calls, [])

        context = backend.for_store()
        self.assertTrue(context.call_tool("echo", "").is_error)
        self.assertTrue(context.find_tool("echo", service_name="demo").call_tool("").is_error)

        with context.with_session("tool-json-string") as session:
            self.assertTrue(session.use_tool("echo", "").is_error)

        result = backend.call_tool("demo", "echo", {})
        self.assertFalse(result.is_error)
        self.assertIsNone(result.data)
        self.assertEqual(inner.calls, [("demo", "echo", {})])

        inner.calls.clear()
        self.assertEqual(inner.calls, [])

        ok = context.call_tool("echo", None)
        self.assertFalse(ok.is_error)
        self.assertEqual(inner.calls, [("demo", "echo", {})])

    def test_prompt_arguments_require_python_dicts(self):
        from mcpstore.core.store.rust_backend import RustStoreBackend

        class FakeInner:
            def __init__(self):
                self.calls = []

            def get_prompt_scoped(self, agent_id, prompt_name, arguments, service_name=None):
                self.calls.append((agent_id, prompt_name, arguments, service_name))
                return {"name": prompt_name, "arguments": arguments}

        inner = FakeInner()
        backend = RustStoreBackend(inner)
        context = backend.for_store()

        self.assertEqual(context.get_prompt("summarize", None).arguments, {})
        with self.assertRaisesRegex(ValueError, "不再接受 JSON 字符串参数"):
            context.get_prompt("summarize", "")
        with self.assertRaisesRegex(ValueError, "不再接受 JSON 字符串参数"):
            context.find_service("demo").get_prompt("summarize", "")

        self.assertEqual(inner.calls, [(None, "summarize", {}, None)])

    def test_python_facade_keeps_session_api_shape(self):
        from mcpstore.core.store.rust_backend import RustStoreBackend, RustStoreContext

        class FakeBackend:
            def __init__(self):
                self.sessions = {}
                self.bindings = {}
                self.session_states = {}

            def find_session(self, session_id, scope="store", agent_id=None):
                return self.sessions.get((scope, agent_id, session_id))

            def get_session(self, session_key):
                for entity in self.sessions.values():
                    if entity["session_key"] == session_key:
                        return entity
                return None

            def find_session_by_user_session_id(self, user_session_id):
                for entity in self.sessions.values():
                    if entity["metadata"].get("user_session_id") == user_session_id:
                        return entity
                return None

            def update_session_metadata(self, session_key, metadata):
                entity = self.get_session(session_key)
                if entity is None:
                    return None
                entity["metadata"] = metadata
                return entity

            def create_session(self, session_id, scope="store", agent_id=None, lease_seconds=None, metadata=None):
                entity = {
                    "session_key": f"{scope}:{agent_id or 'global'}:{session_id}",
                    "session_id": session_id,
                    "scope": scope,
                    "agent_id": agent_id,
                    "metadata": metadata or {},
                    "lease_seconds": lease_seconds,
                    "expires_at": None,
                    "version": 1,
                }
                self.sessions[(scope, agent_id, session_id)] = entity
                return entity

            def list_sessions(self, scope=None, agent_id=None):
                return [
                    entity
                    for (saved_scope, saved_agent_id, _), entity in self.sessions.items()
                    if (scope is None or saved_scope == scope)
                    and (agent_id is None or saved_agent_id == agent_id)
                ]

            def get_session_status(self, session_key):
                return {"session_key": session_key, "status": "active"}

            def extend_session(self, session_key, seconds):
                for entity in self.sessions.values():
                    if entity["session_key"] == session_key:
                        entity["lease_seconds"] = seconds
                        entity["version"] += 1
                        return entity
                raise RuntimeError("missing session")

            def extend_session_with_retry(self, session_key, seconds, max_attempts=3, delay_millis=0):
                return self.extend_session(session_key, seconds)

            def close_session(self, session_key, reason=None):
                return {"session_key": session_key, "status": "closed", "reason": reason}

            def bind_service_to_session(self, session_key, service_name):
                services = self.bindings.setdefault(session_key, [])
                if service_name not in services:
                    services.append(service_name)
                return {"session_key": session_key, "services": services}

            def bind_service_to_session_with_retry(self, session_key, service_name, max_attempts=3, delay_millis=0):
                return self.bind_service_to_session(session_key, service_name)

            def unbind_service_from_session(self, session_key, service_name):
                self.bindings.setdefault(session_key, []).remove(service_name)
                return {"session_key": session_key, "services": self.bindings[session_key]}

            def unbind_service_from_session_with_retry(self, session_key, service_name, max_attempts=3, delay_millis=0):
                return self.unbind_service_from_session(session_key, service_name)

            def list_session_services(self, session_key):
                return [
                    {
                        "service_global_name": service,
                        "service_original_name": service,
                        "source_agent": "global_agent_store",
                    }
                    for service in self.bindings.get(session_key, [])
                ]

            def get_session_state_value(self, session_key, key):
                return self.session_states.get(session_key, {}).get("values", {}).get(key)

            def list_session_state(self, session_key):
                return self.session_states.setdefault(
                    session_key,
                    {
                        "session_key": session_key,
                        "values": {},
                        "updated_at": 0,
                        "version": 0,
                    },
                )

            def set_session_state(self, session_key, key, value):
                state = self.list_session_state(session_key)
                state["values"][key] = value
                state["version"] += 1
                return state

            def set_session_state_with_retry(self, session_key, key, value, max_attempts=3, delay_millis=0):
                return self.set_session_state(session_key, key, value)

            def delete_session_state(self, session_key, key):
                state = self.list_session_state(session_key)
                state["values"].pop(key, None)
                state["version"] += 1
                return state

            def delete_session_state_with_retry(self, session_key, key, max_attempts=3, delay_millis=0):
                return self.delete_session_state(session_key, key)

            def clear_session_state(self, session_key):
                state = self.list_session_state(session_key)
                state["values"].clear()
                state["version"] += 1
                return state

            def list_tools_in_session(self, session_key):
                services = self.bindings.get(session_key) or ["browser", "search"]
                tools = []
                for service in services:
                    tools.extend(self.list_tools_scoped(service_name=service))
                return tools

            def call_tool_in_session(self, session_key, tool_name, args):
                for tool in self.list_tools_in_session(session_key):
                    if tool["name"] == tool_name:
                        service_name = tool["service_name"]
                        return self.call_tool(service_name, tool_name, args)
                raise RuntimeError("missing tool")

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
        with self.assertRaisesRegex(TypeError, "user_session_id"):
            context.create_session("browser_task", user_session_id="legacy")
        with self.assertRaisesRegex(TypeError, "is_user_session_id"):
            context.find_session("browser_task", is_user_session_id=True)

        with context.with_session("browser_task") as active:
            active.bind_service("browser")
            self.assertIs(context.current_session(), active)
            self.assertEqual(active.service_count, 1)
            self.assertEqual(active.list_tools()[0].name, "browser_navigate")
            self.assertEqual(active.use_tool("browser_navigate", {}, return_extracted=True), "browser:browser_navigate")
            with self.assertRaisesRegex(ValueError, "timeout"):
                active.use_tool("browser_navigate", {}, timeout=10)
            self.assertIs(active.extend_session(), active)
            self.assertIs(active.extend_session_with_retry(max_attempts=2, delay_millis=0), active)
            self.assertEqual(active.session_info().lease_seconds, 3600)
            self.assertIs(active.bind_service_with_retry("search", max_attempts=2, delay_millis=0), active)
            self.assertEqual(active.service_count, 2)
            self.assertIs(active.unbind_service_with_retry("search", max_attempts=2, delay_millis=0), active)
            self.assertEqual(active.service_count, 1)
            self.assertIs(active.set_state("cursor", {"page": 2}), active)
            self.assertEqual(active.get_state("cursor").page, 2)
            self.assertIs(active.set_state_with_retry("answer", 42, max_attempts=2), active)
            self.assertEqual(active.state_values().answer, 42)
            self.assertIs(active.delete_state_with_retry("cursor", max_attempts=2), active)
            self.assertIsNone(active.get_state("cursor"))
            self.assertTrue(active.clear_cache())
            self.assertEqual(active.state_values(), {})

            fresh_context = RustStoreContext(backend)
            self.assertEqual(fresh_context.active_session.session_id, "browser_task")
            self.assertEqual([tool.name for tool in fresh_context.list_tools()], ["browser_navigate"])

        self.assertIsNone(context.active_session)
        context.session_auto("auto_browser")
        self.assertEqual(context.active_session.session_id, "auto_browser")
        self.assertEqual(context.active_session.session_info().lease_seconds, 720000)
        self.assertEqual(context.active_session.metadata.created_by, "python_session_auto")
        context.session_manual()

        context.session_auto(default_timeout=1, auto_cleanup=True, session_prefix="legacy_")
        self.assertEqual(context.active_session.session_id, "legacy_session_default")
        self.assertEqual(context.active_session.session_info().lease_seconds, 1)
        self.assertTrue(context.active_session.metadata.auto_cleanup)
        self.assertEqual(context.active_session.metadata.session_prefix, "legacy_")
        context.session_manual()
        self.assertIsNone(context.active_session)

    def test_pyo3_session_bridge_exposes_rust_core_session_methods(self):
        from mcpstore._rust import MCPStore

        workdir = Path(tempfile.mkdtemp(prefix="mcpstore-pyo3-session-"))
        config_path = str(workdir / "mcp.json")
        store = MCPStore.setup_with_options(config_path, "local", "memory", None, "session-smoke")

        created = store.create_session("shared", "store", None, 30, {"owner": "test"})

        self.assertEqual(created["session_key"], "store:global:shared")
        self.assertEqual(store.find_session("shared", "store", None)["session_key"], created["session_key"])
        self.assertEqual(store.get_session_status(created["session_key"])["status"], "active")
        extended = store.extend_session(created["session_key"], 60)
        self.assertEqual(extended["lease_seconds"], 60)
        self.assertEqual(store.find_session("shared", "store", None)["lease_seconds"], 60)
        state = store.set_session_state(created["session_key"], "cursor", {"page": 2})
        self.assertEqual(state["values"]["cursor"]["page"], 2)
        self.assertEqual(store.get_session_state_value(created["session_key"], "cursor")["page"], 2)
        state = store.set_session_state_with_retry(
            created["session_key"],
            "answer",
            42,
            max_attempts=2,
            delay_millis=0,
        )
        self.assertEqual(state["values"]["answer"], 42)
        state = store.delete_session_state_with_retry(
            created["session_key"],
            "cursor",
            max_attempts=2,
            delay_millis=0,
        )
        self.assertNotIn("cursor", state["values"])
        self.assertEqual(store.clear_session_state(created["session_key"])["values"], {})
        self.assertEqual(store.close_session(created["session_key"], "done")["status"], "closed")

    def test_pyo3_session_bridge_shares_redis_backend_namespace_when_available(self):
        redis_url = os.getenv("MCPSTORE_TEST_REDIS_URL")
        if not redis_url:
            self.skipTest("MCPSTORE_TEST_REDIS_URL is not set")

        from mcpstore._rust import MCPStore

        workdir = Path(tempfile.mkdtemp(prefix="mcpstore-pyo3-redis-session-"))
        config_path = str(workdir / "mcp.json")
        namespace = f"session-redis-{os.getpid()}"
        first = MCPStore.setup_with_options(config_path, "local", "redis", redis_url, namespace)
        second = MCPStore.setup_with_options(config_path, "local", "redis", redis_url, namespace)

        created = first.create_session("shared", "store", None, 30, {"owner": "test"})

        self.assertEqual(second.find_session("shared", "store", None)["session_key"], created["session_key"])
        second.extend_session(created["session_key"], 60)
        self.assertEqual(first.find_session("shared", "store", None)["lease_seconds"], 60)

    def test_python_and_rust_session_share_redis_backend_across_processes_when_available(self):
        redis_url = os.getenv("MCPSTORE_TEST_REDIS_URL")
        if not redis_url:
            self.skipTest("MCPSTORE_TEST_REDIS_URL is not set")

        from mcpstore import MCPStore
        from mcpstore.config import RedisConfig

        workdir = Path(tempfile.mkdtemp(prefix="mcpstore-py-rust-redis-session-"))
        namespace = f"session-cross-process-{os.getpid()}"
        config_path = str(workdir / "mcp.json")
        python_store = MCPStore.setup_store(
            config_path,
            cache=RedisConfig(url=redis_url, namespace=namespace),
            cache_mode="shared",
        )
        python_session = python_store.for_store().create_session("py-created")
        python_session.extend_session(45)

        child = subprocess.run(
            [
                sys.executable,
                "-c",
                """
import json
import sys
from mcpstore._rust import MCPStore

config_path, redis_url, namespace = sys.argv[1:4]
store = MCPStore.setup_with_options(config_path, "local", "redis", redis_url, namespace)
seen = store.find_session("py-created", "store", None)
created = store.create_session("rust-created", "store", None, 90, {"owner": "rust-child"})
store.extend_session(seen["session_key"], 120)
print(json.dumps({
    "seen_key": seen["session_key"],
    "seen_lease_seconds": store.find_session("py-created", "store", None)["lease_seconds"],
    "created_key": created["session_key"],
}))
""",
                config_path,
                redis_url,
                namespace,
            ],
            check=True,
            capture_output=True,
            text=True,
        )
        payload = json.loads(child.stdout)

        self.assertEqual(payload["seen_key"], python_session.session_key)
        self.assertEqual(payload["seen_lease_seconds"], 120)
        self.assertEqual(
            python_store.for_store().find_session("py-created").session_info().lease_seconds,
            120,
        )
        rust_session = python_store.for_store().find_session("rust-created")
        self.assertIsNotNone(rust_session)
        self.assertEqual(rust_session.metadata["owner"], "rust-child")

    def test_switch_cache_to_redis_migrates_session_for_other_process_when_available(self):
        redis_url = os.getenv("MCPSTORE_TEST_REDIS_URL")
        if not redis_url:
            self.skipTest("MCPSTORE_TEST_REDIS_URL is not set")

        from mcpstore import MCPStore
        from mcpstore.config import RedisConfig

        workdir = Path(tempfile.mkdtemp(prefix="mcpstore-session-migrate-redis-"))
        namespace = f"session-migrate-{os.getpid()}"
        config_path = str(workdir / "mcp.json")
        store = MCPStore.setup_store(config_path)
        session = store.for_store().create_session("migrated")
        session.extend_session(75)
        session.set_state("cursor", {"page": 3})
        store.switch_cache(RedisConfig(url=redis_url, namespace=namespace))

        child = subprocess.run(
            [
                sys.executable,
                "-c",
                """
import json
import sys
from mcpstore._rust import MCPStore

config_path, redis_url, namespace = sys.argv[1:4]
store = MCPStore.setup_with_options(config_path, "local", "redis", redis_url, namespace)
seen = store.find_session("migrated", "store", None)
cursor = store.get_session_state_value(seen["session_key"], "cursor")
print(json.dumps({
    "session_key": seen["session_key"],
    "lease_seconds": seen["lease_seconds"],
    "cursor_page": cursor["page"],
}))
""",
                config_path,
                redis_url,
                namespace,
            ],
            check=True,
            capture_output=True,
            text=True,
        )
        payload = json.loads(child.stdout)

        self.assertEqual(payload["session_key"], session.session_key)
        self.assertEqual(payload["lease_seconds"], 75)
        self.assertEqual(payload["cursor_page"], 3)

    def test_redis_session_concurrent_bind_extend_converges_across_processes_when_available(self):
        redis_url = os.getenv("MCPSTORE_TEST_REDIS_URL")
        if not redis_url:
            self.skipTest("MCPSTORE_TEST_REDIS_URL is not set")

        from mcpstore._rust import MCPStore

        workdir = Path(tempfile.mkdtemp(prefix="mcpstore-session-concurrent-redis-"))
        namespace = f"session-concurrent-{os.getpid()}"
        config_path = str(workdir / "mcp.json")
        go_path = workdir / "go"
        store = MCPStore.setup_with_options(config_path, "local", "redis", redis_url, namespace)
        session = store.create_session("shared-concurrent", "store", None, 30, {})

        child_code = """
import json
import os
import sys
import time
from mcpstore._rust import MCPStore

config_path, redis_url, namespace, session_key, service_name, go_path, lease_base = sys.argv[1:8]
lease_base = int(lease_base)
store = MCPStore.setup_with_options(config_path, "local", "redis", redis_url, namespace)
store.add_service(service_name, {
    "command": sys.executable,
    "args": ["-c", "print(1)"],
    "env": {},
    "headers": {},
    "transport": "stdio",
})
deadline = time.time() + 10
while not os.path.exists(go_path):
    if time.time() > deadline:
        print(json.dumps({"service": service_name, "success": False, "error": "start barrier timed out"}))
        sys.exit(1)
    time.sleep(0.01)

try:
    store.bind_service_to_session_with_retry(session_key, service_name, max_attempts=80, delay_millis=10)
    for offset in range(20):
        store.extend_session_with_retry(session_key, lease_base + offset, max_attempts=80, delay_millis=10)
    print(json.dumps({
        "service": service_name,
        "success": True,
    }))
except Exception as exc:
    print(json.dumps({
        "service": service_name,
        "success": False,
        "error": str(exc),
    }))
    sys.exit(1)
"""
        children = [
            subprocess.Popen(
                [
                    sys.executable,
                    "-c",
                    child_code,
                    config_path,
                    redis_url,
                    namespace,
                    session["session_key"],
                    f"svc{index}",
                    str(go_path),
                    str(100 + index * 100),
                ],
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
            )
            for index in range(4)
        ]
        go_path.write_text("go", encoding="utf-8")
        results = []
        for child in children:
            stdout, stderr = child.communicate(timeout=30)
            self.assertEqual(child.returncode, 0, stderr or stdout)
            results.append(json.loads(stdout))

        self.assertTrue(all(result["success"] for result in results))
        services = store.list_session_services(session["session_key"])
        self.assertEqual(
            {service["service_global_name"] for service in services},
            {"svc0", "svc1", "svc2", "svc3"},
        )
        final = store.find_session("shared-concurrent", "store", None)
        self.assertIn(final["lease_seconds"], {119, 219, 319, 419})

    def test_python_and_rust_session_state_share_redis_backend_across_processes_when_available(self):
        redis_url = os.getenv("MCPSTORE_TEST_REDIS_URL")
        if not redis_url:
            self.skipTest("MCPSTORE_TEST_REDIS_URL is not set")

        from mcpstore import MCPStore
        from mcpstore.config import RedisConfig

        workdir = Path(tempfile.mkdtemp(prefix="mcpstore-py-rust-redis-session-state-"))
        namespace = f"session-state-cross-process-{os.getpid()}"
        config_path = str(workdir / "mcp.json")
        python_store = MCPStore.setup_store(
            config_path,
            cache=RedisConfig(url=redis_url, namespace=namespace),
            cache_mode="shared",
        )
        python_session = python_store.for_store().create_session("state-shared")
        python_session.set_state("cursor", {"page": 1})
        python_session.set_state("answer", 42)

        child = subprocess.run(
            [
                sys.executable,
                "-c",
                """
import json
import sys
from mcpstore._rust import MCPStore

config_path, redis_url, namespace = sys.argv[1:4]
store = MCPStore.setup_with_options(config_path, "local", "redis", redis_url, namespace)
seen = store.find_session("state-shared", "store", None)
cursor_before = store.get_session_state_value(seen["session_key"], "cursor")
answer_before = store.get_session_state_value(seen["session_key"], "answer")
store.set_session_state_with_retry(
    seen["session_key"],
    "cursor",
    {"page": 2},
    max_attempts=5,
    delay_millis=5,
)
store.delete_session_state_with_retry(
    seen["session_key"],
    "answer",
    max_attempts=5,
    delay_millis=5,
)
state = store.set_session_state(seen["session_key"], "child", {"ok": True})
print(json.dumps({
    "session_key": seen["session_key"],
    "cursor_before": cursor_before,
    "answer_before": answer_before,
    "values": state["values"],
}))
""",
                config_path,
                redis_url,
                namespace,
            ],
            check=True,
            capture_output=True,
            text=True,
        )
        payload = json.loads(child.stdout)

        self.assertEqual(payload["session_key"], python_session.session_key)
        self.assertEqual(payload["cursor_before"], {"page": 1})
        self.assertEqual(payload["answer_before"], 42)
        self.assertEqual(python_session.get_state("cursor").page, 2)
        self.assertIsNone(python_session.get_state("answer"))
        self.assertTrue(python_session.get_state("child").ok)

        python_session.clear_cache()
        child = subprocess.run(
            [
                sys.executable,
                "-c",
                """
import json
import sys
from mcpstore._rust import MCPStore

config_path, redis_url, namespace, session_key = sys.argv[1:5]
store = MCPStore.setup_with_options(config_path, "local", "redis", redis_url, namespace)
print(json.dumps(store.list_session_state(session_key)["values"]))
""",
                config_path,
                redis_url,
                namespace,
                python_session.session_key,
            ],
            check=True,
            capture_output=True,
            text=True,
        )
        self.assertEqual(json.loads(child.stdout), {})

    def test_python_facade_session_state_comes_from_rust_core(self):
        from mcpstore._rust import MCPStore as RustMCPStore
        from mcpstore.core.store.rust_backend import RustStoreBackend

        workdir = Path(tempfile.mkdtemp(prefix="mcpstore-python-session-core-"))
        config_path = str(workdir / "mcp.json")
        rust = RustMCPStore.setup_with_options(config_path, "local", "memory", None, "python-session")
        first = RustStoreBackend(rust)
        second = RustStoreBackend(rust)

        first_session = first.for_store().create_session("shared")
        second_session = second.for_store().find_session("shared")

        self.assertIsNotNone(second_session)
        self.assertEqual(second_session.session_key, first_session.session_key)
        second_session.extend_session(120)
        self.assertEqual(first.for_store().find_session("shared").session_info().lease_seconds, 120)
        first_session.set_state("cursor", {"page": 1})
        self.assertEqual(second_session.get_state("cursor").page, 1)
        second_session.set_state_with_retry("cursor", {"page": 2}, max_attempts=2)
        self.assertEqual(first_session.get_state("cursor").page, 2)
        second_session.clear_cache()
        self.assertEqual(first_session.state_values(), {})
        second_session.close_session()
        self.assertFalse(first.for_store().find_session("shared").is_active)

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
        self.assertEqual(AutoGenAdapter(context).get_functions()[0](text="auto2"), "auto2")

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

    def test_adapters_preserve_object_shaped_non_text_payload(self):
        from mcpstore.adapters.common import call_tool_response_helper

        class ImageContent:
            type = "image"
            data = "base64-object-image"
            mimeType = "image/png"
            width = 640
            height = 480

        view = call_tool_response_helper({"content": [ImageContent()], "is_error": False})

        self.assertEqual(view.text, "")
        self.assertEqual(view.artifacts[0]["type"], "image")
        self.assertEqual(view.artifacts[0]["data"], "base64-object-image")
        self.assertEqual(view.artifacts[0]["mimeType"], "image/png")
        self.assertEqual(view.data["artifacts"][0]["width"], 640)

    def test_python_mcp_module_is_removed(self):
        self.assertIsNone(importlib.util.find_spec("mcpstore.mcp"))

    def test_python_adapters_use_rust_schema_field(self):
        from mcpstore.adapters.common import tool_input_schema

        schema = {"type": "object", "properties": {"text": {"type": "string"}}}
        self.assertEqual(tool_input_schema({"input_schema": schema}), schema)
        self.assertEqual(tool_input_schema({"inputSchema": schema}), schema)

    def test_setup_store_normalizes_public_cache_options_without_old_core(self):
        from mcpstore.config import MemoryConfig, RedisConfig
        from mcpstore.core.store.setup_manager import StoreSetupManager

        redis = RedisConfig(url="redis://localhost:6379/0", namespace="team")
        cache, only_db = StoreSetupManager._normalize_cache_options(
            cache=redis,
            cache_mode="shared",
            only_db=False,
        )
        self.assertIs(cache, redis)
        self.assertEqual(cache.url, "redis://localhost:6379/0")
        self.assertEqual(cache.namespace, "team")
        self.assertTrue(only_db)

        memory = MemoryConfig(max_size=12)
        cache, only_db = StoreSetupManager._normalize_cache_options(
            cache=memory,
            cache_mode="local",
            only_db=True,
        )
        self.assertIs(cache, memory)
        self.assertEqual(cache.max_size, 12)
        self.assertFalse(only_db)

        cache, only_db = StoreSetupManager._normalize_cache_options(
            cache=None,
            cache_mode="auto",
            only_db=False,
        )
        self.assertIsNone(cache)
        self.assertFalse(only_db)

        with self.assertRaisesRegex(ValueError, "cache_mode='shared'.*RedisConfig"):
            StoreSetupManager._normalize_cache_options(
                cache=None,
                cache_mode="shared",
                only_db=False,
            )
        with self.assertRaisesRegex(ValueError, "memory 后端无法跨进程共享 session"):
            StoreSetupManager._normalize_cache_options(
                cache=memory,
                cache_mode="shared",
                only_db=False,
            )

        with self.assertRaisesRegex(ValueError, "cache_mode='hybrid'"):
            StoreSetupManager._normalize_cache_options(
                cache=redis,
                cache_mode="hybrid",
                only_db=False,
            )

    def test_setup_store_rejects_mcp_config_file_alias(self):
        from mcpstore.core.store.setup_manager import StoreSetupManager

        with self.assertRaisesRegex(ValueError, "mcp_config_file"):
            StoreSetupManager.setup_store(mcp_config_file=Path("alias.json"))

    def test_setup_store_rejects_kwargs_aliases(self):
        from mcpstore.config import MemoryConfig
        from mcpstore.core.store.setup_manager import StoreSetupManager

        cache = MemoryConfig()
        with self.assertRaisesRegex(ValueError, "config_path"):
            StoreSetupManager.setup_store(config_path=Path("config-alias.json"))
        with self.assertRaisesRegex(ValueError, "cache_config"):
            StoreSetupManager.setup_store(cache_config=cache)
        with self.assertRaisesRegex(ValueError, "external_db"):
            StoreSetupManager.setup_store(external_db={"cache": {"type": "memory"}})

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

    def test_redis_config_rejects_python_client_runtime(self):
        from mcpstore.config import RedisConfig
        from mcpstore.core.store.rust_backend import RustStoreBackend

        with self.assertRaises(TypeError):
            RedisConfig(client=object())

        config = RedisConfig(
            host="redis.local",
            port=6380,
            db=2,
            password="p@ss word",
            namespace="team",
        )
        backend, redis_url, namespace = RustStoreBackend._cache_options(config)

        self.assertEqual(backend, "redis")
        self.assertEqual(redis_url, "redis://:p%40ss%20word@redis.local:6380/2")
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
                    )

        self.assertEqual(code, 0)
        cmd = run.call_args.args[0]
        self.assertEqual(cmd[:6], ["/bin/mcpstore", "api", "--host", "0.0.0.0", "--port", "18200"])
        self.assertIn("--url-prefix", cmd)
        self.assertIn("--config-path", cmd)
        self.assertIn("--source", cmd)
        self.assertIn("--backend", cmd)

    def test_server_launch_rejects_unsupported_python_only_options(self):
        from mcpstore.core.store.rust_backend import RustStoreBackend

        store = RustStoreBackend(object())

        with self.assertRaisesRegex(ValueError, "show_startup_info"):
            store.start_api_server(show_startup_info=False)
        with self.assertRaisesRegex(ValueError, "log_level"):
            store.start_api_server(log_level="debug")
        with self.assertRaisesRegex(ValueError, "legacy"):
            store.start_api_server(legacy=True)
        with self.assertRaisesRegex(ValueError, "legacy"):
            store.for_store().hub_http(legacy=True)

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

    def test_hub_sse_is_not_faked_with_streamable_http(self):
        from mcpstore.core.store.rust_backend import RustStoreBackend

        store = RustStoreBackend(object())
        with self.assertRaisesRegex(NotImplementedError, "hub_http"):
            store.for_store().hub_sse(
                host="0.0.0.0",
                port=18081,
                path="/sse",
                block=True,
            )

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

    def test_switch_cache_uses_rust_runtime_migration(self):
        from mcpstore.config import RedisConfig
        from mcpstore.core.store.rust_backend import RustStoreBackend

        class FakeRustInner:
            def __init__(self):
                self.switched = None

            def switch_cache_storage(self, backend, redis_url, namespace):
                self.switched = (backend, redis_url, namespace)
                return {"entities": {}, "relations": {}, "states": {}, "events": {}}

        inner = FakeRustInner()
        store = RustStoreBackend(inner)
        store._sessions[("store", "s1")] = object()
        store._active_sessions["store"] = object()
        store._auto_sessions["store"] = object()

        context = store.for_store()
        self.assertIs(
            context.switch_cache(RedisConfig(url="redis://localhost:6379/0", namespace="switched")),
            context,
        )

        self.assertEqual(
            inner.switched,
            ("redis", "redis://localhost:6379/0", "switched"),
        )
        self.assertEqual(store._sessions, {})
        self.assertEqual(store._active_sessions, {})
        self.assertEqual(store._auto_sessions, {})

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
        self.assertFalse(stats.request_metrics_available)
        self.assertIsNone(stats.total_requests)
        self.assertIsNone(stats.hit_rate)
        self.assertEqual(stats.entity_count, 1)
        self.assertEqual(stats.state_count, 1)
        self.assertTrue(asyncio.run(store.registry.clear_all()))
        self.assertTrue(store._inner.reset)
        with self.assertRaises(NotImplementedError):
            asyncio.run(store.registry.reset_cache_statistics())
        self.assertTrue(asyncio.run(store.registry.switch_backend(MemoryConfig())))
        self.assertEqual(switched[0].cache_type.value, "memory")

        class ExternalRedisBackend:
            host = "redis.local"
            port = 6380
            db = 2
            password = "secret"
            namespace = "registry"

        self.assertTrue(asyncio.run(store.registry.switch_backend(ExternalRedisBackend())))
        self.assertEqual(switched[1].cache_type.value, "redis")
        self.assertEqual(switched[1].host, "redis.local")
        self.assertEqual(switched[1].port, 6380)
        self.assertEqual(switched[1].db, 2)
        self.assertEqual(switched[1].password, "secret")
        self.assertEqual(switched[1].namespace, "registry")

        class ExternalRedisWithoutAddress:
            pass

        with self.assertRaisesRegex(ValueError, "缺少 url 或 host"):
            asyncio.run(store.registry.switch_backend(ExternalRedisWithoutAddress()))

    def test_rust_backed_public_api_modules_import(self):
        from mcpstore.api.api_dependencies import get_store
        from mcpstore.config import MemoryConfig
        from mcpstore.config.namespace import get_namespace
        from mcpstore.core.models import ErrorCode, ResponseBuilder, timed_response

        store = object()
        if importlib.util.find_spec("fastapi") is not None:
            from mcpstore.api.api_pack import api_agent_router, api_main_router, api_set_store

            self.assertIsNotNone(api_main_router)
            self.assertIsNotNone(api_agent_router)
            paths = {route.path for route in api_main_router.routes}
            self.assertNotIn("/for_store/update_config/{service_name}", paths)
            self.assertNotIn("/for_store/delete_config/{service_name}", paths)
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
                    "/for_store/update_service/{service_name}",
                    "/for_store/patch_service/{service_name}",
                    "/for_store/remove_service/{service_name}",
                    "/for_store/list_resources",
                    "/for_store/list_resource_templates",
                    "/for_store/read_resource",
                    "/for_store/list_prompts",
                    "/for_store/get_prompt",
                    "/for_store/wait_service",
                    "/for_store/wait_service/{service_name}",
                    "/for_store/connect_service",
                    "/for_store/connect_service/{service_name}",
                    "/for_store/restart_service/{service_name}",
                    "/for_store/disconnect_service/{service_name}",
                    "/for_agent/{agent_id}/list_services",
                    "/for_agent/{agent_id}/add_service",
                    "/for_agent/{agent_id}/list_resources",
                    "/for_agent/{agent_id}/list_resource_templates",
                    "/for_agent/{agent_id}/read_resource",
                    "/for_agent/{agent_id}/list_prompts",
                    "/for_agent/{agent_id}/get_prompt",
                    "/for_agent/{agent_id}/show_config",
                    "/for_agent/{agent_id}/wait_service",
                    "/for_agent/{agent_id}/wait_service/{service_name}",
                    "/for_agent/{agent_id}/patch_service/{service_name}",
                    "/for_agent/{agent_id}/update_service/{service_name}",
                    "/for_agent/{agent_id}/remove_service/{service_name}",
                    "/for_agent/{agent_id}/restart_service",
                    "/for_agent/{agent_id}/restart_service/{service_name}",
                    "/for_agent/{agent_id}/connect_service",
                    "/for_agent/{agent_id}/connect_service/{service_name}",
                    "/for_agent/{agent_id}/disconnect_service",
                    "/for_agent/{agent_id}/disconnect_service/{service_name}",
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
        self.assertIsNone(importlib.util.find_spec("mcpstore.config.factory"))
        self.assertEqual(ErrorCode.MISSING_PARAMETER.value, "missing_parameter")
        self.assertTrue(ResponseBuilder.success()["success"])
        self.assertFalse(ResponseBuilder.error()["success"])

        @timed_response
        def handler():
            return ResponseBuilder.success()

        self.assertIn("elapsed_ms", handler()["meta"])

    def test_python_config_compatibility_exports_are_data_only(self):
        from mcpstore.config import (
            DataSourceStrategy,
            MemoryConfig,
            RedisConfig,
            detect_strategy,
            get_namespace,
            load_app_config,
        )

        redis = RedisConfig(url="redis://localhost:6379/0", namespace="team")
        self.assertEqual(get_namespace(redis), "team")
        self.assertEqual(get_namespace(MemoryConfig()), "mcpstore")
        self.assertEqual(
            detect_strategy(redis, "mcp.json"),
            DataSourceStrategy.LOCAL_DB,
        )
        self.assertEqual(
            detect_strategy(redis, None, only_db=True),
            DataSourceStrategy.ONLY_DB,
        )

        app_config = load_app_config()
        self.assertEqual(app_config["streamable_http_endpoint"], "/mcp")
        self.assertIn("heartbeat_interval", app_config)

    def test_top_level_python_sdk_compatibility_exports(self):
        from mcpstore import (
            APIResponse,
            ClientIDGenerator,
            ErrorCode,
            ErrorDetail,
            MCPStoreException,
            Pagination,
            ResponseBuilder,
            ResponseMeta,
            ServiceConnectionState,
            ServiceInfo,
            ServiceNotFoundException,
            ToolExecutionError,
            ToolExecutionRequest,
            ToolInfo,
            ValidationException,
            timed_response,
        )

        service = ServiceInfo(
            name="weather",
            transport_type="stdio",
            status=ServiceConnectionState.READY,
            tool_count=1,
            keep_alive=True,
            command="python",
            args=["-m", "weather"],
            config={"command": "python"},
        )
        self.assertEqual(service.name, "weather")
        self.assertEqual(service.transport_type.value, "stdio")

        tool = ToolInfo(
            name="weather_get",
            tool_original_name="get",
            service_original_name="weather",
            service_global_name="weather",
            service_name="weather",
            description="Get weather",
            inputSchema={"type": "object"},
        )
        self.assertEqual(tool.tool_original_name, "get")

        request = ToolExecutionRequest(tool_name="get", service_name="weather", args={"city": "SZ"})
        self.assertEqual(request.args["city"], "SZ")

        meta = ResponseMeta(timestamp="2026-06-27T00:00:00Z", request_id="req_1234567890123456", execution_time_ms=3)
        pagination = Pagination(page=1, page_size=10, total=1, total_pages=1, has_next=False, has_prev=False)
        response = APIResponse(success=False, message="failed", errors=[ErrorDetail(code="X", message="x")], meta=meta, pagination=pagination)
        self.assertFalse(response.success)
        self.assertEqual(response.errors[0].code, "X")

        self.assertEqual(ErrorCode.MISSING_PARAMETER.value, "missing_parameter")
        self.assertIsInstance(ResponseBuilder.success(message="ok"), dict)
        self.assertFalse(ResponseBuilder.error(code=ErrorCode.MISSING_PARAMETER)["success"])

        @timed_response
        def handler():
            return ResponseBuilder.success()

        self.assertIn("elapsed_ms", handler()["meta"])

        client_id = ClientIDGenerator.generate_deterministic_id(
            "store",
            "weather",
            {"command": "python"},
            "store",
        )
        self.assertTrue(ClientIDGenerator.is_deterministic_format(client_id))
        self.assertEqual(ClientIDGenerator.parse_client_id(client_id)["type"], "store")

        self.assertIsInstance(MCPStoreException("boom").to_dict()["error_id"], str)
        self.assertEqual(ServiceNotFoundException("weather").to_dict()["field"], "service_name")
        self.assertEqual(ToolExecutionError("get", "bad").to_dict()["details"]["reason"], "bad")
        self.assertEqual(ValidationException("bad", field="name").to_dict()["field"], "name")

    def test_python_context_compatibility_exports_are_rust_backed(self):
        import asyncio

        from mcpstore import MCPStoreContext, Session, SessionContext
        from mcpstore.core.context import (
            AgentServiceMapper,
            AgentStatisticsMixin,
            AgentProxy,
            AdvancedFeaturesMixin,
            AsyncSafeServiceManagement,
            AsyncSafeServiceManagementFactory,
            AddServiceWaitStrategy,
            CacheProxy,
            CallToolResultProtocol,
            ContextType,
            MCPStoreContext as ContextMCPStoreContext,
            ResourcesPromptsMixin,
            ServiceOperationsMixin,
            ServiceProxy,
            Session as ContextSession,
            SessionContext as ContextSessionContext,
            SessionManagementMixin,
            StoreProxy,
            ToolCallResult,
            ToolOperationsMixin,
            ToolProxy,
            ToolTransformConfig,
            ToolTransformer,
            ArgumentTransform,
            UpdateServiceAuthHelper,
        )
        from mcpstore.core.context.advanced_features import AdvancedFeaturesMixin as AdvancedFeaturesModuleExport
        from mcpstore.core.context.agent_service_mapper import AgentServiceMapper as MapperModuleExport
        from mcpstore.core.context.agent_statistics import AgentStatisticsMixin as AgentStatisticsModuleExport
        from mcpstore.core.context.agent_proxy import AgentProxy as AgentProxyModuleExport
        from mcpstore.core.context.async_safe_service_management import AsyncSafeServiceManagement as AsyncSafeModuleExport
        from mcpstore.core.context.base_context import MCPStoreContext as BaseContextExport
        from mcpstore.core.context.cache_proxy import CacheProxy as CacheProxyModuleExport
        from mcpstore.core.context.resources_prompts import ResourcesPromptsMixin as ResourcesPromptsModuleExport
        from mcpstore.core.context.service_operations import AddServiceWaitStrategy as WaitStrategyModuleExport
        from mcpstore.core.context.service_operations import ServiceOperationsMixin as ServiceOperationsModuleExport
        from mcpstore.core.context.service_management import UpdateServiceAuthHelper as AuthHelperModuleExport
        from mcpstore.core.context.service_proxy import ServiceProxy as ServiceProxyModuleExport
        from mcpstore.core.context.session import Session as SessionModuleExport
        from mcpstore.core.context.session_management import SessionManagementMixin as SessionManagementModuleExport
        from mcpstore.core.context.store_proxy import StoreProxy as StoreProxyModuleExport
        from mcpstore.core.context.tool_operations import ToolOperationsMixin as ToolOperationsModuleExport
        from mcpstore.core.context.tool_proxy_annotations import CallToolResultProtocol as ToolProtocolModuleExport
        from mcpstore.core.context.tool_proxy import ToolCallResult as ToolCallResultModuleExport
        from mcpstore.core.context.tool_proxy import ToolProxy as ToolProxyModuleExport
        from mcpstore.core.context.tool_transformation import ToolTransformer as ToolTransformerModuleExport
        from mcpstore.core.context.types import ContextType as ContextTypeModuleExport
        from mcpstore.core.store import (
            CacheProxy as StoreCacheProxy,
            MCPStoreContext as StoreMCPStoreContext,
            RustCacheProxy,
            RustServiceProxy,
            RustSession,
            RustStoreBackend,
            RustStoreContext,
            RustToolProxy,
            ServiceProxy as StoreServiceProxy,
            Session as StoreSession,
            SessionContext as StoreSessionContext,
            ToolProxy as StoreToolProxy,
        )

        self.assertIs(MCPStoreContext, RustStoreContext)
        self.assertIs(Session, RustSession)
        self.assertIs(SessionContext, RustSession)
        self.assertIs(ContextMCPStoreContext, RustStoreContext)
        self.assertIs(ContextSession, RustSession)
        self.assertIs(ContextSessionContext, RustSession)
        self.assertIs(ServiceProxy, RustServiceProxy)
        self.assertIs(ToolProxy, RustToolProxy)
        self.assertIs(CacheProxy, RustCacheProxy)
        self.assertIs(BaseContextExport, RustStoreContext)
        self.assertIs(SessionModuleExport, RustSession)
        self.assertIs(ServiceProxyModuleExport, RustServiceProxy)
        self.assertIs(ToolProxyModuleExport, RustToolProxy)
        self.assertIs(CacheProxyModuleExport, RustCacheProxy)
        self.assertIs(MapperModuleExport, AgentServiceMapper)
        self.assertIs(StoreProxyModuleExport, StoreProxy)
        self.assertIs(AgentProxyModuleExport, AgentProxy)
        self.assertIs(AgentStatisticsModuleExport, AgentStatisticsMixin)
        self.assertIs(AdvancedFeaturesModuleExport, AdvancedFeaturesMixin)
        self.assertIs(AsyncSafeModuleExport, AsyncSafeServiceManagement)
        self.assertIs(WaitStrategyModuleExport, AddServiceWaitStrategy)
        self.assertIs(ServiceOperationsModuleExport, ServiceOperationsMixin)
        self.assertIs(ToolOperationsModuleExport, ToolOperationsMixin)
        self.assertIs(SessionManagementModuleExport, SessionManagementMixin)
        self.assertIs(ToolProtocolModuleExport, CallToolResultProtocol)
        self.assertIsNotNone(AsyncSafeServiceManagementFactory)
        self.assertIs(ResourcesPromptsModuleExport, ResourcesPromptsMixin)
        self.assertIs(AuthHelperModuleExport, UpdateServiceAuthHelper)
        self.assertIs(ToolCallResultModuleExport, ToolCallResult)
        self.assertIs(ToolTransformerModuleExport, ToolTransformer)
        self.assertIs(ContextTypeModuleExport, ContextType)
        self.assertIs(StoreMCPStoreContext, RustStoreContext)
        self.assertIs(StoreSession, RustSession)
        self.assertIs(StoreSessionContext, RustSession)
        self.assertIs(StoreServiceProxy, RustServiceProxy)
        self.assertIs(StoreToolProxy, RustToolProxy)
        self.assertIs(StoreCacheProxy, RustCacheProxy)

        class FakeBackend:
            def __init__(self):
                self.sessions = {}
                self.state = {}
                self.transforms = {}

            def find_session(self, session_id, scope, agent_id):
                return self.sessions.get((scope, agent_id, session_id))

            def create_session(self, session_id, scope, agent_id, lease_seconds, metadata):
                entity = {
                    "session_id": session_id,
                    "session_key": f"{scope}:{agent_id or 'store'}:{session_id}",
                    "scope": scope,
                    "agent_id": agent_id,
                    "metadata": metadata,
                    "lease_seconds": lease_seconds or 3600,
                }
                self.sessions[(scope, agent_id, session_id)] = entity
                return entity

            def get_session_status(self, session_key):
                return {"status": "active"}

            def get_session(self, session_key):
                for entity in self.sessions.values():
                    if entity["session_key"] == session_key:
                        return entity
                return None

            def find_session_by_user_session_id(self, user_session_id):
                for entity in self.sessions.values():
                    if entity["metadata"].get("user_session_id") == user_session_id:
                        return entity
                return None

            def update_session_metadata(self, session_key, metadata):
                entity = self.get_session(session_key)
                if entity is None:
                    return None
                entity["metadata"] = metadata
                return entity

            def set_session_state(self, session_key, key, value):
                record = self.state.setdefault(
                    session_key,
                    {"session_key": session_key, "values": {}, "version": 0},
                )
                record["values"][key] = value
                record["version"] += 1
                return record

            def get_session_state_value(self, session_key, key):
                return self.state.get(session_key, {"values": {}})["values"].get(key)

            def list_session_state(self, session_key):
                return self.state.get(session_key, {"session_key": session_key, "values": {}, "version": 0})

            def clear_session_state(self, session_key):
                record = self.state.setdefault(
                    session_key,
                    {"session_key": session_key, "values": {}, "version": 0},
                )
                record["values"] = {}
                record["version"] += 1
                return record

            def resolve_service_name_for_agent(self, agent_id, service_name):
                return service_name

            def set_tool_transform(self, service_name, tool_name, transform):
                key = (service_name, tool_name)
                record = {
                    "tool_global_name": f"{service_name}_{tool_name}",
                    "service_global_name": service_name,
                    "original_tool_name": tool_name,
                    **transform,
                    "updated_at": 1,
                    "version": 1,
                }
                self.transforms[key] = record
                return record

            def get_tool_transform(self, service_name, tool_name):
                return self.transforms.get((service_name, tool_name))

            def list_tool_transforms(self):
                return list(self.transforms.values())

            def list_resources_scoped(self, agent_id, service_name=None):
                return [{"uri": "memory://doc", "service_name": service_name, "agent_id": agent_id}]

            def list_resource_templates_scoped(self, agent_id, service_name=None):
                return [{"uriTemplate": "memory://{name}", "service_name": service_name, "agent_id": agent_id}]

            def read_resource_scoped(self, agent_id, uri, service_name=None):
                return {"uri": uri, "text": "body", "service_name": service_name, "agent_id": agent_id}

            def list_prompts_scoped(self, agent_id, service_name=None):
                return [{"name": "summarize", "service_name": service_name, "agent_id": agent_id}]

            def get_prompt_scoped(self, agent_id, prompt_name, arguments, service_name=None):
                return {
                    "name": prompt_name,
                    "arguments": arguments,
                    "service_name": service_name,
                    "agent_id": agent_id,
                }

            def list_sessions(self, scope, agent_id):
                return [
                    entity
                    for (stored_scope, stored_agent_id, _), entity in self.sessions.items()
                    if stored_scope == scope and stored_agent_id == agent_id
                ]

            def list_services_scoped(self, agent_id=None):
                return [{"name": "svc", "transport": "stdio", "agent_id": agent_id}]

            def list_tools_scoped(self, agent_id=None, service_name=None):
                return [{"name": "echo", "service_name": service_name or "svc", "agent_id": agent_id}]

            def list_changed_tools_scoped(self, agent_id=None, service_name=None, *, force_refresh=False):
                return {
                    "changed": True,
                    "services": [service_name or "svc"],
                    "trigger": "manual_force" if force_refresh else "manual",
                    "timestamp": 1,
                    "details": {
                        "service_results": [
                            {
                                "service_name": service_name or "svc",
                                "client_id": service_name or "svc",
                                "added_tools": ["echo"],
                                "removed_tools": [],
                                "updated_tools": [],
                                "changes_count": 1,
                                "changed": True,
                                "agent_id": agent_id,
                            }
                        ]
                    },
                }

            def check_services_scoped(self, agent_id=None):
                return {"svc": "connected"}

            def find_service(self, name):
                return {"name": name, "transport": "stdio"}

            def service_status_scoped(self, agent_id, service_name):
                return {"status": "connected", "service_name": service_name, "agent_id": agent_id}

            def wait_service_ready(self, name, timeout=10.0):
                return {"service_global_name": name, "health_status": "ready"}

            def list_agents(self):
                return [{"agent_id": "agent_a"}]

            def check_services_scoped(self, agent_id=None):
                return {"svc": "connected"}

            def event_history(self, count=100):
                return []

            def namespace(self):
                return "test"

            def current_backend(self):
                return "memory"

            def resolve_tool_for_agent(self, agent_id, user_input):
                return {"global_service_name": "svc", "canonical_tool_name": "echo"}

            def call_tool(self, service_name, tool_name, args):
                return {"content": [{"type": "text", "text": args["text"]}], "is_error": False}

            def import_openapi_service(self, name, spec_url, options=None):
                return {
                    "service_name": name,
                    "spec_url": spec_url,
                    "base_url": "https://example.test",
                    "spec_info": {"title": "Example", "version": "1.0", "description": None},
                    "security_schemes": {},
                    "security": [],
                    "components": [{"name": "listItems", "type": "resource", "service_name": name}],
                    "total_endpoints": 1,
                    "component_types": {"tools": 0, "resources": 1, "resource_templates": 0},
                    "runtime_executable": True,
                    "options": options or {},
                }

            def import_openapi_service_from_spec(self, name, spec_url, spec, options=None):
                return self.import_openapi_service(name, spec_url, options)

            def import_openapi_service_from_spec_text(self, name, spec_url, spec_text, options=None):
                return self.import_openapi_service(name, spec_url, options)

            def bundle_openapi_spec(self, spec_url, options=None):
                return {"spec_url": spec_url, "options": options or {}}

            def bundle_openapi_spec_from_spec(self, spec_url, spec, options=None):
                return {"spec_url": spec_url, "spec": spec, "options": options or {}}

            def bundle_openapi_artifact(self, spec_url, options=None):
                return {"spec_url": spec_url, "options": options or {}, "bundle": {}}

            def bundle_openapi_artifact_from_spec(self, spec_url, spec, options=None):
                return {"spec_url": spec_url, "spec": spec, "options": options or {}, "bundle": {}}

            def get_openapi_import(self, name):
                return None

            def list_openapi_imports(self):
                return []

        backend = RustStoreBackend(FakeBackend())
        context = MCPStoreContext(backend)
        store_proxy = StoreProxy(context)
        agent_proxy = AgentProxy(context, "agent_a")

        self.assertIs(store_proxy.get_context(), context)
        self.assertEqual(store_proxy.get_id(), "global_agent_store")
        self.assertEqual(store_proxy.list_prompts("svc")[0].name, "summarize")
        self.assertEqual(store_proxy.find_agent("agent_a").get_id(), "agent_a")
        self.assertEqual(agent_proxy.get_context().agent_id, "agent_a")
        self.assertEqual(agent_proxy.get_id(), "agent_a")
        self.assertEqual(agent_proxy.map_global("svc"), "svc_byagent_agent_a")
        self.assertEqual(agent_proxy.map_local("svc_byagent_agent_a"), "svc")

        class ContextOperationsCompat(
            ServiceOperationsMixin,
            ToolOperationsMixin,
            SessionManagementMixin,
            AdvancedFeaturesMixin,
            AgentStatisticsMixin,
        ):
            def __init__(self, rust_context):
                self._context = rust_context

        operations = ContextOperationsCompat(context)
        wait_strategy = AddServiceWaitStrategy()
        self.assertEqual(wait_strategy.parse_wait_parameter("2500"), 2.5)
        self.assertEqual(wait_strategy.get_service_wait_timeout({"url": "https://example.test"}), 2.0)
        self.assertEqual(operations.list_services()[0].name, "svc")
        self.assertEqual(operations.list_tools()[0].name, "echo")
        self.assertEqual(operations.call_tool("echo", {"text": "mix"}).text_output, "mix")
        self.assertEqual(operations.wait_service("svc").health_status, "ready")
        self.assertEqual(operations.get_agents_summary().total_agents, 1)
        self.assertEqual(operations.create_shared_session("shared", "user-1").metadata.user_session_id, "user-1")
        self.assertEqual(operations.find_user_session("user-1").session_id, "shared")
        self.assertTrue(operations.register_session_globally("shared", "user-2"))
        self.assertEqual(operations.find_user_session("user-2").session_id, "shared")
        self.assertIs(operations.import_api("https://example.test/openapi.json", "example"), operations)
        self.assertEqual(operations.last_openapi_import().service_name, "example")
        self.assertTrue(operations.last_openapi_import().runtime_executable)
        self.assertIs(
            operations.import_api(
                "https://example.test/secure-openapi.json",
                "secure",
                auth={"ApiKeyAuth": "secret"},
            ),
            operations,
        )
        self.assertEqual(
            operations.last_openapi_import().options["auth"]["ApiKeyAuth"],
            "secret",
        )
        self.assertIs(
            operations.import_api(
                "https://example.test/cache-openapi.json",
                "cache",
                ref_cache={"ttl_seconds": 21},
            ),
            operations,
        )
        self.assertEqual(
            operations.last_openapi_import().options["ref_cache"]["ttl_seconds"],
            21,
        )
        self.assertEqual(
            store_proxy.bundle_openapi_artifact(
                "memory://bundle",
                ref_cache={"enabled": False},
            )["options"]["ref_cache"]["enabled"],
            False,
        )

        session = context.with_session("compat")

        self.assertIsInstance(session, Session)
        self.assertIsInstance(session, ContextSession)
        self.assertIs(session.set_state("cursor", {"page": 7}), session)
        self.assertEqual(session.get_state("cursor").page, 7)
        self.assertEqual(session.state_values().cursor.page, 7)
        self.assertTrue(session.clear_cache())
        self.assertEqual(session.state_values(), {})

        mapper = AgentServiceMapper("agent_a")
        self.assertEqual(mapper.to_global_name("svc"), "svc_byagent_agent_a")
        self.assertEqual(mapper.to_local_name("svc_byagent_agent_a"), "svc")
        self.assertEqual(AgentServiceMapper.parse_agent_service_name("svc_byagent_agent_a"), ("agent_a", "svc"))
        self.assertEqual(ContextType.STORE.value, "store")

        class ResourcePromptCompat(ResourcesPromptsMixin):
            def __init__(self, rust_context):
                self._context = rust_context

        resource_prompt = ResourcePromptCompat(context)
        self.assertEqual(resource_prompt.list_resources("svc")[0].uri, "memory://doc")
        self.assertEqual(
            resource_prompt.list_resource_templates("svc")[0].uriTemplate,
            "memory://{name}",
        )
        self.assertEqual(resource_prompt.read_resource("memory://doc", "svc").text, "body")
        self.assertEqual(resource_prompt.list_prompts("svc")[0].name, "summarize")
        self.assertEqual(resource_prompt.get_prompt("summarize", {"topic": "rust"}, "svc").arguments.topic, "rust")
        self.assertEqual(asyncio.run(resource_prompt.list_prompts_async("svc"))[0].name, "summarize")
        changed_tools = resource_prompt.list_changed_tools("svc", force_refresh=True)
        self.assertTrue(changed_tools.changed)
        self.assertEqual(changed_tools.trigger, "manual_force")
        self.assertEqual(changed_tools.details.service_results[0].added_tools, ["echo"])

        result = ToolCallResult(
            {"content": [{"type": "text", "text": "ok"}], "is_error": False},
            "echo",
            {"text": "ok"},
        )
        self.assertEqual(result.text_output, "ok")
        self.assertFalse(result.to_dict()["is_error"])

        transformer = ToolTransformer(context, service_name="svc")
        transformed_name = transformer.create_parameter_renamed_tool(
            "echo",
            {"text": "message"},
            new_tool_name="say",
        )
        self.assertEqual(transformed_name, "say")
        rule = context.get_tool_transform("svc", "echo")
        self.assertEqual(rule.display_name, "say")
        self.assertEqual(rule.arguments[0].original_name, "text")
        self.assertEqual(rule.arguments[0].new_name, "message")
        config = ToolTransformConfig(
            original_tool_name="echo",
            new_tool_name="hidden_echo",
            argument_transforms={
                "debug": ArgumentTransform("debug", hidden=True, default_value=False)
            },
        )
        self.assertEqual(transformer.register_transformation(config), "hidden_echo")
        with self.assertRaisesRegex(NotImplementedError, "Callable"):
            ToolTransformConfig(
                original_tool_name="echo",
                pre_execution_hooks=[lambda name, args: args],
            ).to_rust_payload()

    def test_top_level_adapter_import_errors_are_not_silenced(self):
        import builtins
        import mcpstore

        original_import = builtins.__import__

        def fail_langchain(name, *args, **kwargs):
            if name == "mcpstore.adapters.langchain_adapter":
                raise ImportError("langchain unavailable")
            return original_import(name, *args, **kwargs)

        with patch("builtins.__import__", side_effect=fail_langchain):
            with self.assertRaisesRegex(ImportError, "langchain unavailable"):
                mcpstore.__getattr__("LangChainAdapter")

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

    def test_api_sync_status_reports_rust_event_capability_only(self):
        if importlib.util.find_spec("fastapi") is None:
            self.skipTest("fastapi is not installed")

        import asyncio

        from mcpstore.api.api_pack import api_set_store, store_sync_status

        class FakeContext:
            def __init__(self):
                self.called = False

            async def event_capability_report_async(self):
                self.called = True
                return {"event_bus": True, "history": True}

        class FakeStore:
            def __init__(self):
                self.context = FakeContext()

            def for_store(self):
                return self.context

        store = FakeStore()
        api_set_store(store)
        result = asyncio.run(store_sync_status())

        self.assertTrue(result["success"])
        self.assertTrue(store.context.called)
        self.assertEqual(result["data"]["source"], "rust_event_capability")
        self.assertEqual(result["data"]["event_capability"]["event_bus"], True)
        self.assertNotIn("is_running", result["data"])

    def test_api_agent_show_config_delegates_to_agent_context(self):
        if importlib.util.find_spec("fastapi") is None:
            self.skipTest("fastapi is not installed")

        import asyncio

        from mcpstore.api.api_pack import agent_show_config, api_set_store

        class FakeContext:
            def __init__(self, agent_id):
                self.agent_id = agent_id
                self.scope = None

            async def show_config_async(self, scope="all"):
                self.scope = scope
                return {"agents": {self.agent_id: ["demo"]}}

        class FakeStore:
            def __init__(self):
                self.contexts = {}

            def for_agent(self, agent_id):
                context = FakeContext(agent_id)
                self.contexts[agent_id] = context
                return context

        store = FakeStore()
        api_set_store(store)
        result = asyncio.run(agent_show_config("agent-a", scope="mcp"))

        self.assertTrue(result["success"])
        self.assertEqual(store.contexts["agent-a"].scope, "mcp")
        self.assertEqual(result["data"], {"agents": {"agent-a": ["demo"]}})

    def test_api_service_management_routes_delegate_to_context(self):
        if importlib.util.find_spec("fastapi") is None:
            self.skipTest("fastapi is not installed")

        import asyncio

        from mcpstore.api.api_pack import (
            agent_connect_service,
            agent_connect_service_by_name,
            agent_disconnect_service,
            agent_disconnect_service_by_name,
            agent_patch_service,
            agent_restart_service,
            agent_restart_service_by_name,
            agent_wait_service_by_name,
            api_set_store,
            store_connect_service,
            store_connect_service_by_name,
            store_patch_service,
            store_restart_service_by_name,
            store_wait_service_by_name,
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

            async def connect_service_async(self, service_name):
                self.calls.append(("connect", service_name))
                return True

            async def disconnect_service_async(self, service_name):
                self.calls.append(("disconnect", service_name))
                return True

            async def wait_service_async(self, service_name, status=None, timeout=10.0):
                self.calls.append(("wait", service_name, status, timeout))
                return {"service_name": service_name, "health_status": "ready"}

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

        result = asyncio.run(store_connect_service({"service_name": "demo"}))
        self.assertTrue(result["success"])
        result = asyncio.run(store_connect_service_by_name("path-demo"))
        self.assertTrue(result["success"])
        result = asyncio.run(store_restart_service_by_name("path-demo"))
        self.assertTrue(result["success"])
        result = asyncio.run(store_wait_service_by_name("path-demo", status="healthy", timeout=2))
        self.assertTrue(result["success"])
        self.assertEqual(
            store.store_context.calls,
            [
                ("patch", "demo", {"timeout": 3}),
                ("connect", "demo"),
                ("connect", "path-demo"),
                ("restart", "path-demo"),
                ("wait", "path-demo", "healthy", 2),
            ],
        )

        result = asyncio.run(agent_patch_service("agent-a", "demo", {"timeout": 5}))
        self.assertTrue(result["success"])
        self.assertEqual(store.agent_contexts["agent-a"].calls, [("patch", "demo", {"timeout": 5})])

        result = asyncio.run(agent_restart_service("agent-b", {"service_name": "demo"}))
        self.assertTrue(result["success"])
        self.assertEqual(store.agent_contexts["agent-b"].calls, [("restart", "demo")])

        result = asyncio.run(agent_connect_service("agent-d", {"name": "demo"}))
        self.assertTrue(result["success"])
        self.assertEqual(store.agent_contexts["agent-d"].calls, [("connect", "demo")])

        result = asyncio.run(agent_connect_service_by_name("agent-e", "demo"))
        self.assertTrue(result["success"])
        self.assertEqual(store.agent_contexts["agent-e"].calls, [("connect", "demo")])

        result = asyncio.run(agent_restart_service_by_name("agent-f", "demo"))
        self.assertTrue(result["success"])
        self.assertEqual(store.agent_contexts["agent-f"].calls, [("restart", "demo")])

        result = asyncio.run(agent_disconnect_service_by_name("agent-g", "demo"))
        self.assertTrue(result["success"])
        self.assertEqual(store.agent_contexts["agent-g"].calls, [("disconnect", "demo")])

        result = asyncio.run(agent_wait_service_by_name("agent-h", "demo", status="healthy", timeout=3))
        self.assertTrue(result["success"])
        self.assertEqual(store.agent_contexts["agent-h"].calls, [("wait", "demo", "healthy", 3)])

        result = asyncio.run(agent_disconnect_service("agent-c", {"name": "demo"}))
        self.assertTrue(result["success"])
        self.assertEqual(store.agent_contexts["agent-c"].calls, [("disconnect", "demo")])

    def test_api_resources_and_prompts_delegate_to_context(self):
        if importlib.util.find_spec("fastapi") is None:
            self.skipTest("fastapi is not installed")

        import asyncio

        from mcpstore.api.api_pack import (
            agent_get_prompt,
            agent_list_prompts,
            agent_list_resource_templates,
            agent_list_resources,
            agent_read_resource,
            api_set_store,
            store_get_prompt,
            store_list_prompts,
            store_list_resource_templates,
            store_list_resources,
            store_read_resource,
        )

        class FakeContext:
            def __init__(self):
                self.calls = []

            async def list_resources_async(self, service_name=None):
                self.calls.append(("list_resources", service_name))
                return [{"uri": "memory://doc"}]

            async def list_resource_templates_async(self, service_name=None):
                self.calls.append(("list_resource_templates", service_name))
                return [{"uriTemplate": "memory://{name}"}]

            async def read_resource_async(self, uri, service_name=None):
                self.calls.append(("read_resource", uri, service_name))
                return {"contents": [{"uri": uri, "text": "body"}]}

            async def list_prompts_async(self, service_name=None):
                self.calls.append(("list_prompts", service_name))
                return [{"name": "summarize"}]

            async def get_prompt_async(self, prompt_name, arguments=None, service_name=None):
                self.calls.append(("get_prompt", prompt_name, arguments, service_name))
                return {"messages": [{"role": "user", "content": {"text": prompt_name}}]}

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

        result = asyncio.run(store_list_resources(service_name="svc"))
        self.assertTrue(result["success"])
        self.assertEqual(result["data"]["total"], 1)
        result = asyncio.run(store_list_resource_templates(service_name="svc"))
        self.assertTrue(result["success"])
        result = asyncio.run(store_read_resource(uri="memory://doc", service_name="svc"))
        self.assertTrue(result["success"])
        result = asyncio.run(store_list_prompts(service_name="svc"))
        self.assertTrue(result["success"])
        result = asyncio.run(
            store_get_prompt(
                {"prompt_name": "summarize", "arguments": {"topic": "rust"}, "service_name": "svc"}
            )
        )
        self.assertTrue(result["success"])
        self.assertEqual(
            store.store_context.calls,
            [
                ("list_resources", "svc"),
                ("list_resource_templates", "svc"),
                ("read_resource", "memory://doc", "svc"),
                ("list_prompts", "svc"),
                ("get_prompt", "summarize", {"topic": "rust"}, "svc"),
            ],
        )

        result = asyncio.run(agent_list_resources("agent-a", service_name="svc"))
        self.assertTrue(result["success"])
        result = asyncio.run(agent_list_resource_templates("agent-b", service_name="svc"))
        self.assertTrue(result["success"])
        result = asyncio.run(agent_read_resource("agent-c", uri="memory://doc", service_name="svc"))
        self.assertTrue(result["success"])
        result = asyncio.run(agent_list_prompts("agent-d", service_name="svc"))
        self.assertTrue(result["success"])
        result = asyncio.run(
            agent_get_prompt(
                "agent-e",
                {"name": "summarize", "args": {"topic": "agent"}, "service_name": "svc"},
            )
        )
        self.assertTrue(result["success"])
        self.assertEqual(store.agent_contexts["agent-a"].calls, [("list_resources", "svc")])
        self.assertEqual(
            store.agent_contexts["agent-b"].calls,
            [("list_resource_templates", "svc")],
        )
        self.assertEqual(
            store.agent_contexts["agent-c"].calls,
            [("read_resource", "memory://doc", "svc")],
        )
        self.assertEqual(store.agent_contexts["agent-d"].calls, [("list_prompts", "svc")])
        self.assertEqual(
            store.agent_contexts["agent-e"].calls,
            [("get_prompt", "summarize", {"topic": "agent"}, "svc")],
        )

    def test_api_tool_and_prompt_arguments_are_not_squashed(self):
        import asyncio
        import importlib
        import sys
        import types

        if importlib.util.find_spec("fastapi") is None:
            class FakeAPIRouter:
                def __init__(self, *args, **kwargs):
                    pass

                def get(self, *args, **kwargs):
                    return lambda func: func

                def post(self, *args, **kwargs):
                    return lambda func: func

                def put(self, *args, **kwargs):
                    return lambda func: func

                def patch(self, *args, **kwargs):
                    return lambda func: func

                def delete(self, *args, **kwargs):
                    return lambda func: func

                def include_router(self, *args, **kwargs):
                    return None

            fake_fastapi = types.ModuleType("fastapi")
            fake_fastapi.APIRouter = FakeAPIRouter
            fake_fastapi.Body = lambda default=None, **kwargs: default
            fake_fastapi.Query = lambda default=None, **kwargs: default
            with patch.dict(sys.modules, {"fastapi": fake_fastapi}):
                sys.modules.pop("mcpstore.api.api_pack", None)
                api_pack = importlib.import_module("mcpstore.api.api_pack")
        else:
            api_pack = importlib.import_module("mcpstore.api.api_pack")

        class FakeContext:
            def __init__(self):
                self.calls = []

            async def call_tool_async(self, tool_name, args=None):
                self.calls.append(("call_tool", tool_name, args))
                return {"ok": True}

            async def get_prompt_async(self, prompt_name, arguments=None, service_name=None):
                self.calls.append(("get_prompt", prompt_name, arguments, service_name))
                return {"ok": True}

        class FakeStore:
            def __init__(self):
                self.store_context = FakeContext()
                self.agent_contexts = {}

            def for_store(self):
                return self.store_context

            def for_agent(self, agent_id):
                if agent_id not in self.agent_contexts:
                    self.agent_contexts[agent_id] = FakeContext()
                return self.agent_contexts[agent_id]

        store = FakeStore()
        api_pack.api_set_store(store)

        result = asyncio.run(api_pack.store_call_tool({"tool_name": "echo", "args": ""}))
        self.assertTrue(result["success"])
        result = asyncio.run(
            api_pack.store_call_tool(
                {"tool_name": "echo", "arguments": {"topic": "rust"}}
            )
        )
        self.assertTrue(result["success"])
        result = asyncio.run(
            api_pack.store_get_prompt(
                {"prompt_name": "summarize", "arguments": '{"topic":"rust"}'}
            )
        )
        self.assertTrue(result["success"])
        self.assertEqual(
            store.store_context.calls,
            [
                ("call_tool", "echo", ""),
                ("call_tool", "echo", {"topic": "rust"}),
                ("get_prompt", "summarize", '{"topic":"rust"}', None),
            ],
        )

        result = asyncio.run(
            api_pack.agent_call_tool("agent-a", {"tool_name": "echo", "args": ""})
        )
        self.assertTrue(result["success"])
        result = asyncio.run(
            api_pack.agent_call_tool(
                "agent-a",
                {"tool_name": "echo", "arguments": {"topic": "agent-rust"}},
            )
        )
        self.assertTrue(result["success"])
        result = asyncio.run(api_pack.agent_get_prompt("agent-b", {"name": "summarize", "args": ""}))
        self.assertTrue(result["success"])
        self.assertEqual(
            store.agent_contexts["agent-a"].calls,
            [
                ("call_tool", "echo", ""),
                ("call_tool", "echo", {"topic": "agent-rust"}),
            ],
        )
        self.assertEqual(
            store.agent_contexts["agent-b"].calls,
            [("get_prompt", "summarize", "", None)],
        )

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
