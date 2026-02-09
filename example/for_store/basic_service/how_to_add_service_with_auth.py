#!/usr/bin/env python3
"""
MCPStore Service Authentication Configuration Example
Demonstrates how to configure various authentication information when adding services,
including Bearer Token, API Key, and custom request headers
Supports combination of multiple authentication methods
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
# MCPStore Service Authentication Configuration Example
# ============================================================

print("\n" + "=" * 60)
print("  MCPStore Service Authentication Configuration Example")
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
# Step 3: Add Service with Bearer Token (Method 1)
# ------------------------------------------------------------
print("\n[Step 3] Add Service with Bearer Token (Method 1)")
store.for_store().add_service(
    {"name": "auth-api-1", "url": "https://api.example.com/mcp"},
    auth="bearer-token-123"
)
print("  ├─ Service Name: auth-api-1")
print("  ├─ Auth Type: Bearer Token")
print("  └─ ✓ Service added successfully")

# ------------------------------------------------------------
# Step 4: Add Service with Custom Headers (Method 2)
# ------------------------------------------------------------
print("\n[Step 4] Add Service with Custom Headers (Method 2)")
store.for_store().add_service(
    {"name": "api-key-service", "url": "https://api.example.com/mcp"},
    headers={"X-API-Key": "api-key-456", "Authorization": "Custom token"}
)
print("  ├─ Service Name: api-key-service")
print("  ├─ Auth Type: Custom Headers")
print("  └─ ✓ Service added successfully")

# ------------------------------------------------------------
# Step 5: Add Service with Auth in Config (Method 3)
# ------------------------------------------------------------
print("\n[Step 5] Add Service with Auth in Config (Method 3)")
store.for_store().add_service({
    "name": "config-auth-service",
    "url": "https://api.example.com/mcp",
    "auth": "config-token-789"
})
print("  ├─ Service Name: config-auth-service")
print("  ├─ Auth Type: Config-based")
print("  └─ ✓ Service added successfully")

# ------------------------------------------------------------
# Step 6: Add Service with Combined Auth and Headers (Method 4)
# ------------------------------------------------------------
print("\n[Step 6] Add Service with Combined Auth and Headers (Method 4)")
store.for_store().add_service(
    {"name": "full-auth-service", "url": "https://api.example.com/mcp"},
    auth="combined-token",
    headers={"X-Custom": "header-value"}
)
print("  ├─ Service Name: full-auth-service")
print("  ├─ Auth Type: Combined (auth + headers)")
print("  └─ ✓ Service added successfully")

# ------------------------------------------------------------
# Step 7: Verify Configuration Saved
# ------------------------------------------------------------
print("\n[Step 7] Verify Configuration Saved")
config = store.config.load_config()
servers = config.get("mcpServers", {})
print(f"  ├─ Services Configured: {len(servers)}")
print("  └─ ✓ Configuration verified")

# ------------------------------------------------------------
# Step 8: Reset Configuration (Final Cleanup)
# ------------------------------------------------------------
print("\n[Step 8] Reset Configuration (Final Cleanup)")
store.for_store().reset_config()
print("  └─ ✓ Configuration reset successfully")

# ============================================================
print("\n" + "=" * 60)
print("  Service Authentication Configuration Example Completed")
print("=" * 60)
print()

