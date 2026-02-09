"""
Service Restart Basic Functionality Example
Demonstrates how to use restart_service() method to restart a service
"""

from pathlib import Path
import sys

FOR_STORE_DIR = Path(__file__).resolve().parent.parent
if str(FOR_STORE_DIR) not in sys.path:
    sys.path.insert(0, str(FOR_STORE_DIR))

from example_utils import setup_example_import

setup_example_import()
from mcpstore import MCPStore

# ============================================================
# Service Restart Basic Functionality Example
# ============================================================

print("\n" + "=" * 60)
print("  MCPStore Service Restart Example")
print("=" * 60)

# ------------------------------------------------------------
# Step 1: Initialize MCPStore
# ------------------------------------------------------------
print("\n[Step 1] Initialize MCPStore")
store = MCPStore.setup_store(debug=False)
print("  └─ ✓ MCPStore instance created successfully")

# ------------------------------------------------------------
# Step 2: Reset Configuration
# ------------------------------------------------------------
print("\n[Step 2] Reset Configuration")
store.for_store().reset_config()
print("  └─ ✓ Configuration reset successfully")

# ------------------------------------------------------------
# Step 3: Add MCP Service
# ------------------------------------------------------------
print("\n[Step 3] Add MCP Service")
service_config = {
    "mcpServers": {
        "mcpstore-wiki": {
            "url": "https://www.mcpstore.wiki/mcp"
        }
    }
}
store.for_store().add_service(service_config)
print("  ├─ Service Name: mcpstore-wiki")
print("  └─ ✓ Service added successfully")

# ------------------------------------------------------------
# Step 4: Wait for Service Ready
# ------------------------------------------------------------
print("\n[Step 4] Wait for Service Ready")
store.for_store().wait_service("mcpstore-wiki", timeout=15)
print("  ├─ Service Name: mcpstore-wiki")
print("  └─ ✓ Service is ready")

# ------------------------------------------------------------
# Step 5: Get Service Proxy
# ------------------------------------------------------------
print("\n[Step 5] Get Service Proxy")
service_proxy = store.for_store().find_service("mcpstore-wiki")
print("  └─ ✓ Service proxy retrieved")

# ------------------------------------------------------------
# Step 6: Restart Service
# ------------------------------------------------------------
print("\n[Step 6] Restart Service")
restart_result = store.for_store().restart_service("mcpstore-wiki")
print("  ├─ Service Name: mcpstore-wiki")
print("  └─ ✓ Service restart initiated")

# ------------------------------------------------------------
# Step 7: Wait for Service Ready After Restart
# ------------------------------------------------------------
print("\n[Step 7] Wait for Service Ready After Restart")
store.for_store().wait_service("mcpstore-wiki", timeout=30.0)
print("  ├─ Service Name: mcpstore-wiki")
print("  └─ ✓ Service is ready after restart")

# ------------------------------------------------------------
# Step 8: Verify Service Availability
# ------------------------------------------------------------
print("\n[Step 8] Verify Service Availability")
tools = service_proxy.list_tools()
print(f"  ├─ Tools Available: {len(tools)}")
print("  └─ ✓ Service availability verified")

# ------------------------------------------------------------
# Step 9: Test Tool Call
# ------------------------------------------------------------
print("\n[Step 9] Test Tool Call")
if tools:
    try:
        tool_name = tools[0].name
        result = store.for_store().use_tool(tool_name, {"query": "Beijing"})
        print(f"  ├─ Tool: {tool_name}")
        print("  └─ ✓ Tool call test completed")
    except Exception:
        print("  └─ ⚠ Tool call test skipped (service may not support this tool)")

# ------------------------------------------------------------
# Step 10: Reset Configuration (Final Cleanup)
# ------------------------------------------------------------
print("\n[Step 10] Reset Configuration (Final Cleanup)")
store.for_store().reset_config()
print("  └─ ✓ Configuration reset successfully")

# ============================================================
print("\n" + "=" * 60)
print("  Service Restart Example Completed")
print("=" * 60)
print()

