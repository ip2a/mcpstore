"""
Store Service Configuration Incremental Update Example
Demonstrates using patch_config() method to incrementally update service configuration, supporting partial field updates
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
# Store Service Configuration Incremental Update Example
# ============================================================

print("\n" + "=" * 60)
print("  MCPStore Service Configuration Incremental Update Example")
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
# Step 5: Get Initial Configuration
# ------------------------------------------------------------
print("\n[Step 5] Get Initial Configuration")
service_proxy = store.for_store().find_service("mcpstore-wiki")
initial_info = service_proxy.service_info()
print("  └─ ✓ Initial configuration retrieved")

# ------------------------------------------------------------
# Step 6: Patch Configuration (First Update)
# ------------------------------------------------------------
print("\n[Step 6] Patch Configuration (First Update)")
patch_config = {
    "timeout": 60
}
result = service_proxy.patch_config(patch_config)
patched_info = service_proxy.service_info()
patched_config = patched_info.get('config', {})
print(f"  ├─ Patch Config: {patch_config}")
print(f"  ├─ Updated Config: {patched_config}")
print("  └─ ✓ Configuration patched successfully")

# ------------------------------------------------------------
# Step 7: Patch Configuration (Second Update)
# ------------------------------------------------------------
print("\n[Step 7] Patch Configuration (Second Update)")
patch_config2 = {
    "retry": 3,
    "cache": True
}
result2 = service_proxy.patch_config(patch_config2)
print(f"  ├─ Patch Config: {patch_config2}")
print("  └─ ✓ Additional fields added successfully")

# ------------------------------------------------------------
# Step 8: Patch Configuration (Update Existing Field)
# ------------------------------------------------------------
print("\n[Step 8] Patch Configuration (Update Existing Field)")
patch_config3 = {
    "timeout": 90
}
result3 = service_proxy.patch_config(patch_config3)
final_info = service_proxy.service_info()
final_config = final_info.get('config', {})
print(f"  ├─ Patch Config: {patch_config3}")
print(f"  ├─ Final Config: {final_config}")
print("  └─ ✓ Existing field updated successfully")

# ------------------------------------------------------------
# Step 9: Verify Service Still Available
# ------------------------------------------------------------
print("\n[Step 9] Verify Service Still Available")
store.for_store().wait_service("mcpstore-wiki", timeout=30.0)
tools = service_proxy.list_tools()
print(f"  ├─ Tools Available: {len(tools)}")
print("  └─ ✓ Service verification completed")

# ------------------------------------------------------------
# Step 10: Reset Configuration (Final Cleanup)
# ------------------------------------------------------------
print("\n[Step 10] Reset Configuration (Final Cleanup)")
store.for_store().reset_config()
print("  └─ ✓ Configuration reset successfully")

# ============================================================
print("\n" + "=" * 60)
print("  Service Configuration Incremental Update Completed")
print("=" * 60)
print()

