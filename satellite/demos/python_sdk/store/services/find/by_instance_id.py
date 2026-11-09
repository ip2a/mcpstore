"""Look up a Service object in the current Store scope by its instance ID."""

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
        instance_id = service.info()["instance_id"]

        found = context.find_service(instance_id=instance_id)
        assert found.info()["instance_id"] == instance_id
        print(found.info())
    finally:
        context.reset_config()


if __name__ == "__main__":
    main()
