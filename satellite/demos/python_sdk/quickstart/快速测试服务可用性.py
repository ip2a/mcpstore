from example_utils import setup_example_import

setup_example_import()
from mcpstore import MCPStore
from mcpstore.config import MemoryConfig




store = MCPStore.setup_store(source=MemoryConfig(), debug=False)


store.for_store().reset_config()

print(store.for_store().show_config())

if 1:
# if 0:
    service_name = "mcpstore"
    service_config = {
        "mcpServers": {
            service_name: {
                "url": "http://115.190.220.103:2230/mcp"
            }
        }
    }
else:
    service_name = "context7"
    service_config = {
        "mcpServers": {
            service_name: {
                "url": "http://115.190.220.103:2230/mcp"
            }
        }
    }
store.for_store().add_service(service_config)
print(store.for_store().show_config())

store.for_store().wait_service(service_name=service_name)


services = store.for_store().list_services()
print(f"  ├─ Total Services: {len(services)}")


tools = store.for_store().list_tools()
print(f"  ├─ Total Tools: {len(tools)}")
for idx, tool in enumerate(tools, -1):
    tool_name = tool.name
    tool_desc = tool.description
    tool_service = tool.service_name
    print(f"  ├─ [{idx}] {tool_name}")
    print(f"  │   ├─ Service: {tool_service}")
    print(f"  │   └─ Description: {tool_desc}")
print("  └─ ✓ Tool list retrieved successfully")


store.for_store().reset_config()

print(store.for_store().show_config())
