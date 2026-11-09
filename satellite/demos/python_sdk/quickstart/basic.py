"""The current official API: the minimal complete flow under the store scope."""

from example_utils import setup_example_import

setup_example_import()
from mcpstore import MCPStore
from mcpstore.config import MemoryConfig

URL = "http://115.190.220.103:2230/mcp"


def main() -> None:
    store = MCPStore.setup_store(source=MemoryConfig(), debug=False)
    context = store.for_store()
    context.reset_config()
    try:
        context.add_service({"mcpServers": {"weather": {"url": URL}}})
        context.wait_service(service_name="weather", timeout=20.0)

        services = context.list_services()
        tools = context.list_tools()
        print(f"services={len(services)} tools={len(tools)}")
        print(f"service={services[0]['service_name']} instance_id={services[0]['instance_id']}")

        weather = next(tool for tool in tools if tool["tool_name"] == "get_weather")
        result = context.call_tool(weather["name"], {"city": "北京"})
        print(f"tool={weather['name']} is_error={result['is_error']} content={result['content']}")
    finally:
        context.reset_config()


if __name__ == "__main__":
    main()
