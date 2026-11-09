"""Official API: Service.restart_service restarts an HTTP MCP service in the current Store scope."""

from mcpstore import MCPStore

URL = "http://115.190.220.103:2230/mcp"


def main() -> None:
    store = MCPStore.setup_store(debug=False)
    context = store.for_store()
    context.reset_config()
    try:
        context = context.add_service({"mcpServers": {"weather": {"url": URL}}})
        service = context.wait_service(service_name="weather", timeout=20.0).find_service(
            service_name="weather"
        )
        service = service.restart_service().wait_service(timeout=20.0)
        tool = service.find_tool("get_weather")
        result = tool.call({"city": "北京"})
        print(f"restarted tool={tool.info()['name']} is_error={result['is_error']}")
    finally:
        context.reset_config()


if __name__ == "__main__":
    main()
