"""Official API: Context.call_tool kept in place, alongside the object-chained Tool.call."""

from mcpstore import MCPStore

URL = "http://115.190.220.103:2230/mcp"
TOOL_NAME = "get_weather"
ARGS = {"city": "北京"}


def main() -> None:
    store = MCPStore.setup_store(debug=False)
    context = store.for_store()
    context.reset_config()
    try:
        context = context.add_service({"mcpServers": {"weather": {"url": URL}}})
        context = context.wait_service(service_name="weather", timeout=20.0)
        tool = context.find_tool(TOOL_NAME)

        direct_result = context.call_tool(TOOL_NAME, ARGS)
        object_result = tool.call(ARGS)
        assert not direct_result["is_error"], direct_result
        assert not object_result["is_error"], object_result
        print(f"tool={tool.info()['name']} content={object_result['content']}")
    finally:
        context.reset_config()


if __name__ == "__main__":
    main()
