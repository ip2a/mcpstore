"""Add HTTP and local stdio services in one call, then wait for each in turn within the current Store scope."""

from mcpstore import MCPStore

URL = "http://115.190.220.103:2230/mcp"


def main() -> None:
    store = MCPStore.setup_store(debug=False)
    context = store.for_store()
    context.reset_config()
    try:
        context = context.add_service(
            {
                "mcpServers": {
                    "weather": {"url": URL},
                    "gitodo": {"command": "uvx", "args": ["gitodo"]},
                }
            }
        )
        for service_name in ("weather", "gitodo"):
            context = context.wait_service(service_name=service_name, timeout=60.0)
            print(service_name, context.find_service(service_name=service_name).state())
        print("services=", [service.info() for service in context.list_services()])
    finally:
        context.reset_config()


if __name__ == "__main__":
    main()
