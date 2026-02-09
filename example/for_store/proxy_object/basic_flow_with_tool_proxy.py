from example_utils import setup_example_import

setup_example_import()
from mcpstore import MCPStore


print("\n" + "=" * 60)
print("  ToolProxy Usage Example")
print("=" * 60)

print("\n" + "NOTE: ToolProxy works with unified AgentProxy caching system")
print("      Consistent behavior across all agent access methods")

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
            # "url": "https://mcp.context7.com/mcp"
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
# Step 6: List All Services
# ------------------------------------------------------------
print("\n[Step 6] List All Services")
services = store.for_store().list_services()
print(f"  ├─ Total Services: {len(services)}")
for idx, service in enumerate(services, 1):
    svc_name = getattr(service, "name", "N/A")
    svc_status = str(getattr(service, "status", "N/A")).split('.')[-1].replace("'", "")
    svc_url = getattr(service, "url", "N/A")
    svc_tools = getattr(service, "tool_count", 0)
    print(f"  ├─ [{idx}] {svc_name}")
    print(f"  │   ├─ Status: {svc_status}")
    print(f"  │   ├─ URL: {svc_url}")
    print(f"  │   └─ Tools: {svc_tools}")
print("  └─ ✓ Service list retrieved successfully")

# ------------------------------------------------------------
# Step 7: List All Tools
# ------------------------------------------------------------
print("\n[Step 7] List All Tools")
tools = store.for_store().list_tools()
print(f"  ├─ Total Tools: {len(tools)}")
for idx, tool in enumerate(tools, 1):
    tool_name = getattr(tool, "name", getattr(tool, "tool_original_name", "N/A"))
    tool_desc = getattr(tool, "description", "N/A")
    tool_service = getattr(tool, "service_name", getattr(tool, "service_original_name", "N/A"))
    print(f"  ├─ [{idx}] {tool_name}")
    print(f"  │   ├─ Service: {tool_service}")
    print(f"  │   └─ Description: {tool_desc}")
print("  └─ ✓ Tool list retrieved successfully")

# ------------------------------------------------------------
# Step 8: Get ServiceProxy Object
# ------------------------------------------------------------
print("\n[Step 8] Get ServiceProxy Object")
service_proxy = store.for_store().find_service(service_name)
print(f"  ├─ Service Proxy: {service_proxy}")
print("  └─ ✓ ServiceProxy obtained successfully")

# ------------------------------------------------------------
# Step 9: Get ToolProxy Object (Multiple Ways)
# ------------------------------------------------------------
print("\n[Step 9] Get ToolProxy Object")

# Use specific tool name for testing
selected_tool_name = "mcpstore_get_mcpstore_docs"
tool_proxy_from_store = store.for_store().find_tool(selected_tool_name)
print(f"  ├─ ToolProxy (Store Context): {tool_proxy_from_store}")

# Also get via service context
tool_proxy_from_service = service_proxy.find_tool(selected_tool_name)
print(f"  ├─ ToolProxy (Service Context): {tool_proxy_from_service}")

# Get tool info for reference
tool_info_for_ref = tool_proxy_from_store.tool_info()
selected_tool_desc = tool_info_for_ref.get('description', 'N/A')

print(f"  ├─ Selected Tool: {selected_tool_name}")
print("  └─ ✓ ToolProxy objects obtained successfully")

# ------------------------------------------------------------
# Step 10: Basic Properties Test
# ------------------------------------------------------------
print("\n[Step 10] Basic Properties Test")
tool_proxy = tool_proxy_from_store
print(f"  ├─ Tool Name: {tool_proxy.tool_name}")
print(f"  ├─ Tool Name (Property): {tool_proxy.name}")
print(f"  ├─ Context Type: {tool_proxy.context_type}")
print(f"  ├─ Scope: {tool_proxy.scope}")
print(f"  ├─ Service Name: {tool_proxy.service_name}")
print("  └─ ✓ Basic properties retrieved successfully")

