"""
MCPStore remote data space call example
Demonstrates using MCPStore in a remote-configuration environment,
with a custom config file path and basic remote service calls.
"""

from mcpstore import MCPStore

# Configure a service
demo_mcp = {
    "mcpServers": {
        "mcpstore-demo": {
            "url": "https://mcpstore.wiki/mcp"
        }
    }
}


store = MCPStore.setup_store(debug=False)


# Add the service and wait until it is ready
store.for_store().add_service(demo_mcp)
wait_result = store.for_store().wait_service("mcpstore-demo")

# Retrieve configuration info
mcpjson_content = store.show_mcpjson()

# List services and tools
services = store.for_store().list_services()
tools = store.for_store().list_tools()

# Test a tool call
result = store.for_store().use_tool('get_current_weather', {"query": "北京"})

# Clear the configuration so the demo can run again cleanly
store.for_store().reset_config()