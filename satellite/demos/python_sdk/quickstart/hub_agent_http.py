"""Example of quickly exposing an Agent as an MCP service over HTTP.
See the example guide document (示例指导文件.md).
"""

from example_utils import setup_example_import

setup_example_import()


from mcpstore import MCPStore
from mcpstore.config import MemoryConfig


store = MCPStore.setup_store(source=MemoryConfig(), debug=True)
# store.for_store().reset_config()
agent_name = "demo_agent_http"
agent = store.for_agent(agent_name)

service_config = {
    "mcpServers": {
        "mcpstore": {
            # "url": "http://115.190.220.103:2230/mcp"
            "url": "http://115.190.220.103:2230/mcp"
        }
    }
}
agent.add_service(service_config)
agent.wait_service(service_name="mcpstore")
agent.hub_http(port=8011, host="0.0.0.0", path="/mcp",block=True)


