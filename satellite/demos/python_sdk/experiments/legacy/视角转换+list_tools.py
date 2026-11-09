"""
Perspective switch + list_tools example

Scenario:
- demo1 and demo2 are added under the Store (same URL).
- demo3 is added under Agent1.
- demo4 is added under Agent2.
Demonstrates:
- How list_services and list_tools differ across the Store / Agent1 / Agent2 / for_agent(global_agent_store) perspectives.
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
    ctx.wait_service(name)


def print_services(label: str, services):
    print(f"\n[{label}] service list, {len(services)} total:")
    for idx, svc in enumerate(services, 1):
        status = getattr(svc.status, "value", str(svc.status))
        print(f"  [{idx}] name={svc.name}, client_id={svc.client_id}, status={status}, global?={'_byagent_' in svc.name}")


def print_tools(label: str, tools):
    print(f"\n[{label}] tool list, {len(tools)} total:")
    for idx, tool in enumerate(tools, 1):
        print(f"  [{idx}] name={tool.name}, service={tool.service_name}, global_service={getattr(tool, 'service_global_name', '')}")


def main():
    store = MCPStore.setup_store(debug=True)
    url = "https://www.mcpstore.wiiki/mcp"
    url = "http://101.126.12.142:21923/mcp"
    url = "http://115.190.220.103:2230/mcp"

    # Clean up the old configuration
    store.for_store().reset_config()

    # Add the services
    add_service_with_url(store.for_store(), "demo1", url)
    add_service_with_url(store.for_store(), "demo2", url)
    add_service_with_url(store.for_agent("agent1"), "demo3", url)
    add_service_with_url(store.for_agent("agent2"), "demo4", url)

    # List from each perspective
    print_services("Store perspective", store.for_store().list_services())
    print_tools("Store perspective tools", store.for_store().list_tools())

    print_services("Agent1 perspective", store.for_agent("agent1").list_services())
    print_tools("Agent1 perspective tools", store.for_agent("agent1").list_tools())

    print_services("Agent2 perspective", store.for_agent("agent2").list_services())
    print_tools("Agent2 perspective tools", store.for_agent("agent2").list_tools())

    print_services("for_agent(global_agent_store) perspective", store.for_agent("global_agent_store").list_services())
    print_tools("for_agent(global_agent_store) tools", store.for_agent("global_agent_store").list_tools())

    # Clean up
    store.for_store().reset_config()


if __name__ == "__main__":
    main()
