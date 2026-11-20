"""
Special Playwright session example
"""

from mcpstore import MCPStore


def main():
    """Demonstrate the core feature."""

    # Initialize the store
    store = MCPStore.setup_store(debug=False)
    ctx = store.for_store()

    # Add the sample service
    ctx.add_service({service_config})

    # Wait for the service to become ready
    ctx.wait_service("playwright")

    # Core feature demo
    try:
        # Perform the operation for this file type
        # Special session demo
        session = ctx.create_session("special_playwright")
        session.bind_service("playwright")
        session.use_tool("playwright_browser_navigate", {"url": "https://www.baidu.com"})
    except Exception:
        pass  # Ignore tool call errors

    # Clean up the configuration
    ctx.reset_config()


if __name__ == "__main__":
    main()