# ------------------------------------------------------------
# Step 11: Tool Information Test
# ------------------------------------------------------------
print("\n[Step 11] Tool Information Test")
tool_info = tool_proxy.tool_info()
print(f"  ├─ Tool Info Type: {type(tool_info).__name__}")
print(f"  ├─ Name: {tool_info.get('name', 'N/A')}")
print(f"  ├─ Description: {tool_info.get('description', 'N/A')}")
print(f"  ├─ Service Name: {tool_info.get('service_name', 'N/A')}")
print(f"  ├─ Client ID: {tool_info.get('client_id', 'N/A')}")
print(f"  ├─ Scope: {tool_info.get('scope', 'N/A')}")
print(f"  ├─ Tags Count: {len(tool_info.get('tags', []))}")
print(f"  ├─ Meta Keys: {list(tool_info.get('meta', {}).keys())}")
print("  └─ ✓ Tool information retrieved successfully")

# ------------------------------------------------------------
# Step 12: Tool Schema Test
# ------------------------------------------------------------
print("\n[Step 12] Tool Schema Test")
tool_schema = tool_proxy.tool_schema()
print(f"  ├─ Has Schema: {tool_schema is not None}")
print(f"  ├─ Schema Type: {type(tool_schema).__name__ if tool_schema else 'None'}")
if tool_schema:
    schema_keys = list(tool_schema.keys()) if isinstance(tool_schema, dict) else 'N/A'
    print(f"  ├─ Schema Keys: {schema_keys}")
print(f"  ├─ Has Schema (Property): {tool_proxy.has_schema}")
print("  └─ ✓ Tool schema retrieved successfully")

# ------------------------------------------------------------
# Step 13: Tool Tags and Meta Test
# ------------------------------------------------------------
print("\n[Step 13] Tool Tags and Meta Test")
tool_tags = tool_proxy.tool_tags()
tool_meta = tool_proxy.tool_meta()
print(f"  ├─ Tags: {tool_tags}")
print(f"  ├─ Tags Count: {len(tool_tags)}")
print(f"  ├─ Meta Keys: {list(tool_meta.keys())}")
print(f"  ├─ MCPStore Meta: {tool_meta.get('_mcpstore', {})}")
print("  └─ ✓ Tags and meta retrieved successfully")

# ------------------------------------------------------------
# Step 14: Tool Availability Test
# ------------------------------------------------------------
print("\n[Step 14] Tool Availability Test")
print(f"  ├─ Is Available: {tool_proxy.is_available}")
print(f"  ├─ Description: {tool_proxy.description}")
print("  └─ ✓ Tool availability checked successfully")

# ------------------------------------------------------------
# Step 15: Call Tool Test
# ------------------------------------------------------------
print("\n[Step 15] Call Tool Test")
tool_name = selected_tool_name
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
# Step 15.1: ToolProxy Call Tool Test
# ------------------------------------------------------------
print("\n[Step 15.1] ToolProxy Call Tool Test")
try:
    tool_proxy_result = tool_proxy.call_tool(tool_params, return_extracted=False)
    print(f"  ├─ ToolProxy: {tool_proxy}")
    print(f"  ├─ Parameters: {tool_params}")
    print(f"  ├─ Result Type: {type(tool_proxy_result).__name__}")

    # Check if result is ToolCallResult
    if hasattr(tool_proxy_result, 'content'):
        print(f"  ├─ Is ToolCallResult: True")
        print(f"  ├─ Is Error: {tool_proxy_result.is_error}")
        content_items = tool_proxy_result.content if tool_proxy_result.content else []
        print(f"  ├─ Content Items: {len(content_items)}")

        # Try to get text output safely
        try:
            text_output = tool_proxy_result.text_output if hasattr(tool_proxy_result, 'text_output') else 'N/A'
            if text_output != 'N/A' and len(text_output) > 100:
                text_output = text_output[:100] + "..."
            print(f"  ├─ Text Output: {text_output}")
        except:
            print(f"  ├─ Text Output: Not available")

        # Try to get called_at safely
        try:
            called_at = tool_proxy_result.called_at if hasattr(tool_proxy_result, 'called_at') else 'N/A'
            print(f"  ├─ Called At: {called_at}")
        except:
            print(f"  ├─ Called At: Not available")

    else:
        print(f"  ├─ Is ToolCallResult: False")
        print(f"  ├─ Result: {str(tool_proxy_result)[:100]}...")

    print("  └─ ✓ ToolProxy call completed successfully")
