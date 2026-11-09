"""
Auto-session agent example
Demonstrates web automation with a LangChain agent in auto-session mode
"""

from mcpstore import MCPStore


def main():
    """Demonstrate agent operations in auto-session mode."""

    # Initialize the store
    store = MCPStore.setup_store(debug=False)
    store_ctx = store.for_store()


    # Enable auto-session mode
    store.for_store().session_auto("auto_agent_session")

    # Add the Playwright browser service
    store.for_store().add_service({
        "name": "playwright",
        "command": "npx",
        "args": ["-y", "@playwright/mcp@latest", "server"]
    })

    # Wait for the service to become ready
    store.for_store().wait_service("playwright")

    # Pre-bind the service to reduce first-call latency
    try:
        sess = store.for_store().current_session()
        sess.bind_service("playwright")
    except Exception:
        pass  # Ignore binding errors

    # Get the LangChain tools
    lc_adapter = store_ctx.for_langchain()
    tools = lc_adapter.list_tools()

    # Direct tool invocation demo (no OpenAI API key required)
    if tools:
        # Navigate to Baidu
        nav_tool = next((t for t in tools if hasattr(t, 'name') and 'navigate' in t.name), None)
        if nav_tool:
            nav_result = nav_tool.invoke({"url": "https://www.baidu.com"})

        # Take a snapshot
        snap_tool = next((t for t in tools if hasattr(t, 'name') and 'snapshot' in t.name), None)
        if snap_tool:
            snap_result = snap_tool.invoke({"input": ""})

    # Disable auto-session mode
    store.for_store().session_manual()

    # Clean up the configuration
    store.for_store().reset_config()


if __name__ == "__main__":
    main()

