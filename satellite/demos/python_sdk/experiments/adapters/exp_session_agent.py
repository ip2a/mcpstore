"""
Session agent example
Demonstrates agent operations in session mode
"""

from mcpstore import MCPStore


def main():
    """Demonstrate basic session agent functionality."""

    # Initialize the store
    store = MCPStore.setup_store(debug=False)
    ctx = store.for_store()

    # Create a session
    session = ctx.create_session("agent_session")

    # Add the sample service
    ctx.add_service({
        "name": "playwright",
        "command": "npx",
        "args": ["-y", "@playwright/mcp@latest", "server"]
    })

    # Wait for the service to become ready
    ctx.wait_service("playwright")

    # Bind the service to the session
    try:
        session.bind_service("playwright")

        # Run tool calls within the session
        session.use_tool("playwright_browser_navigate", {"url": "https://www.baidu.com"})

    except Exception:
        pass  # Ignore tool call errors

    # Clean up the configuration
    ctx.reset_config()


if __name__ == "__main__":
    main()
