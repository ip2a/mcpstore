"""
Session-synchronized click example
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
        # Demonstrate session-synchronized clicks
        ctx.session_auto("sync_session")
        session = ctx.current_session()
        session.use_tool("playwright_browser_navigate", {"url": "https://www.baidu.com"})
        session.use_tool("playwright_browser_click", {"selector": "#kw"})
    except Exception:
        pass  # Ignore tool call errors

    # Clean up the configuration
    ctx.reset_config()


if __name__ == "__main__":
    main()
