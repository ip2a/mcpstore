"""Special topic: read the input JSON Schema from the data returned by Tool.info()."""

from mcpstore import MCPStore

URL = "http://115.190.220.103:2230/mcp"


def main() -> None:
    store = MCPStore.setup_store(debug=False)
    context = store.for_store()
    context.reset_config()
    try:
        context = context.add_service({"mcpServers": {"weather": {"url": URL}}})
        context = context.wait_service(service_name="weather", timeout=20.0)
        tool_info = context.find_tool("get_weather").info()
        print("input_schema=", tool_info["input_schema"])
    finally:
        context.reset_config()


if __name__ == "__main__":
    main()
