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
# Redis Backend
# ============================================================

print("\n" + "=" * 60)
print("  MCPStore Standard Workflow Example (Redis Backend)")
print("=" * 60)

# ------------------------------------------------------------
# Step 1: Initialize MCPStore with Redis Backend
# ------------------------------------------------------------
print("\n[Step 1] Initialize MCPStore with Redis Backend")
redis_config = RedisConfig(
    host=ExampleConfig.REDIS_HOST,
    port=ExampleConfig.REDIS_PORT,
    password=ExampleConfig.REDIS_PASSWORD,
    namespace=ExampleConfig.REDIS_NAMESPACE
)
store = MCPStore.setup_store(debug=False, cache=redis_config,only_db=True)
print(f"  ├─ Redis Host: {ExampleConfig.REDIS_HOST}")
print(f"  ├─ Redis Port: {ExampleConfig.REDIS_PORT}")
print(f"  ├─ Namespace: {ExampleConfig.REDIS_NAMESPACE}")
print("  └─ ✓ MCPStore instance created successfully (Redis Backend)")

# ------------------------------------------------------------
# Step 2: Reset Configuration (Show Before/After)
# ------------------------------------------------------------
print("\n[Step 2] Configuration (Show Before/After)")
config_before_db = store.for_store().show_config()
print("  ├─ Config before db =>")
print(config_before_db)


# ------------------------------------------------------------
# Step 5: List All Services
# ------------------------------------------------------------
print("\n[Step 5] List All Services")
services = store.for_store().list_services()
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
# Step 6: List All Tools
# ------------------------------------------------------------
print("\n[Step 6] List All Tools")
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
# Step 7: Call Tool
# ------------------------------------------------------------
print("\n[Step 7] Call Tool")
tool_name = "mcpstore_byagent_demo_agent_http_get_mcpstore_docs"
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

# ============================================================
print("\n" + "=" * 60)
print("  Standard Workflow Completed (Redis Backend)")
print("=" * 60)
print()
