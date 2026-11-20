"""
Multi-session example
Demonstrates MCPStore's multi-session management and session isolation
"""

from mcpstore import MCPStore


def main():
    """Demonstrate basic multi-session functionality."""

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

    # Create several distinct sessions
    browser_session = ctx.create_session("browser_work")
    test_session = ctx.create_session("test_work")

    # Bind the service to each session
    browser_session.bind_service("playwright")
    test_session.bind_service("playwright")

    # Run a different task in each session
    try:
        # Browser session: open Baidu
        browser_session.use_tool("playwright_browser_navigate", {"url": "https://www.baidu.com"})

        # Test session: open Google
        test_session.use_tool("playwright_browser_navigate", {"url": "https://www.google.com"})

    except Exception:
        pass  # Ignore tool call errors

    # Create a session in an agent context
    agent_session = store.for_agent("test_agent").create_session("agent_session")

    # List all sessions
    all_sessions = ctx.list_sessions()

    # Clean up the configuration
    ctx.reset_config()


if __name__ == "__main__":
    main()