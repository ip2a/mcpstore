from mcpstore import MCPStore

store = MCPStore.setup_store(debug=False)
agent_id = "agent_demo"

demo_mcp = {"mcpServers": {"mcpstore-demo-weather": {"url": "https://mcpstore.wiki/mcp"}}}

store.for_agent(agent_id).add_service(demo_mcp)
store.for_agent(agent_id).wait_service("mcpstore-demo-weather")

# 1) 直接工具名（单服务场景可行）
print("-- call_tool: 直接工具名 --")
print(store.for_agent(agent_id).call_tool("get_current_weather", {"query": "北京"}))

# 2) 服务前缀（新格式，推荐在多服务场景使用）
print("-- call_tool: 服务前缀 --")
print(store.for_agent(agent_id).call_tool("mcpstore-demo-weather__get_current_weather", {"query": "北京"}))

# 3) 旧格式（保持向后兼容）
print("-- call_tool: 旧格式 --")
print(store.for_agent(agent_id).call_tool("mcpstore-demo-weather_get_current_weather", {"query": "北京"}))

# 4) use_tool 别名（与 call_tool 等价）
print("-- use_tool: 直接工具名 --")
print(store.for_agent(agent_id).use_tool("get_current_weather", {"query": "北京"}))

print("-- 清理配置 --")
print(store.for_store().reset_config())

