"""
Pure API scenario example
Demonstrates using MCPStore purely through its API
"""

from mcpstore import MCPStore


def main():
    """Demonstrate the pure API call style."""

    # Initialize the store
    store = MCPStore.setup_store(debug=False)
    ctx = store.for_store()

    # Add the remote API service
    ctx.add_service({
        "name": "api_service",
        "url": "https://api.example.com/mcp"
    })

    # Wait for the service to become ready
    ctx.wait_service("api_service")

    # Make the API calls
    try:
        # List the available tools
        tools = ctx.list_tools()

        # Invoke a concrete tool
        if tools:
            tool_name = tools[0].name
            ctx.use_tool(tool_name, {"param": "value"})

    except Exception:
        pass  # Ignore API call errors

    # Clean up the configuration
    ctx.reset_config()


if __name__ == "__main__":
    main()
