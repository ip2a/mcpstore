from example_utils import setup_example_import

setup_example_import()
from mcpstore import MCPStore


# ============================================================
# Standard Workflow Example - MCPStore Complete Operations
# ============================================================

print("\n" + "=" * 60)
print("  MCPStore Standard Workflow Example")
print("=" * 60)

# ------------------------------------------------------------
# Step 1: Initialize MCPStore
# ------------------------------------------------------------
print("\n[Step 1] Initialize MCPStore")
store = MCPStore.setup_store(debug=True)
print("  └─ ✓ MCPStore instance created successfully")

# ------------------------------------------------------------
# Step 2: Reset Configuration (Clean Environment)
# ------------------------------------------------------------
print("\n[Step 2] Reset Configuration")
store.for_store().reset_config()
print("  └─ ✓ Configuration reset successfully")

# ------------------------------------------------------------
# Step 3: Show Initial Configuration (Empty State)
# ------------------------------------------------------------
print("\n[Step 3] Show Initial Configuration")
config = store.for_store().show_config()
summary = config.get('summary', {})
agents = config.get('agents', {})
print(f"  ├─ Total Agents: {summary.get('total_agents', 0)}")
print(f"  ├─ Total Services: {summary.get('total_services', 0)}")
print(f"  ├─ Total Clients: {summary.get('total_clients', 0)}")
print(f"  ├─ Agents: {list(agents.keys()) if agents else []}")
print("  └─ ✓ Initial configuration is empty")

# ------------------------------------------------------------
# Step 4: Add MCP Service
# ------------------------------------------------------------
print("\n[Step 4] Add MCP Service")
agent_name = "demo_agent"
service_name = "mcpstore"
service_config = {
    "mcpServers": {
        service_name: {
            "url": "https://www.mcpstore.wiki/mcp"
        }
    }
}
store.for_store().add_service(service_config)
print(f"  ├─ Service Name: {service_name}")
print(f"  ├─ Agent Name: {agent_name}")
print("  └─ ✓ Service added successfully")

# ------------------------------------------------------------
# Step 5: Wait for Service Ready
# ------------------------------------------------------------
print("\n[Step 5] Wait for Service Ready")
store.for_store().wait_service(service_name)
print(f"  ├─ Service Name: {service_name}")
print("  └─ ✓ Service is ready")

# ------------------------------------------------------------
# Step 6: List All Services
# ------------------------------------------------------------
print("\n[Step 6] List All Services")
services = store.for_store().list_services()
print(f"  ├─ Total Services: {len(services)}")
# for idx, service in enumerate(services, 1):
#     svc_name = service.name
#     svc_status = str(service.get('status', 'N/A')).split('.')[-1].replace("'", "")
#     svc_url = service.get('url', 'N/A')
#     svc_tools = service.get('tool_count', 0)
#     print(f"  ├─ [{idx}] {svc_name}")
#     print(f"  │   ├─ Status: {svc_status}")
#     print(f"  │   ├─ URL: {svc_url}")
#     print(f"  │   └─ Tools: {svc_tools}")
# print("  └─ ✓ Service list retrieved successfully")

# ------------------------------------------------------------
# Step 7: List All Tools
# ------------------------------------------------------------
print("\n[Step 7] List All Tools")
tools = store.for_store().list_tools()
print(f"  ├─ Total Tools: {len(tools)}")
for idx, tool in enumerate(tools, 1):
    tool_name = tool.get('name', 'N/A')
    tool_desc = tool.get('description', 'N/A')
    tool_service = tool.get('service_name', 'N/A')
    print(f"  ├─ [{idx}] {tool_name}")
    print(f"  │   ├─ Service: {tool_service}")
    print(f"  │   └─ Description: {tool_desc}")
print("  └─ ✓ Tool list retrieved successfully")

# ------------------------------------------------------------
# Step 8: Call Tool
# ------------------------------------------------------------
print("\n[Step 8] Call Tool")
tool_name = "mcpstore_get_mcpstore_docs"
tool_params = {}
tool_result = store.for_store().call_tool(tool_name, tool_params)
print(f"  ├─ Tool: {tool_name}")
print(f"  ├─ Parameters: {tool_params}")
if isinstance(tool_result, dict):
    is_error = tool_result.get('is_error', False)
    content = tool_result.get('content', [])
    print(f"  ├─ Is Error: {is_error}")
    print(f"  ├─ Content Items: {len(content)}")
    for idx, item in enumerate(content, 1):
        item_type = item.get('type', 'N/A')
        item_text = item.get('text', 'N/A')
        print(f"  ├─ [{idx}] Type: {item_type}")
        print(f"  │   └─ Text: {item_text}")
print("  └─ ✓ Tool called successfully")

# ------------------------------------------------------------
# Step 9: Show Configuration Before Reset (With Services)
# ------------------------------------------------------------
print("\n[Step 9] Show Configuration Before Reset")
config = store.for_store().show_config()
summary = config.get('summary', {})
agents = config.get('agents', {})
print(f"  ├─ Total Agents: {summary.get('total_agents', 0)}")
print(f"  ├─ Total Services: {summary.get('total_services', 0)}")
print(f"  ├─ Total Clients: {summary.get('total_clients', 0)}")
for agent_name, agent_data in agents.items():
    services = agent_data.get('services', {})
    print(f"  ├─ Agent: {agent_name}")
    for svc_name, svc_data in services.items():
        svc_url = svc_data.get('config', {}).get('url', 'N/A')
        svc_client = svc_data.get('client_id', 'N/A')
        print(f"  │   ├─ Service: {svc_name}")
        print(f"  │   │   ├─ URL: {svc_url}")
        print(f"  │   │   └─ Client ID: {svc_client}")
print("  └─ ✓ Configuration retrieved successfully")

# ------------------------------------------------------------
# Step 10: Reset Configuration (Final Cleanup)
# ------------------------------------------------------------
print("\n[Step 10] Reset Configuration")
store.for_store().reset_config()
print("  └─ ✓ Configuration reset successfully")

# ============================================================
print("\n" + "=" * 60)
print("  Standard Workflow Completed")
print("=" * 60)
print()
