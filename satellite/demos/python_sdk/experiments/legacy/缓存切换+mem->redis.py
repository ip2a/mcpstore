from example_utils import setup_example_import, ExampleConfig

setup_example_import()
from mcpstore import MCPStore
from mcpstore.config import RedisConfig


# ============================================================
# Cache Switch Example - Memory → Redis (Preserve Existing Service)
# ============================================================

print("\n" + "=" * 60)
print("  MCPStore Cache Switch Example (Memory -> Redis)")
print("=" * 60)

# ------------------------------------------------------------
# Step 1: Initialize MCPStore (Memory Backend)
# ------------------------------------------------------------
print("\n[Step 1] Initialize MCPStore (Memory Backend)")
store = MCPStore.setup_store(debug=True)
print("  ├─ Backend: Memory")
print("  └─ ✓ MCPStore instance created successfully (Memory Backend)")

# ------------------------------------------------------------
# Step 2: Reset Configuration (Show Before/After)
# ------------------------------------------------------------
print("\n[Step 2] Reset Configuration (Show Before/After)")
config_before_reset = store.for_store().show_config()
store.for_store().reset_config()
config_after_reset = store.for_store().show_config()
print("  ├─ Config before reset =>")
print(config_before_reset)
print("  ├─ Config after reset  =>")
print(config_after_reset)
print("  └─ ✓ Configuration reset and shown (before/after)")

# ------------------------------------------------------------
# Step 3: Add MCP Service (Memory Backend)
# ------------------------------------------------------------
print("\n[Step 3] Add MCP Service (Memory Backend)")
agent_name = "demo_agent"
service_name = "mcpstore"
service_config = {
    "mcpServers": {
        service_name: {
            "url": "http://115.190.220.103:2230/mcp"
        }
    }
}
store.for_store().add_service(service_config)
print(f"  ├─ Service Name: {service_name}")
print(f"  ├─ Agent Name: {agent_name}")
print("  └─ ✓ Service added successfully")

# ------------------------------------------------------------
# Step 3.5: Show Configuration After Add Service (Compare with After Reset)
# ------------------------------------------------------------
print("\n[Step 3.5] Show Configuration After Add Service (Compare with After Reset)")
config_after_add_service = store.for_store().show_config()
print("  ├─ Config after reset (from Step 2) =>")
print(config_after_reset)
print("  ├─ Config after add service        =>")
print(config_after_add_service)
print("  └─ ✓ Configuration comparison shown (reset vs add service)")

# ------------------------------------------------------------
# Step 4: Wait for Service Ready
# ------------------------------------------------------------
print("\n[Step 4] Wait for Service Ready")
store.for_store().wait_service(service_name)
print(f"  ├─ Service Name: {service_name}")
print("  └─ ✓ Service is ready")

# ------------------------------------------------------------
# Step 5: List All Services (Memory Backend)
# ------------------------------------------------------------
print("\n[Step 5] List All Services (Memory Backend)")
services_before = store.for_store().list_services()
print(f"  ├─ Total Services: {len(services_before)}")
for idx, service in enumerate(services_before, 1):
    svc_name = service.name
    svc_status = str(service.status).split(".")[-1].replace("'", "")
    svc_transport = service.transport_type
    svc_transport_str = str(svc_transport).split(".")[-1].replace("'", "") if svc_transport else None
    svc_client_id = service.client_id
    svc_config = service.config if service.config else (service.state_metadata.service_config if service.state_metadata else None)
    svc_tools = service.tool_count
    print(f"  ├─ [{idx}] {svc_name}")
    print(f"  │   ├─ Status: {svc_status}")
    if svc_transport_str:
        print(f"  │   ├─ Transport: {svc_transport_str}")
    # Show URL for HTTP transport, Command/Args for STDIO transport
    if svc_transport_str == "streamable_http" and service.url:
        print(f"  │   ├─ URL: {service.url}")
    elif svc_transport_str == "stdio":
        if service.command:
            print(f"  │   ├─ Command: {service.command}")
        if service.args:
            print(f"  │   ├─ Args: {service.args}")
    if svc_client_id:
        print(f"  │   ├─ Client ID: {svc_client_id}")
    if svc_config:
        print(f"  │   ├─ Config: {svc_config}")
    print(f"  │   └─ Tools: {svc_tools}")
