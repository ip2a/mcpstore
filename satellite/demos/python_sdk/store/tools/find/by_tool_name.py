"""Look up a Tool object in the current Store scope by its tool name."""

from mcpstore import MCPStore

URL = "http://115.190.220.103:2230/mcp"


def main() -> None:
    store = MCPStore.setup_store(debug=False)
    context = store.for_store()
    context.reset_config()
    try:
        context = context.add_service({"mcpServers": {"weather": {"url": URL}}})
        context = context.wait_service(service_name="weather", timeout=20.0)

        listed_tool = context.list_tools()[0]
        tool = context.find_tool(listed_tool.info()["name"])
        assert tool.info()["instance_id"] == listed_tool.info()["instance_id"]
        assert tool.info()["tool_name"] == listed_tool.info()["tool_name"]
        print(tool.info())
    finally:
        context.reset_config()


if __name__ == "__main__":
    main()
