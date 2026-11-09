"""Special topic: verify that waiting on an unreachable service times out with an error."""

from mcpstore import MCPStore

UNREACHABLE_URL = "http://127.0.0.1:9/mcp"


def main() -> None:
    store = MCPStore.setup_store(debug=False)
    context = store.for_store()
    context.reset_config()
    try:
        context.add_service({"mcpServers": {"unreachable": {"url": UNREACHABLE_URL}}})
        try:
            context.wait_service(service_name="unreachable", timeout=1.0)
        except RuntimeError as error:
            print("timeout_error=", error)
        else:
            raise AssertionError("unreachable service unexpectedly became ready")
    finally:
        context.reset_config()


if __name__ == "__main__":
    main()
