"""
Perspective switch + update example

Scenario:
- demo1 and demo2 are added under the Store (same URL).
- demo3 is added under Agent1.
- list is printed first to show the current per-perspective results (verification).
- Demonstrates how update/patch is lenient on input but canonicalizes on write, in both the Store and Agent perspectives:
  - Store: update the config of demo3_byagent_agent1 by its global name (even though it is an Agent service).
  - Agent: update the config of demo3 by its local name (converted to the global name internally).
"""

from example_utils import setup_example_import

setup_example_import()
from mcpstore import MCPStore


def add_service_with_url(ctx, name: str, url: str):
    ctx.add_service(
        {
            "mcpServers": {
                name: {
                    "url": url,
                }
            }
        }
    )


def print_services(label: str, services):
    print(f"\n[{label}] service list, {len(services)} total:")
    for idx, svc in enumerate(services, 1):
        status = getattr(svc.status, "value", str(svc.status))
        print(f"  [{idx}] name={svc.name}, client_id={svc.client_id}, status={status}, global?={'_byagent_' in svc.name}")


def main():
    store = MCPStore.setup_store(debug=False)
    url = "http://115.190.220.103:2230/mcp"

    # Clean up the old configuration
    store.for_store().reset_config()

    # Add the Store services demo1 and demo2
    add_service_with_url(store.for_store(), "demo1", url)
    add_service_with_url(store.for_store(), "demo2", url)

    # Add the Agent1 service demo3
    add_service_with_url(store.for_agent("agent1"), "demo3", url)

    # Initial listing
    print_services("Store perspective", store.for_store().list_services())
    print_services("Agent1 perspective", store.for_agent("agent1").list_services())

    # Update the Agent service from the Store perspective (global name)
    store.for_store().patch_service("demo3_byagent_agent1", {"headers": {"X-Demo": "store-update"}})
    print("\n[Store update] called patch_service('demo3_byagent_agent1', headers...)")

    # Update its own service from the Agent perspective (local name, converted to the global name internally)
    store.for_agent("agent1").patch_service("demo3", {"headers": {"X-Agent": "agent-update"}})
    print("[Agent update] called agent1.patch_service('demo3', headers...)")

    # List again to verify names are unchanged and configs updated
    print_services("Store perspective (after update)", store.for_store().list_services())
    print_services("Agent1 perspective (after update)", store.for_agent("agent1").list_services())

    # Clean up
    store.for_store().reset_config()


if __name__ == "__main__":
    main()
