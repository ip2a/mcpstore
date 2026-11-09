"""Official API: Service.disconnect_service drops the connection without removing the service's declaration from the current scope."""

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
        service = service.disconnect_service()
        assert context.find_service(service_name="weather").info()["service_name"] == "weather"
        print("disconnect_keeps_declaration=", service.info()["instance_id"])
    finally:
        context.reset_config()


if __name__ == "__main__":
    main()
