from example_utils import setup_example_import

setup_example_import()
from mcpstore import MCPStore


# ============================================================
# ServiceProxy Usage Example - Complete Method Demonstration
# ============================================================

print("\n" + "=" * 60)
print("  ServiceProxy Usage Example")
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
print("  └─ ✓ Service added successfully")

# ------------------------------------------------------------
# Step 5: Wait for Service Ready
# ------------------------------------------------------------
print("\n[Step 5] Wait for Service Ready")
store.for_store().wait_service(service_name)
print(f"  ├─ Service Name: {service_name}")
print("  └─ ✓ Service is ready")

# ------------------------------------------------------------
# Step 6: Get ServiceProxy Object
# ------------------------------------------------------------
print("\n[Step 6] Get ServiceProxy Object")
service_proxy = store.for_store().find_service(service_name)
print(f"  ├─ Proxy Object: {service_proxy}")
print("  └─ ✓ ServiceProxy obtained successfully")

# ------------------------------------------------------------
# Step 7: Basic Properties
# ------------------------------------------------------------
print("\n[Step 7] Basic Properties")
print(f"  ├─ Service Name: {service_proxy.name}")
print(f"  ├─ Service Name (Alt): {service_proxy.service_name}")
print(f"  ├─ Context Type: {service_proxy.context_type}")
print(f"  ├─ Tools Count: {service_proxy.tools_count}")
print(f"  ├─ Is Connected: {service_proxy.is_connected}")
print("  └─ ✓ Basic properties retrieved successfully")

# ------------------------------------------------------------
# Step 8: Service Information
# ------------------------------------------------------------
print("\n[Step 8] Service Information")
service_info = service_proxy.service_info()
print(f"  ├─ Service Info Type: {type(service_info).__name__}")
if hasattr(service_info, 'name'):
    print(f"  ├─ Name: {service_info.name}")
    print(f"  ├─ URL: {getattr(service_info, 'url', 'N/A')}")
    print(f"  ├─ Status: {getattr(service_info, 'status', 'N/A')}")
print("  └─ ✓ Service information retrieved successfully")

# ------------------------------------------------------------
# Step 9: Service Status
# ------------------------------------------------------------
print("\n[Step 9] Service Status")
service_status = service_proxy.service_status()
print(f"  ├─ Status Type: {type(service_status).__name__}")
if isinstance(service_status, dict):
    for key, value in list(service_status.items())[:5]:
        print(f"  ├─ {key}: {value}")
print("  └─ ✓ Service status retrieved successfully")

# ------------------------------------------------------------
# Step 10: Health Check
# ------------------------------------------------------------
print("\n[Step 10] Health Check")
is_healthy = service_proxy.is_healthy()
print(f"  ├─ Is Healthy: {is_healthy}")
health_check = service_proxy.check_health()
print(f"  ├─ Service Name: {health_check.get('service_name', 'N/A')}")
print(f"  ├─ Status: {health_check.get('status', 'N/A')}")
print(f"  ├─ Healthy: {health_check.get('healthy', False)}")
print(f"  ├─ Response Time: {health_check.get('response_time', 'N/A')}")
print("  └─ ✓ Health check completed successfully")

# ------------------------------------------------------------
# Step 11: Health Details
# ------------------------------------------------------------
print("\n[Step 11] Health Details")
health_details = service_proxy.health_details()
print(f"  ├─ Service Name: {health_details.get('service_name', 'N/A')}")
print(f"  ├─ Status: {health_details.get('status', 'N/A')}")
print(f"  ├─ Healthy: {health_details.get('healthy', False)}")
print(f"  ├─ Response Time: {health_details.get('response_time', 'N/A')}")
print(f"  ├─ Error Message: {health_details.get('error_message', 'None')}")
print("  └─ ✓ Health details retrieved successfully")

# ------------------------------------------------------------
# Step 12: List Tools
# ------------------------------------------------------------
print("\n[Step 12] List Tools")
tools = service_proxy.list_tools()
print(f"  ├─ Total Tools: {len(tools)}")
for idx, tool in enumerate(tools, 1):
    tool_name = tool.name if hasattr(tool, 'name') else 'N/A'
    tool_desc = tool.description if hasattr(tool, 'description') else 'N/A'
    print(f"  ├─ [{idx}] {tool_name}")
    print(f"  │   └─ Description: {tool_desc}")
print("  └─ ✓ Tools list retrieved successfully")

# ------------------------------------------------------------
# Step 13: Tools Statistics
# ------------------------------------------------------------
print("\n[Step 13] Tools Statistics")
tools_stats = service_proxy.tools_stats()
metadata = tools_stats.get('metadata', {})
print(f"  ├─ Total Tools: {metadata.get('total_tools', 0)}")
print(f"  ├─ Services Count: {metadata.get('services_count', 0)}")
tools_by_service = metadata.get('tools_by_service', {})
for svc, count in tools_by_service.items():
    print(f"  ├─ {svc}: {count} tools")
print("  └─ ✓ Tools statistics retrieved successfully")

# ------------------------------------------------------------
# Step 14: Find Specific Tool
# ------------------------------------------------------------
print("\n[Step 14] Find Specific Tool")
if len(tools) > 0:
    first_tool_name = tools[0].name if hasattr(tools[0], 'name') else None
    if first_tool_name:
        tool_proxy = service_proxy.find_tool(first_tool_name)
        print(f"  ├─ Tool Proxy: {tool_proxy}")
        print(f"  ├─ Tool Name: {first_tool_name}")
        print("  └─ ✓ Tool proxy obtained successfully")
    else:
        print("  └─ ⚠ No tool name available")
else:
    print("  └─ ⚠ No tools available to find")

# ------------------------------------------------------------
# Step 15: Patch Service Configuration
# ------------------------------------------------------------
print("\n[Step 15] Patch Service Configuration")
patch_updates = {"custom_field": "example_value"}
patch_result = service_proxy.patch_config(patch_updates)
print(f"  ├─ Patch Updates: {patch_updates}")
print(f"  ├─ Patch Result: {patch_result}")
print("  └─ ✓ Configuration patched successfully")

# ------------------------------------------------------------
# Step 16: Refresh Service Content
# ------------------------------------------------------------
print("\n[Step 16] Refresh Service Content")
refresh_result = service_proxy.refresh_content()
print(f"  ├─ Refresh Result: {refresh_result}")
print("  └─ ✓ Service content refreshed successfully")

# ------------------------------------------------------------
# Step 17: Show Configuration Before Reset
# ------------------------------------------------------------
print("\n[Step 17] Show Configuration Before Reset")
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
# Step 18: Reset Configuration (Final Cleanup)
# ------------------------------------------------------------
print("\n[Step 18] Reset Configuration")
store.for_store().reset_config()
print("  └─ ✓ Configuration reset successfully")

# ============================================================
print("\n" + "=" * 60)
print("  ServiceProxy Usage Completed")
print("=" * 60)
print()
