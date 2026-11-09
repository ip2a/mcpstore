"""
Store configuration reset example
Demonstrates using reset_config() to fully reset the MCPStore configuration,
clearing all services and cached data.
"""

from mcpstore import MCPStore

# Initialize the store and add services
store = MCPStore.setup_store(debug=False)
service_config = {
    "mcpServers": {
        "weather": {
            "url": "https://mcpstore.wiki/mcp"
        },
        "search": {
            "url": "https://mcpstore.wiki/mcp"
        }
    }
}
store.for_store().add_service(service_config)

# Inspect the configuration before the reset
config_before = store.for_store().show_config()
services_before = store.for_store().list_services()

# Reset the configuration with reset_config()
reset_result = store.for_store().reset_config()

# Inspect the configuration after the reset
config_after = store.for_store().show_config()
services_after = store.for_store().list_services()

