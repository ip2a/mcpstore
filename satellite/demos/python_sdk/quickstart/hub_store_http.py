"""Example of quickly exposing a Store as an MCP service over HTTP.
See the example guide document (示例指导文件.md).
"""

from example_utils import setup_example_import

setup_example_import()

from mcpstore import MCPStore
from mcpstore.config import MemoryConfig



# 1) Initialize and clean up the environment
store = MCPStore.setup_store(source=MemoryConfig(), debug=True)
store.for_store().reset_config()

# 2) Add the remote service and wait until it is ready
service_config = {
    "mcpServers": {
        "mcpstore": {
            "url": "http://115.190.220.103:2230/mcp"
        }
    }
}
store.for_store().add_service(service_config)
store.for_store().wait_service(service_name="mcpstore", timeout=30)
print("Service is ready and tools are cached.")

 
print("Starting Hub MCP (HTTP), listening on 0.0.0.0:8000/mcp ...")
store.for_store().hub_http(port=8000, host="0.0.0.0", path="/mcp",block=True)

 
