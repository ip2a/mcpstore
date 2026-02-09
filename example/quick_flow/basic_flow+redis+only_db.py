from pathlib import Path
import sys

EXAMPLE_DIR = Path(__file__).resolve().parent.parent
if str(EXAMPLE_DIR) not in sys.path:
    sys.path.insert(0, str(EXAMPLE_DIR))

from example_utils import setup_example_import, ExampleConfig

setup_example_import()
from mcpstore import MCPStore
from mcpstore.config import RedisConfig


# ============================================================
# Standard Workflow Example - MCPStore with Enhancements
# Redis + Only DB Mode (Read & Use Existing Data)
# ============================================================

print("\n" + "=" * 60)
print("  MCPStore Standard Workflow Example (Redis + Only DB Mode)")
print("=" * 60)

# ------------------------------------------------------------
# Step 1: Initialize MCPStore (ONLY_DB + Redis)
# ------------------------------------------------------------
print("\n[Step 1] Initialize MCPStore (ONLY_DB + Redis)")
redis_config = RedisConfig(
    host=ExampleConfig.REDIS_HOST,
    port=ExampleConfig.REDIS_PORT,
    password=ExampleConfig.REDIS_PASSWORD,
    namespace=ExampleConfig.REDIS_NAMESPACE,
    allow_partial=True,
)
store = MCPStore.setup_store(debug=True, cache=redis_config, only_db=True)
snapshot = getattr(store, "_setup_snapshot", {}) or {}
print(f"  ├─ Backend: Redis ({redis_config.host}:{redis_config.port})")
print(f"  ├─ Namespace: {redis_config.namespace or 'mcpstore'}")
print(f"  ├─ Mode: ONLY_DB (Read existing data only)")
print(f"  └─ mcp.json Path: {snapshot.get('mcp_json', 'N/A')}")

# ------------------------------------------------------------
# Step 2: Inspect Cache Backend (ONLY_DB View)
# ------------------------------------------------------------
print("\n[Step 2] Inspect Cache Backend (ONLY_DB View)")
cache_view = store.for_store().find_cache().inspect()
print(f"  ├─ Backend Type: {cache_view.get('backend')}")
print(f"  ├─ Namespace: {cache_view.get('namespace')}")
print(f"  └─ Collections: {cache_view.get('collections')}")
print(f"  ├─ Entities Count: {cache_view.get('counts', {}).get('entities')}")
print(f"  ├─ Relations Count: {cache_view.get('counts', {}).get('relations')}")
print(f"  └─ States Count: {cache_view.get('counts', {}).get('states')}")

# ------------------------------------------------------------
# Step 3: List All Services (Reuse Existing)
# ------------------------------------------------------------
print("\n[Step 3] List All Services (ONLY_DB - Reuse Existing)")
services = store.for_store().list_services()
if not services:
    print("  └─ [WARNING] No services found in cache. Please first write mcpstore services and tools in non-only_db mode.")
    exit(1)
print(f"  ├─ Total Services: {len(services)}")
for idx, service in enumerate(services, 1):
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
# Step 4: List All Tools (Reuse Existing)
# ------------------------------------------------------------
print("\n[Step 4] List All Tools (ONLY_DB - Reuse Existing)")
tools = store.for_store().list_tools()
print(f"  ├─ Total Tools: {len(tools)}")
for idx, tool in enumerate(tools, 1):
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
# Step 5: Call Tool (Reuse Existing)
# ------------------------------------------------------------
print("\n[Step 5] Call Tool (ONLY_DB - Reuse Existing)")
tool_name = "mcpstore_get_mcpstore_docs"
tool_params = {}
tool_result = store.for_store().call_tool(tool_name, tool_params)
print(f"  ├─ Tool: {tool_name}")
print(f"  ├─ Parameters: {tool_params}")
is_error = tool_result.is_error
content = tool_result.content
print(f"  ├─ Is Error: {is_error}")
print(f"  ├─ Content Items: {len(content)}")
for idx, item in enumerate(content, 1):
    item_type = item.type
    item_text = item.text
    item_text_display = item_text[:100] + "..." if len(item_text) > 100 else item_text
    print(f"  ├─ [{idx}] Type: {item_type}")
    print(f"  │   └─ Text: {item_text_display}")
if tool_result.data:
    print(f"  ├─ Data: {tool_result.data}")
print("  └─ ✓ Tool called successfully")

# ------------------------------------------------------------
# Step 6: Show Configuration (ONLY_DB - Read Only)
# ------------------------------------------------------------
print("\n[Step 6] Show Configuration (ONLY_DB - Read Only)")
config = store.for_store().show_config()
print("  ├─ Config (read-only view) =>")
print(config)
print("  └─ ✓ Configuration retrieved successfully")

# ============================================================
print("\n" + "=" * 60)
print("  Standard Workflow Completed (Redis + Only DB Mode)")
print("=" * 60)
print()