print("  └─ ✓ Service list retrieved successfully")

# ------------------------------------------------------------
# Step 6: List All Tools (Memory Backend)
# ------------------------------------------------------------
print("\n[Step 6] List All Tools (Memory Backend)")
tools_before = store.for_store().list_tools()
print(f"  ├─ Total Tools: {len(tools_before)}")
for idx, tool in enumerate(tools_before, 1):
    tool_name = tool.name
    tool_desc = tool.description
    tool_desc_display = tool_desc[:80] + "..." if len(tool_desc) > 80 else tool_desc
    tool_service = tool.service_name
    tool_input_schema = tool.inputSchema
    tool_required_params = tool_input_schema.get("required", []) if isinstance(tool_input_schema, dict) else []
    print(f"  ├─ [{idx}] {tool_name}")
    print(f"  │   ├─ Service: {tool_service}")
    if tool_required_params:
        print(f"  │   ├─ Required Params: {tool_required_params}")
    print(f"  │   └─ Description: {tool_desc_display}")
print("  └─ ✓ Tool list retrieved successfully")

# ------------------------------------------------------------
# Step 7: Call Tool (Memory Backend)
# ------------------------------------------------------------
print("\n[Step 7] Call Tool (Memory Backend)")
tool_name = "mcpstore_get_mcpstore_docs"
tool_params = {}
tool_result_before = store.for_store().call_tool(tool_name, tool_params)
print(f"  ├─ Tool: {tool_name}")
print(f"  ├─ Parameters: {tool_params}")
is_error = tool_result_before.is_error
content = tool_result_before.content
print(f"  ├─ Is Error: {is_error}")
print(f"  ├─ Content Items: {len(content)}")
for idx, item in enumerate(content, 1):
    item_type = item.type
    item_text = item.text
    item_text_display = item_text[:100] + "..." if len(item_text) > 100 else item_text
    print(f"  ├─ [{idx}] Type: {item_type}")
    print(f"  │   └─ Text: {item_text_display}")
if tool_result_before.data:
    print(f"  ├─ Data: {tool_result_before.data}")
print("  └─ ✓ Tool called successfully")

# ------------------------------------------------------------
# Step 8: Inspect Cache Backend (Before Switch)
# ------------------------------------------------------------
print("\n[Step 8] Inspect Cache Backend (Before Switch)")
cache_view_before = store.for_store().find_cache().inspect()
print(f"  ├─ Backend Type: {cache_view_before.get('backend')}")
print(f"  ├─ Namespace: {cache_view_before.get('namespace')}")
print(f"  └─ Collections: {cache_view_before.get('collections')}")
print(f"  ├─ Entities Count: {cache_view_before.get('counts', {}).get('entities')}")
print(f"  ├─ Relations Count: {cache_view_before.get('counts', {}).get('relations')}")
print(f"  └─ States Count: {cache_view_before.get('counts', {}).get('states')}")

# ------------------------------------------------------------
# Step 9: Switch Cache Backend to Redis (Preserve Existing Service)
# ------------------------------------------------------------
print("\n[Step 9] Switch Cache Backend to Redis")
redis_config = RedisConfig(
    host=ExampleConfig.REDIS_HOST,
    port=ExampleConfig.REDIS_PORT,
    db=7,
    password=ExampleConfig.REDIS_PASSWORD,
    namespace=ExampleConfig.REDIS_NAMESPACE,
    allow_partial=True,
)
store.for_store().switch_cache(redis_config)
print(f"  ├─ Target Host: {redis_config.host}")
print(f"  ├─ Target Port: {redis_config.port}")
print(f"  ├─ Namespace: {redis_config.namespace}")
print("  └─ ✓ Cache backend switched to Redis")

