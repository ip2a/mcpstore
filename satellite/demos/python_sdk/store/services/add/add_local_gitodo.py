"""Official API: add a local ``uvx gitodo`` service and wait for it to become ready in the current Store scope."""

from mcpstore import MCPStore


def main() -> None:
    store = MCPStore.setup_store(debug=False)
    context = store.for_store()
    context.reset_config()
    try:
        context = context.add_service(
            {"mcpServers": {"gitodo": {"command": "uvx", "args": ["gitodo"]}}}
        )
        context = context.wait_service(service_name="gitodo", timeout=60.0)
        print("services=", [service.info() for service in context.list_services()])
        print("tools=", [tool.info() for tool in context.list_tools()])
    finally:
        context.reset_config()


if __name__ == "__main__":
    main()
