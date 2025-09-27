import time

from mcpstore import MCPStore

demo_mcp = {
  "mcpServers": {
    "mcpstore-demo-weather": {
      "url": "https://mcpstore.wiki/mcp"
    }
  }
}
store = MCPStore.setup_store(debug=False)
store.for_store().add_service(demo_mcp)
w1 = store.for_store().wait_service("mcpstore-demo-weather")
print(w1)
ls = store.for_store().list_services()
print(ls)
lt = store.for_store().for_langchain().list_tools()
print(lt)
p = {"query":'北京'}
lu = store.for_store().use_tool('get_current_weather', p)
print(lu)