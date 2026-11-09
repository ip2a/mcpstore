"""Example of quickly exposing the Store as an MCP service over HTTP.
See 示例指导文件.md for guidance.
"""

from example_utils import setup_example_import

setup_example_import()

from mcpstore import MCPStore



# 1) Initialize and clean up the environment
store = MCPStore.setup_store(debug=True)
store.for_store().reset_config()

# 2) Add the remote service and wait for it to become ready
service_config = {
    "mcpServers": {
        "mcpstore_wiki": {
            "url": "http://115.190.220.103:2230/mcp"
        }
    }
}
store.for_store().add_service(service_config)
store.for_store().wait_service("mcpstore_wiki", timeout=30)
print("Service is ready; tools are cached.")

 
print("Starting Hub MCP (HTTP), listening on 0.0.0.0:8000/mcp ...")
store.for_store().hub_http(port=8000, host="0.0.0.0", path="/mcp",block=True)

 
