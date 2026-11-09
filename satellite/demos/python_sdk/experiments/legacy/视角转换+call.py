"""
Perspective switch + call example

Scenario setup:
- demo1 and demo2 are added under the Store (same URL).
- demo3 is added under Agent1.
- demo4 is added under Agent2.
Calls demonstrated:
- Store perspective: invoke a tool by its global name (taken from list_tools).
- Agent perspective: invoke a tool by its local name (resolved to the global name for execution).
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
    url = "http://101.126.12.142:21923/mcp"
    url = "http://115.190.220.103:2230/mcp"

    # Clean up the old configuration
    store.for_store().reset_config()
    print(store.for_store().show_config())
    # Add the services
    add_service_with_url(store.for_store(), "demo1", url)
    add_service_with_url(store.for_store(), "demo2", url)
    add_service_with_url(store.for_agent("agent1"), "demo3", url)
    add_service_with_url(store.for_agent("agent2"), "demo4", url)

    # List services and tools
    store_services = store.for_store().list_services()
    print_services("Store perspective", store_services)
    store_tools = store.for_store().list_tools()
    print_tools("Store perspective tools", store_tools)

    agent1_services = store.for_agent("agent1").list_services()
    print_services("Agent1 perspective", agent1_services)
    agent1_tools = store.for_agent("agent1").list_tools()
    print_tools("Agent1 perspective tools", agent1_tools)

    # Pick a sample tool name (if any tools exist)
    if store_tools:
        store_tool_name = next((t.name for t in store_tools if "get_mcpstore_docs" in t.name), store_tools[0].name)
        print(f"\n[Store call] using tool: {store_tool_name}")
        res = store.for_store().call_tool(store_tool_name, {})
        print(res)
        print(f"  ├─ success={not res.is_error}, items={len(res.content)}")

    if agent1_tools:
        agent_tool_name = next((t.name for t in agent1_tools if "get_mcpstore_docs" in t.name), agent1_tools[0].name)  # local name
        print(f"\n[Agent1 call] using tool: {agent_tool_name}")
        res = store.for_agent("agent1").call_tool(agent_tool_name, {})
        print(res)
        print(f"  ├─ success={not res.is_error}, items={len(res.content)}")

    # Clean up
    store.for_store().reset_config()


if __name__ == "__main__":
    main()
