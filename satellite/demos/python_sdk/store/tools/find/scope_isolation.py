"""Look up Tool objects for the same MCP service separately in the Store and Agent scopes."""

from mcpstore import MCPStore

URL = "http://115.190.220.103:2230/mcp"


def add_ready_service(context, service_name: str):
    context = context.add_service({"mcpServers": {service_name: {"url": URL}}})
    return context.wait_service(service_name=service_name, timeout=20.0)


def main() -> None:
    store = MCPStore.setup_store(debug=False)
    store_context = store.for_store()
    agent_context = store.for_agent("weather-agent")
    store_context.reset_config()
    agent_context.reset_config()
    try:
        store_context = add_ready_service(store_context, "weather-store")
        agent_context = add_ready_service(agent_context, "weather-agent")

        store_tool = store_context.find_tool(store_context.list_tools()[0].info()["name"])
        agent_tool = agent_context.find_tool(agent_context.list_tools()[0].info()["name"])
        assert store_tool.info()["scope"] == {"type": "store"}
        assert agent_tool.info()["scope"] == {"type": "agent", "agent_id": "weather-agent"}
        print(f"store={store_tool.info()['name']} agent={agent_tool.info()['name']}")
    finally:
        store_context.reset_config()
        agent_context.reset_config()


if __name__ == "__main__":
    main()
