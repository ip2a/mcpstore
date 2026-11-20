"""
Standard workflow + Redis + Async example

Demonstrates how to:
1. Initialize with `MCPStore.setup_store_async()` in an async environment;
2. Complete the standard workflow using the `*_async` APIs provided by the context.
"""

import asyncio

from example_utils import setup_example_import, ExampleConfig

setup_example_import()

from mcpstore import MCPStore  # noqa: E402
from mcpstore.config import RedisConfig  # noqa: E402


class AsyncMCPWorkflow:
    """
    Mirrors a real-world scenario: a class needs to hold MCPStore in __init__,
    but the instance is created inside an async context.
    An async factory (create) uses setup_store_async() so __init__ never blocks directly.
    """

    def __init__(self, store):
        self.store = store
        self.store_ctx = store.for_store()

    @classmethod
    async def create(cls):
        """Async factory ensuring safe initialization within the event loop."""
        redis_config = RedisConfig(
            host=ExampleConfig.REDIS_HOST,
            port=ExampleConfig.REDIS_PORT,
            password=ExampleConfig.REDIS_PASSWORD,
            namespace=ExampleConfig.REDIS_NAMESPACE,
        )
        store = await MCPStore.setup_store_async(debug=True, cache=redis_config)
        return cls(store)


async def main():
    print("\n" + "=" * 60)
    print("  MCPStore standard workflow (Redis + Async)")
    print("=" * 60)

    print("\n[Step 1] Create the business-class instance inside an async context")
    workflow = await AsyncMCPWorkflow.create()
    store_ctx = workflow.store_ctx
    print("  └─ ✓ Initialization completed via AsyncMCPWorkflow.create()")

    print("\n[Step 2] Reset the config and compare before/after")
    config_before_reset = await store_ctx.show_config_async()
    await store_ctx.reset_config_async()
    config_after_reset = await store_ctx.show_config_async()
    print("  ├─ Before reset: ", config_before_reset)
    print("  ├─ After reset: ", config_after_reset)
    print("  └─ ✓ Reset complete")

    print("\n[Step 3] Add the MCP service")
    service_name = "mcpstore"
    service_config = {
        "mcpServers": {
            service_name: {
                "url": "http://115.190.220.103:2230/mcp",
            }
        }
    }
    await store_ctx.add_service_async(service_config)
    print("  └─ ✓ Service added")

    print("\n[Step 4] Wait for the service to become ready")
    await store_ctx.wait_service_async(service_name)
    print("  └─ ✓ Service status: ready")

    print("\n[Step 5] List all services")
    services = await store_ctx.list_services_async()
    print(f"  ├─ Service count: {len(services)}")
    for svc in services:
        print(f"  ├─ {svc.name} / status={svc.status}")
    print("  └─ ✓ Service list retrieved")

    print("\n[Step 6] List all tools")
    tools = await store_ctx.list_tools_async()
    print(f"  ├─ Tool count: {len(tools)}")
    for tool in tools:
        print(f"  ├─ {tool.name} / service={tool.service_name}")
    print("  └─ ✓ Tool list retrieved")

    print("\n[Step 7] Call a tool (sync API wrapped as async)")
    tool_name = "mcpstore_wiki_byagent_demo_agent_http_get_mcpstore_docs"
    tool_result = await store_ctx.call_tool_async(tool_name, {})
    print("  ├─ Tool:", tool_name)
    print("  ├─ Is error:", tool_result.is_error)
    print("  └─ Content items:", len(tool_result.content))

    print("\n[Step 8] Wrap up and reset the config")
    await store_ctx.reset_config_async()
    print("  └─ ✓ Cleanup complete")

    print("\n" + "=" * 60)
    print("  Standard workflow example finished (Redis + Async)")
    print("=" * 60)


if __name__ == "__main__":
    asyncio.run(main())