except Exception as e:
    print(f"  ├─ ToolProxy: {tool_proxy}")
    print(f"  ├─ Parameters: {tool_params}")
    print(f"  ├─ Error: {str(e)}")
    print("  └─ ⚠ ToolProxy call failed")

# ------------------------------------------------------------
# Step 16: Usage Statistics Test
# ------------------------------------------------------------
print("\n[Step 16] Usage Statistics Test")
usage_stats = tool_proxy.usage_stats()
print(f"  ├─ Tool Name: {usage_stats.get('tool_name', 'N/A')}")
print(f"  ├─ Total Calls: {usage_stats.get('total_calls', 0)}")
print(f"  ├─ Recent Calls: {usage_stats.get('recent_calls', 0)}")
print(f"  ├─ Success Rate: {usage_stats.get('success_rate', 0.0)}")
print(f"  ├─ Average Duration: {usage_stats.get('average_duration', 0.0)}")
if 'note' in usage_stats:
    print(f"  ├─ Note: {usage_stats['note']}")
if 'error' in usage_stats:
    print(f"  ├─ Error: {usage_stats['error']}")
print("  └─ ✓ Usage statistics retrieved successfully")

# ------------------------------------------------------------
# Step 17: Call History Test
# ------------------------------------------------------------
print("\n[Step 17] Call History Test")
call_history = tool_proxy.call_history(limit=5)
print(f"  ├─ History Records: {len(call_history)}")
for idx, record in enumerate(call_history, 1):
    record_time = record.get('timestamp', 'N/A')
    record_success = not record.get('is_error', True)
    print(f"  ├─ [{idx}] Time: {record_time}, Success: {record_success}")
print("  └─ ✓ Call history retrieved successfully")

# ------------------------------------------------------------
# Step 18: Redirect Configuration Test
# ------------------------------------------------------------
print("\n[Step 18] Redirect Configuration Test")
original_tool = tool_proxy
redirected_tool = tool_proxy.set_redirect(True)
print(f"  ├─ Original Tool: {original_tool}")
print(f"  ├─ Redirected Tool: {redirected_tool}")
print(f"  ├─ Same Object: {original_tool is redirected_tool}")
print("  └─ ✓ Redirect configuration applied successfully")

# ------------------------------------------------------------
# Step 19: Test Call with Validation
# ------------------------------------------------------------
print("\n[Step 19] Test Call with Validation")
try:
    test_result = tool_proxy.test_call({})
    print(f"  ├─ Test Call: Successful")
    print(f"  ├─ Result Type: {type(test_result).__name__}")
    print("  └─ ✓ Test call completed successfully")
except Exception as e:
    print(f"  ├─ Test Call: Failed")
    print(f"  ├─ Error: {str(e)}")
    print("  └─ ⚠ Test call failed")

# ------------------------------------------------------------
# Step 20: String Representation Test
# ------------------------------------------------------------
print("\n[Step 20] String Representation Test")
print(f"  ├─ String Representation: {str(tool_proxy)}")
print(f"  ├─ Repr: {repr(tool_proxy)}")
print("  └─ ✓ String representation completed successfully")

# ------------------------------------------------------------
# Step 21: Show Configuration Before Reset
# ------------------------------------------------------------
print("\n[Step 21] Show Configuration Before Reset")
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
# Step 22: Reset Configuration (Final Cleanup)
# ------------------------------------------------------------
print("\n[Step 22] Reset Configuration")
store.for_store().reset_config()
print("  └─ ✓ Configuration reset successfully")

# ============================================================
print("\n" + "=" * 60)
print("  ToolProxy Usage Completed")
print("=" * 60)
print()
