from mcpstore import MCPStore

demo_mcp = {
  "mcpServers": {
    "howtocook": {
      "command": "npx",
      "args": [
        "-y",
        "howtocook-mcp"
      ]
    }
  }
}
store = MCPStore.setup_store(debug=True)
store.for_store().add_service(demo_mcp)
ws = store.for_store().wait_service("howtocook")
print(ws)
ls = store.for_store().list_services()
print(ls)

lt = store.for_store().list_tools()
print(lt)

rt = store.for_store().use_tool('mcp_howtocook_getAllRecipes',{})
print(rt)