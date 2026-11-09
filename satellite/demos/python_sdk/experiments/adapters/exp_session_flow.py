"""
Session flow example
Demonstrates the basic MCPStore flow of session creation, service binding, and tool calls
"""

from mcpstore import MCPStore


def main():
    """Demonstrate the basic session workflow."""

    # Initialize the store
    store = MCPStore.setup_store(debug=False)
    ctx = store.for_store()

    # Add the Playwright browser service
    ctx.add_service({
        "name": "playwright",
        "command": "npx",
        "args": ["-y", "@playwright/mcp@latest", "server"]
    })

    # Wait for the service to become ready
    ctx.wait_service("playwright")

    # Create a session
    session = ctx.create_session("demo_session")

    # Bind the service to the session
    try:
        session.bind_service("playwright")

        # Run tool calls within the session
        session.use_tool("playwright_browser_navigate", {"url": "https://www.baidu.com"})
        session.use_tool("playwright_browser_snapshot", {"input": ""})

    except Exception:
        pass  # Ignore tool call errors

    # Clean up the configuration
    ctx.reset_config()


if __name__ == "__main__":
    main()
