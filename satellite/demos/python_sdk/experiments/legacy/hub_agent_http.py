"""Example of quickly exposing an Agent as an MCP service over HTTP.
See 示例指导文件.md for guidance.
"""

from example_utils import setup_example_import

setup_example_import()


from mcpstore import MCPStore


store = MCPStore.setup_store(debug=True)
# store.for_store().reset_config()
agent_name = "demo_agent_http"
agent = store.for_agent(agent_name)

service_config = {
    "mcpServers": {
        "mcpstore_wiki": {
            # "url": "http://115.190.220.103:2230/mcp"
            "url": "http://115.190.220.103:2230/mcp"
        }
    }
}
agent.add_service(service_config)
agent.wait_service("mcpstore_wiki")
agent.hub_http(port=8011, host="0.0.0.0", path="/mcp",block=True)


