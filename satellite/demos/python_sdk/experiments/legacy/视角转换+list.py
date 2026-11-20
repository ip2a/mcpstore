"""
Perspective switch + list example

Scenario:
- demo1 and demo2 are added under the Store (same configured URL).
- demo3 is added under Agent1 (same configured URL).
- demo4 is added under Agent2 (same configured URL).
- Shows three listing results:
  1) for_store().list_services() → returns global names (Agent services carrying the _byagent_ suffix)
  2) for_agent("agent1").list_services() → returns local names (no suffix)
  3) for_agent("global_agent_store").list_services() → returns the Store's own services (no suffix), with any mapped suffixed services resolved to local names
"""

from example_utils import setup_example_import

setup_example_import()
from mcpstore import MCPStore


def add_service_with_url(ctx, name: str, url: str):
    """Helper that adds a single-service config."""
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

    # Add the Agent2 service demo4
    add_service_with_url(store.for_agent("agent2"), "demo4", url)

    # List from the Store perspective (shows global names)
    store_services = store.for_store().list_services()
    print_services("Store perspective for_store()", store_services)

    # List from the Agent1 perspective (local names, no suffix)
    agent1_services = store.for_agent("agent1").list_services()
    print_services("Agent perspective for_agent('agent1')", agent1_services)

    # List with global_agent_store acting as an Agent (global names; no other Agent services)
    global_agent_services = store.for_agent("global_agent_store").list_services()
    print_services("Agent perspective for_agent('global_agent_store')", global_agent_services)

    # List from the Agent2 perspective (local names, no suffix)
    agent2_services = store.for_agent("agent2").list_services()
    print_services("Agent perspective for_agent('agent2')", agent2_services)

    # Clean up
    store.for_store().reset_config()


if __name__ == "__main__":
    main()