# ------------------------------------------------------------
# Step 10: Inspect Cache Backend (After Switch)
# ------------------------------------------------------------
print("\n[Step 10] Inspect Cache Backend (After Switch)")
cache_view_after = store.for_store().find_cache().inspect()
print(f"  ├─ Backend Type: {cache_view_after.get('backend')}")
print(f"  ├─ Namespace: {cache_view_after.get('namespace')}")
print(f"  └─ Collections: {cache_view_after.get('collections')}")
print(f"  ├─ Entities Count: {cache_view_after.get('counts', {}).get('entities')}")
print(f"  ├─ Relations Count: {cache_view_after.get('counts', {}).get('relations')}")
print(f"  └─ States Count: {cache_view_after.get('counts', {}).get('states')}")

# ------------------------------------------------------------
# Step 11: List All Services (After Switch, No Re-Add)
# ------------------------------------------------------------
print("\n[Step 11] List All Services (After Switch)")
services_after = store.for_store().list_services()
print(f"  ├─ Total Services: {len(services_after)}")
for idx, service in enumerate(services_after, 1):
    svc_name = service.name
    svc_status = str(service.status).split(".")[-1].replace("'", "")
    svc_transport = service.transport_type
    svc_transport_str = str(svc_transport).split(".")[-1].replace("'", "") if svc_transport else None
    svc_client_id = service.client_id
    svc_config = service.config if service.config else (service.state_metadata.service_config if service.state_metadata else None)
    svc_tools = service.tool_count
    print(f"  ├─ [{idx}] {svc_name}")
    print(f"  │   ├─ Status: {svc_status}")
    if svc_transport_str:
        print(f"  │   ├─ Transport: {svc_transport_str}")
    # Show URL for HTTP transport, Command/Args for STDIO transport
    if svc_transport_str == "streamable_http" and service.url:
        print(f"  │   ├─ URL: {service.url}")
    elif svc_transport_str == "stdio":
        if service.command:
            print(f"  │   ├─ Command: {service.command}")
        if service.args:
            print(f"  │   ├─ Args: {service.args}")
    if svc_client_id:
        print(f"  │   ├─ Client ID: {svc_client_id}")
    if svc_config:
        print(f"  │   ├─ Config: {svc_config}")
    print(f"  │   └─ Tools: {svc_tools}")
print("  └─ ✓ Service list retrieved successfully (after switch)")

# ------------------------------------------------------------
# Step 12: Call Tool (After Switch, No Re-Add)
# ------------------------------------------------------------
print("\n[Step 12] Call Tool (After Switch)")
tool_result_after = store.for_store().call_tool(tool_name, tool_params)
print(f"  ├─ Tool: {tool_name}")
print(f"  ├─ Parameters: {tool_params}")
is_error = tool_result_after.is_error
content = tool_result_after.content
print(f"  ├─ Is Error: {is_error}")
print(f"  ├─ Content Items: {len(content)}")
for idx, item in enumerate(content, 1):
    item_type = item.type
    item_text = item.text
    item_text_display = item_text[:100] + "..." if len(item_text) > 100 else item_text
    print(f"  ├─ [{idx}] Type: {item_type}")
    print(f"  │   └─ Text: {item_text_display}")
if tool_result_after.data:
    print(f"  ├─ Data: {tool_result_after.data}")
print("  └─ ✓ Tool called successfully (after switch)")

# ------------------------------------------------------------
# Step 13: Reset Configuration (Final Cleanup - Show Before/After)
# ------------------------------------------------------------
print("\n[Step 13] Reset Configuration (Final Cleanup - Show Before/After)")
config_before_final_reset = store.for_store().show_config()
store.for_store().reset_config()
config_after_final_reset = store.for_store().show_config()
print("  ├─ Config before final reset =>")
print(config_before_final_reset)
print("  ├─ Config after final reset  =>")
print(config_after_final_reset)
print("  └─ ✓ Configuration reset and shown (before/after)")

# ============================================================
print("\n" + "=" * 60)
print("  Cache Switch Completed (Memory -> Redis)")
print("=" * 60)
print()
