import time

from mcpstore import MCPStore

demo_mcp = {
  "mcpServers": {
    "mcpstore-demo": {
      "url": "https://mcpstore.wiki/mcp"
    }
  }
}
store = MCPStore.setup_store(debug=True)
store.for_store().add_service(demo_mcp)
ws = store.for_store().wait_service("mcpstore-demo")
print(ws)
ls = store.for_store().list_services()
print(ls)
lt = store.for_store().list_tools()
print(lt)
rt = store.for_store().use_tool('get_current_weather', {"query":'北京'})
print(rt)