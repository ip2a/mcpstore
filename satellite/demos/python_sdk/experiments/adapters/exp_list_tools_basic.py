"""
Basic Store LangChain adapter example
Demonstrates listing LangChain-compatible tools via the for_langchain() adapter
"""

from mcpstore import MCPStore

# Initialize the store and add the service
store = MCPStore.setup_store(debug=False)
store_ctx = store.for_store()
demo_mcp = {
    "mcpServers": {
        "mcpstore-demo-weather": {
            "url": "https://mcpstore.wiki/mcp"
        }
    }
}
store.for_store().add_service(demo_mcp)
store.for_store().wait_service("mcpstore-demo-weather")

# List tools through the LangChain adapter
langchain_tools = store_ctx.for_langchain().list_tools()

# Clean up the configuration
store.for_store().reset_config()