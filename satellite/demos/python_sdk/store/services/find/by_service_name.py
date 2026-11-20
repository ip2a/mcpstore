"""Look up a Service object in the current Store scope by its service name."""

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
        assert service.info()["service_name"] == "weather"
        print(service.info())
    finally:
        context.reset_config()


if __name__ == "__main__":
    main()
