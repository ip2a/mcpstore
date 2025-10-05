from mcpstore import MCPStore

store = MCPStore.setup_store(debug=True)
# store = MCPStore.setup_store(mcp_json=r'S:\BaiduSyncdisk\2025_6\mcpstore\test_workspaces\workspace1\mcp.json', debug=False)
# store = MCPStore.setup_store(mcp_json=r'S:\BaiduSyncdisk\2025_6\mcpstore\test_workspaces\workspace1\mcp.json')
l = store.get_json_config()
print(l)

# l = store.get_health_status()
# print(l)
print('--')
l = store.show_mcpjson()
print(l)

print('--')
l = store.get_data_space_info()
print(l)


# print('重置mcpjson')
# l = store.for_store().reset_mcp_json_file()
# print(l)
#

print('重置mcpjson')
l = store.for_store().reset_config()
print(l)

print('--')
l = store.show_mcpjson()
print(l)
