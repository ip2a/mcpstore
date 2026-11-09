"""
Simple integration example
Demonstrates basic MCPStore integration with various frameworks
"""

from mcpstore import MCPStore


def main():
    """Demonstrate basic adapter integration."""

    # Initialize the store
    store = MCPStore.setup_store(debug=False)
    ctx = store.for_store()

    # Add the sample service
    ctx.add_service({
        "name": "weather",
        "url": "https://mcpstore.wiki/mcp"
    })

    # Wait for the service to become ready
    ctx.wait_service("weather")

    # Fetch tools through the framework adapters
    langchain_tools = ctx.for_langchain().list_tools()
    openai_tools = ctx.for_openai().list_tools()

    # Clean up the configuration
    ctx.reset_config()


if __name__ == "__main__":
    main()
