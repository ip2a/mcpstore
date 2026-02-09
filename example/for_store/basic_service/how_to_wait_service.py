"""
Wait for Specific Service Status Example
Demonstrates wait_service() status parameter, waiting for service to reach specified health status
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
# Wait for Specific Service Status Example
# ============================================================

print("\n" + "=" * 60)
print("  MCPStore Wait Service Status Example")
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
# Step 3: Add Demo Service
# ------------------------------------------------------------
print("\n[Step 3] Add Demo Service")
demo_mcp = {
    "mcpServers": {
        "mcpstore-demo-weather": {
            "url": "https://mcpstore.wiki/mcp"
        }
    }
}
store.for_store().add_service(demo_mcp)
print("  ├─ Service Name: mcpstore-demo-weather")
print("  └─ ✓ Service added successfully")

# ------------------------------------------------------------
# Step 4: Wait for Service to Reach Healthy Status
# ------------------------------------------------------------
print("\n[Step 4] Wait for Service to Reach Healthy Status")
store.for_store().wait_service("mcpstore-demo-weather", status="healthy", timeout=10)
print("  ├─ Service Name: mcpstore-demo-weather")
print("  ├─ Target Status: healthy")
print("  └─ ✓ Service reached healthy status")

# ------------------------------------------------------------
# Step 5: Wait for Service to Reach Healthy or Warning Status
# ------------------------------------------------------------
print("\n[Step 5] Wait for Service to Reach Healthy or Warning Status")
store.for_store().wait_service("mcpstore-demo-weather", status=["healthy", "warning"], timeout=10)
print("  ├─ Service Name: mcpstore-demo-weather")
print("  ├─ Target Status: healthy or warning")
print("  └─ ✓ Service reached target status")

# ------------------------------------------------------------
# Step 6: Reset Configuration (Final Cleanup)
# ------------------------------------------------------------
print("\n[Step 6] Reset Configuration (Final Cleanup)")
store.for_store().reset_config()
print("  └─ ✓ Configuration reset successfully")

# ============================================================
print("\n" + "=" * 60)
print("  Wait Service Status Example Completed")
print("=" * 60)
print()

