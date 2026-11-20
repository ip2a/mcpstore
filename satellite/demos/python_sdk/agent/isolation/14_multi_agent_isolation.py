#!/usr/bin/env python3
"""
Example 14: Multi-agent data isolation
Purpose: demonstrates complete data isolation between agents
Highlight: collection mapping with no interference between agents
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent))
from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore

print("=" * 60)
print("Example 14: Multi-agent data isolation")
print("=" * 60)

# Initialize the store
store = MCPStore.setup_store(
    mcpjson_path="./mcp.json",
    debug=False
)

print("\n Store initialized successfully")

# Agent 1: add services
print("\n" + "=" * 60)
print("Agent 1 operations")
print("=" * 60)

store.for_agent("agent_001").add_service({
    "mcpServers": {
        "weather": {
            "command": "echo",
            "args": ["weather"]
        }
    }
})
print(" Agent 1 added the weather service")

store.for_agent("agent_001").add_service({
    "mcpServers": {
        "calendar": {
            "command": "echo",
            "args": ["calendar"]
        }
    }
})
print(" Agent 1 added the calendar service")

services_1 = store.for_agent("agent_001").list_services()
print(f"\n📋 Agent 1 services: {len(services_1)}")
for service in services_1:
    print(f"   - {service}")

# Agent 2: add different services
print("\n" + "=" * 60)
print("Agent 2 operations")
print("=" * 60)

store.for_agent("agent_002").add_service({
    "mcpServers": {
        "database": {
            "command": "echo",
            "args": ["database"]
        }
    }
})
print(" Agent 2 added the database service")

store.for_agent("agent_002").add_service({
    "mcpServers": {
        "email": {
            "command": "echo",
            "args": ["email"]
        }
    }
})
print(" Agent 2 added the email service")

services_2 = store.for_agent("agent_002").list_services()
print(f"\n📋 Agent 2 services: {len(services_2)}")
for service in services_2:
    print(f"   - {service}")

# Agent 3: add more services
print("\n" + "=" * 60)
print("Agent 3 operations")
print("=" * 60)

store.for_agent("agent_003").add_service({
    "mcpServers": {
        "filesystem": {
            "command": "echo",
            "args": ["filesystem"]
        }
    }
})
print(" Agent 3 added the filesystem service")

services_3 = store.for_agent("agent_003").list_services()
print(f"\n📋 Agent 3 services: {len(services_3)}")
for service in services_3:
    print(f"   - {service}")

# Verify isolation
print("\n" + "=" * 60)
print("Verifying data isolation")
print("=" * 60)

print("\nChecking data isolation between agents:")
print(f"   Agent 1 services: {services_1}")
print(f"   Agent 2 services: {services_2}")
print(f"   Agent 3 services: {services_3}")

# Verify there is no overlap
if (set(services_1) & set(services_2)) == set():
    print("\n Agent 1 and Agent 2 data fully isolated")
else:
    print("\n⚠️  Warning: Agent 1 and Agent 2 share data")

if (set(services_2) & set(services_3)) == set():
    print(" Agent 2 and Agent 3 data fully isolated")
else:
    print("⚠️  Warning: Agent 2 and Agent 3 share data")

if (set(services_1) & set(services_3)) == set():
    print(" Agent 1 and Agent 3 data fully isolated")
else:
    print("⚠️  Warning: Agent 1 and Agent 3 share data")

# Store-level services
print("\n" + "=" * 60)
print("Store level (global)")
print("=" * 60)

store.for_store().add_service({
    "mcpServers": {
        "global_service": {
            "command": "echo",
            "args": ["global"]
        }
    }
})
print(" Store added global_service")

store_services = store.for_store().list_services()
print(f"\n📋 Store services: {len(store_services)}")
for service in store_services:
    print(f"   - {service}")

# Verify store/agent isolation
if (set(store_services) & set(services_1)) == set():
    print("\n Store and agent data fully isolated")
else:
    print("\n⚠️  Warning: the store and agents share data")

# Cleanup
store.for_agent("agent_001").reset_config()
store.for_agent("agent_002").reset_config()
store.for_agent("agent_003").reset_config()
store.for_store().reset_config()
print("\n All configurations reset")

print("\n" + "=" * 60)
print("How data isolation works:")
print("=" * 60)
print("\n📦 Collection mapping strategy:")
print("   Store:    agent:global_agent_store:tools")
print("   Agent 1:  agent:agent_001:tools")
print("   Agent 2:  agent:agent_002:tools")
print("   Agent 3:  agent:agent_003:tools")
print("\n Each agent uses its own collection")
print(" Data is fully isolated at the storage layer")
print(" No interference; safe and reliable")

print("\n💡 Use cases:")
print("   - Multi-tenant SaaS applications")
print("   - Multi-user isolation")
print("   - Development/test environment isolation")
print("   - Isolation across business lines")

