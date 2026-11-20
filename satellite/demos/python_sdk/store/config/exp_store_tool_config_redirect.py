"""
MCPStore tool redirect configuration example
Demonstrates using set_redirect() to control tool redirect behavior,
governing how tool call results are handled — useful for framework integration.
"""

from mcpstore import MCPStore
import json

# Initialize the store and add services
store = MCPStore.setup_store(debug=True)
service_config = {
    "mcpServers": {
        "weather": {
            "url": "https://mcpstore.wiki/mcp"
        }
    }
}
store.for_store().add_service(service_config)
store.for_store().wait_service("weather", timeout=30.0)

# Find a tool and exercise the redirect feature
tool_name = "get_current_weather"
tool_proxy = store.for_store().find_tool(tool_name)

# Check the initial redirect state
initial_redirect = tool_proxy.set_redirect()

# Enable the redirect and test
tool_proxy.set_redirect(True)
redirect_status_true = tool_proxy.set_redirect()
params = {"query": "北京"}
result_true = tool_proxy.call_tool(params)

# Disable the redirect and test
tool_proxy.set_redirect(False)
redirect_status_false = tool_proxy.set_redirect()
result_false = tool_proxy.call_tool(params)

# Exercise redirect settings across multiple tools
tools = store.for_store().list_tools()
if len(tools) >= 2:
    for tool in tools[:2]:
        proxy = store.for_store().find_tool(tool.name)
        proxy.set_redirect(True)
        redirect_enabled = proxy.set_redirect()
        proxy.set_redirect(False)
        redirect_disabled = proxy.set_redirect()

# Clear the configuration so the demo can run again cleanly
store.for_store().reset_config()

