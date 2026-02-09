"""
Store Service Removal Example
Demonstrates using remove_service() to remove running service instances, showing dynamic service management
Context: Store level
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
# Store Service Removal Example
# ============================================================

print("\n" + "=" * 60)
print("  MCPStore Service Removal Example")
print("=" * 60)

# ------------------------------------------------------------
# Step 1: Initialize MCPStore
# ------------------------------------------------------------
print("\n[Step 1] Initialize MCPStore")
store = MCPStore.setup_store(debug=True)
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
            "url": "https://mcpstore.wiki/mcp"
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
store.for_store().wait_service("mcpstore-wiki", timeout=30.0)
print("  ├─ Service Name: mcpstore-wiki")
print("  └─ ✓ Service is ready")

# ------------------------------------------------------------
# Step 5: Verify Service Exists
# ------------------------------------------------------------
print("\n[Step 5] Verify Service Exists")
services_before = store.for_store().list_services()
service_proxy = store.for_store().find_service("mcpstore-wiki")
tools_before = service_proxy.list_tools()
print(f"  ├─ Services Before: {len(services_before)}")
print(f"  ├─ Tools Available: {len(tools_before)}")
print("  └─ ✓ Service verified")

# ------------------------------------------------------------
# Step 6: Remove Service
# ------------------------------------------------------------
print("\n[Step 6] Remove Service")
result = service_proxy.remove_service()
print("  ├─ Service Name: mcpstore-wiki")
print("  └─ ✓ Service removed successfully")

# ------------------------------------------------------------
# Step 7: Verify Service Removed
# ------------------------------------------------------------
print("\n[Step 7] Verify Service Removed")
services_after = store.for_store().list_services()
print(f"  ├─ Services After: {len(services_after)}")
try:
    removed_service = store.for_store().find_service("mcpstore-wiki")
    print("  ├─ Service Found: Yes (unexpected)")
except Exception:
    print("  ├─ Service Found: No (expected)")
print("  └─ ✓ Service removal verified")

# ------------------------------------------------------------
# Step 8: Re-add Service
# ------------------------------------------------------------
print("\n[Step 8] Re-add Service")
store.for_store().add_service(service_config)
store.for_store().wait_service("mcpstore-wiki", timeout=30.0)
print("  ├─ Service Name: mcpstore-wiki")
print("  └─ ✓ Service re-added successfully")

# ------------------------------------------------------------
# Step 9: Add Multiple Services and Selective Removal
# ------------------------------------------------------------
print("\n[Step 9] Add Multiple Services and Selective Removal")
multi_config = {
    "mcpServers": {
        "search": {"url": "https://mcpstore.wiki/mcp"},
        "translate": {"url": "https://mcpstore.wiki/mcp"}
    }
}
store.for_store().add_service(multi_config)
store.for_store().wait_service("search", timeout=30.0)
store.for_store().wait_service("translate", timeout=30.0)
print("  ├─ Services Added: search, translate")
print("  └─ ✓ Multiple services added successfully")

# ------------------------------------------------------------
# Step 10: Remove Specific Service
# ------------------------------------------------------------
print("\n[Step 10] Remove Specific Service")
search_proxy = store.for_store().find_service("search")
search_proxy.remove_service()
print("  ├─ Service Removed: search")
print("  └─ ✓ Selective service removal completed")

# ------------------------------------------------------------
# Step 11: Reset Configuration (Final Cleanup)
# ------------------------------------------------------------
print("\n[Step 11] Reset Configuration (Final Cleanup)")
store.for_store().reset_config()
print("  └─ ✓ Configuration reset successfully")

# ============================================================
print("\n" + "=" * 60)
print("  Service Removal Example Completed")
print("=" * 60)
print()
