#!/usr/bin/env python3
"""
Authentication example: auth isolation in agent mode
Shows how multiple agents can each hold their own auth configuration
"""

from mcpstore import MCPStore

print("=" * 60)
print(" Auth isolation example (agent mode)")
print("=" * 60)

# Initialize the store
store = MCPStore.setup_store(debug=False)

# Agent 1 adds an authenticated service (fully chained)
print("\n Agent 1 adds an authenticated service")
store.for_agent("team1").add_service(
    {"name": "shared-api", "url": "https://api.example.com/mcp"}, 
    auth="team1-token"
)
print("   team1 service added (using auth)")

# Agent 2 adds a same-named service with different auth (fully chained)
print("\n Agent 2 adds a same-named service with different auth")
store.for_agent("team2").add_service(
    {"name": "shared-api", "url": "https://api.example.com/mcp"}, 
    headers={"X-Team": "team2", "Authorization": "Bearer team2-token"}
)
print("   team2 service added (using headers)")

# Agent 3 adds the service first, then chains an auth update (fully chained)
print("\n Agent 3 adds first, then chains an auth update")
store.for_agent("team3").add_service({"name": "shared-api", "url": "https://api.example.com/mcp"})
store.for_agent("team3").update_service("shared-api").bearer_auth(auth="team3-updated-token")
print("   team3 service added and updated")

# Verify service isolation across the three agents
print("\n📋 Verifying agent auth isolation:")
config = store.config.load_config()
servers = config.get("mcpServers", {})
print(f"\n   team1 actual service name: shared-api_byagent_team1")
print(f"   team1 auth: {servers['shared-api_byagent_team1'].get('auth')}")
print(f"   team1 has headers: {'headers' in servers['shared-api_byagent_team1']}")

print(f"\n   team2 actual service name: shared-api_byagent_team2")
print(f"   team2 headers: {servers['shared-api_byagent_team2'].get('headers')}")
print(f"   team2 has auth: {'auth' in servers['shared-api_byagent_team2']}")

print(f"\n   team3 actual service name: shared-api_byagent_team3")
print(f"   team3 auth: {servers['shared-api_byagent_team3'].get('auth')}")

print("\n💡 Auth isolation highlights:")
print("   - Each agent holds an independent auth configuration")
print("   - Same-named services are fully isolated between agents")
print("   - No interference; each is managed independently")

print("\n" + "=" * 60)
print(" Agent auth isolation example complete!")
print("=" * 60)

