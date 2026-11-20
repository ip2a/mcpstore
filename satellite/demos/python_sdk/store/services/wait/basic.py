"""Special topic: wait for a service to become ready; wait_service returns the current Context."""

from mcpstore import MCPStore

URL = "http://115.190.220.103:2230/mcp"


def main() -> None:
    store = MCPStore.setup_store(debug=False)
    context = store.for_store()
    context.reset_config()
    try:
        context = context.add_service({"mcpServers": {"weather": {"url": URL}}})
        ready_context = context.wait_service(service_name="weather", timeout=20.0)
        assert type(ready_context) is type(context)
        print("ready_state=", ready_context.find_service(service_name="weather").state())
        print("tools=", [tool.info() for tool in ready_context.list_tools()])
    finally:
        context.reset_config()


if __name__ == "__main__":
    main()
