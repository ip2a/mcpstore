"""
Standard workflow + Redis + Async example.

Demonstrates how to:
1. Initialize MCPStore in an async environment with `MCPStore.setup_store_async()`.
2. Use the context `*_async` methods to run the standard workflow end-to-end.
"""

import asyncio
from pathlib import Path
import sys

EXAMPLE_DIR = Path(__file__).resolve().parent.parent
if str(EXAMPLE_DIR) not in sys.path:
    sys.path.insert(0, str(EXAMPLE_DIR))

from example_utils import setup_example_import, ExampleConfig

setup_example_import()

from mcpstore import MCPStore  # noqa: E402
from mcpstore.config import RedisConfig  # noqa: E402


class AsyncMCPWorkflow:
    """
    Simulate a real-world scenario:
    - A class needs to hold an MCPStore instance on __init__,
      but the instance itself must be created inside an async context.
    - We use an async factory (`create`) with `setup_store_async()` to avoid
      blocking work inside __init__.
    """

    def __init__(self, store):
        self.store = store
        self.store_ctx = store.for_store()

    @classmethod
    async def create(cls):
        """Async factory that safely initializes MCPStore inside the event loop."""
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
    print("  MCPStore Standard Workflow (Redis + Async)")
    print("=" * 60)

    print("\n[Step 1] Create workflow instance in async context")
    workflow = await AsyncMCPWorkflow.create()
    store_ctx = workflow.store_ctx
    print("  └─ ✓ Initialized via AsyncMCPWorkflow.create()")

    print("\n[Step 2] Reset configuration and compare before/after")
    config_before_reset = await store_ctx.show_config_async()
    await store_ctx.reset_config_async()
    config_after_reset = await store_ctx.show_config_async()
    print("  ├─ Config before reset:", config_before_reset)
    print("  ├─ Config after reset :", config_after_reset)
    print("  └─ ✓ Reset completed")

    print("\n[Step 3] Add MCP Service")
    service_name = "mcpstore"
    service_config = {
        "mcpServers": {
            service_name: {
                "url": "https://www.mcpstore.wiki/mcp",
            }
        }
    }
    await store_ctx.add_service_async(service_config)
    print("  └─ ✓ Service added successfully")

    print("\n[Step 4] Wait for Service Ready")
    await store_ctx.wait_service_async(service_name)
    print("  └─ ✓ Service status: ready")

    print("\n[Step 5] List All Services")
    services = await store_ctx.list_services_async()
    print(f"  ├─ Total Services: {len(services)}")
    for svc in services:
        print(f"  ├─ {svc.name} / status={svc.status}")
    print("  └─ ✓ Service list retrieved successfully")

    print("\n[Step 6] List All Tools")
    tools = await store_ctx.list_tools_async()
    print(f"  ├─ Total Tools: {len(tools)}")
    for tool in tools:
        print(f"  ├─ {tool.name} / service={tool.service_name}")
    print("  └─ ✓ Tool list retrieved successfully")

    print("\n[Step 7] Call Tool (sync API wrapped as async)")
    tool_name = "mcpstore_byagent_demo_agent_http_get_mcpstore_docs"
    tool_result = await store_ctx.call_tool_async(tool_name, {})
    print("  ├─ Tool:", tool_name)
    print("  ├─ Is Error:", tool_result.is_error)
    print("  └─ Content Items:", len(tool_result.content))

    print("\n[Step 8] Final cleanup and reset configuration")
    await store_ctx.reset_config_async()
    print("  └─ ✓ Cleanup completed")

    print("\n" + "=" * 60)
    print("  Standard Workflow Example Completed (Redis + Async)")
    print("=" * 60)


if __name__ == "__main__":
    asyncio.run(main())
