"""Official API: patch and update a service's public definition by its name in the current scope."""

from mcpstore import MCPStore

URL = "http://115.190.220.103:2230/mcp"


def main() -> None:
    store = MCPStore.setup_store(debug=False)
    context = store.for_store()
    context.reset_config()
    try:
        context = context.add_service({"mcpServers": {"weather": {"url": URL}}})
        service = context.find_service(service_name="weather")
        service = service.patch_service({"headers": {"X-Demo": "patched"}})
        print("after_patch=", service.config())
        service = service.update_service({"url": URL, "headers": {"X-Demo": "updated"}})
        print("after_update=", service.config())
    finally:
        context.reset_config()


if __name__ == "__main__":
    main()
