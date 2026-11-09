
from example_utils import setup_example_import, ExampleConfig

setup_example_import()


from mcpstore import MCPStore
# prod_store = MCPStore.setup_store('./prod_workspace/mcp.json')
prod_store = MCPStore.setup_store(debug = True)
# prod_store = MCPStore.setup_store(mcp_config_file=r'S:\BaiduSyncdisk\2025_6\mcpstore\test_workspaces\workspace1\mcp.json')
# prod_store = MCPStore.setup_store(mcp_config_file=r'S:\BaiduSyncdisk\2025_6\mcpstore\test_workspaces\workspace1\mcp.json',debug = True)
prod_store.start_api_server(
    host='0.0.0.0',
    port=18200,
    show_startup_info=False,
    # log_level='warning'
)

