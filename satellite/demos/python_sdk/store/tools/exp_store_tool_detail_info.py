"""Special topic: Tool.info() returns a complete data snapshot of the current tool."""

from mcpstore import MCPStore

URL = "http://115.190.220.103:2230/mcp"


def main() -> None:
    store = MCPStore.setup_store(debug=False)
    context = store.for_store()
    context.reset_config()
    try:
        context = context.add_service({"mcpServers": {"weather": {"url": URL}}})
        context = context.wait_service(service_name="weather", timeout=20.0)
        tool = context.find_tool("get_weather")
        print("tool_info=", tool.info())
    finally:
        context.reset_config()


if __name__ == "__main__":
    main()
