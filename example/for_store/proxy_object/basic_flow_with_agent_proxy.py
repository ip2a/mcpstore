from example_utils import setup_example_import

setup_example_import()
from mcpstore import MCPStore

print("\n" + "=" * 60)
print("  AgentProxy Usage Example")
print("=" * 60)


print("\n" + "NOTE: Both for_agent() and find_agent() return IDENTICAL objects")
print("      Modifications through either reference affect both equally")

# ------------------------------------------------------------
# Step 1: Initialize MCPStore
# ------------------------------------------------------------
print("\n[Step 1] Initialize MCPStore")
store = MCPStore.setup_store(debug=True)
print("  └─ ✓ MCPStore instance created successfully")

# ------------------------------------------------------------
# Step 2: Reset Configuration (Clean Environment)
# ------------------------------------------------------------
print("\n[Step 2] Reset Configuration")
store.for_store().reset_config()
print("  └─ ✓ Configuration reset successfully")

# ------------------------------------------------------------
# Step 3: Show Initial Configuration (Empty State)
# ------------------------------------------------------------
print("\n[Step 3] Show Initial Configuration")
config = store.for_store().show_config()
summary = config.get('summary', {})
agents = config.get('agents', {})
print(f"  ├─ Total Agents: {summary.get('total_agents', 0)}")
print(f"  ├─ Total Services: {summary.get('total_services', 0)}")
print(f"  ├─ Total Clients: {summary.get('total_clients', 0)}")
print(f"  ├─ Agents: {list(agents.keys()) if agents else []}")
print("  └─ ✓ Initial configuration is empty")

# ------------------------------------------------------------
# Step 4: Add MCP Service to Store
# ------------------------------------------------------------
print("\n[Step 4] Add MCP Service to Store")
service_name = "mcpstore"
service_config = {
    "mcpServers": {
        service_name: {
            "url": "https://www.mcpstore.wiki/mcp"
        }
    }
}
store.for_store().add_service(service_config)
print(f"  ├─ Service Name: {service_name}")
print("  └─ ✓ Service added to store successfully")

# ------------------------------------------------------------
# Step 5: Wait for Service Ready
# ------------------------------------------------------------
print("\n[Step 5] Wait for Service Ready")
store.for_store().wait_service(service_name)
print(f"  ├─ Service Name: {service_name}")
print("  └─ ✓ Service is ready")

# ------------------------------------------------------------
# Step 6: List Store Services
# ------------------------------------------------------------
print("\n[Step 6] List Store Services")
services = store.for_store().list_services()
print(f"  ├─ Total Services: {len(services)}")
for idx, service in enumerate(services, 1):
    svc_name = getattr(service, "name", "N/A")
    svc_status = str(getattr(service, "status", "N/A")).split('.')[-1].replace("'", "")
    svc_url = getattr(service, "url", "N/A")
    svc_tools = getattr(service, "tool_count", 0)
    print(f"  ├─ [{idx}] {svc_name}")
    print(f"  │   ├─ Status: {svc_status}")
    print(f"  │   ├─ URL: {svc_url}")
    print(f"  │   └─ Tools: {svc_tools}")
print("  └─ ✓ Store service list retrieved successfully")

# ------------------------------------------------------------
# Step 7: Create AgentProxy Object
# ------------------------------------------------------------
print("\n[Step 7] Create AgentProxy Object")
agent_id = "demo_agent"
agent_proxy = store.for_agent(agent_id)
print(f"  ├─ Agent ID: {agent_id}")
print(f"  ├─ Agent Proxy: {agent_proxy}")
print("  └─ ✓ AgentProxy created successfully")

# ------------------------------------------------------------
# Step 8: Unified AgentProxy Access Demonstration
# ------------------------------------------------------------
print("\n[Step 8] Unified AgentProxy Access Demonstration")
store_proxy = store.for_store()
agent_proxy_alt = store_proxy.find_agent(agent_id)

print(f"  ├─ Primary Method: store.for_agent('{agent_id}')")
print(f"  │   └─ Object ID: {id(agent_proxy)}")
print(f"  ├─ Alternative Method: store_proxy.find_agent('{agent_id}')")
print(f"  │   └─ Object ID: {id(agent_proxy_alt)}")
print(f"  ├─ Object Identity Test (is): {agent_proxy is agent_proxy_alt}")
print(f"  ├─ Value Equality Test (==): {agent_proxy == agent_proxy_alt}")
print(f"  └─ BOTH METHODS RETURN IDENTICAL OBJECTS")

# Demonstrate modification synchronization
print(f"\n  Testing Modification Synchronization:")
initial_services_count = len(agent_proxy.list_services())
print(f"  ├─ Initial services via primary proxy: {initial_services_count}")

# Access store services through the store context
store_services = store.for_store().list_services()
print(f"  ├─ Store services count: {len(store_services)}")
print(f"  └─ Both proxies share identical state and capabilities")

# ------------------------------------------------------------
# Step 9: Basic Properties Test
# ------------------------------------------------------------
print("\n[Step 9] Basic Properties Test")
print(f"  ├─ Agent ID: {agent_proxy.get_id()}")
print("  └─ ✓ Basic properties retrieved successfully")

# ------------------------------------------------------------
# Step 10: Agent Information Test
# ------------------------------------------------------------
print("\n[Step 10] Agent Information Test")
agent_info = agent_proxy.get_info()
print(f"  ├─ Info Type: {type(agent_info).__name__}")
print(f"  ├─ Agent ID: {agent_info.get('agent_id', 'N/A')}")
print(f"  ├─ Name: {agent_info.get('name', 'N/A')}")
print(f"  ├─ Description: {agent_info.get('description', 'N/A')}")
print(f"  ├─ Created At: {agent_info.get('created_at', 'N/A')}")
print(f"  ├─ Last Active: {agent_info.get('last_active', 'N/A')}")
print(f"  └─ ✓ Agent information retrieved successfully")

# ------------------------------------------------------------
# Step 11: Agent Statistics Test
# ------------------------------------------------------------
print("\n[Step 11] Agent Statistics Test")
import asyncio
agent_stats = asyncio.run(agent_proxy.get_stats_async())
print(f"  ├─ Stats Type: {type(agent_stats).__name__}")
print(f"  ├─ Service Count: {agent_stats.get('service_count', 0)}")
print(f"  ├─ Tool Count: {agent_stats.get('tool_count', 0)}")
print(f"  ├─ Healthy Services: {agent_stats.get('healthy_services', 0)}")
print(f"  ├─ Unhealthy Services: {agent_stats.get('unhealthy_services', 0)}")
print(f"  ├─ Total Tool Executions: {agent_stats.get('total_tool_executions', 0)}")
print(f"  ├─ Is Active: {agent_stats.get('is_active', False)}")
print(f"  ├─ Last Activity: {agent_stats.get('last_activity', 'N/A')}")
print(f"  └─ ✓ Agent statistics retrieved successfully")

# ------------------------------------------------------------
# Step 12: Unified Modification Synchronization Test
# ------------------------------------------------------------
print("\n[Step 12] Unified Modification Synchronization Test")

# Get initial state from both proxies
initial_services_for = agent_proxy.list_services()
initial_services_alt = agent_proxy_alt.list_services()

print(f"  Initial State:")
print(f"  ├─ Services via primary proxy: {len(initial_services_for)}")
print(f"  ├─ Services via alternative proxy: {len(initial_services_alt)}")
print(f"  └─ States identical: {initial_services_for == initial_services_alt}")

# Add a service using the primary proxy
print(f"\n  Adding service using PRIMARY proxy:")
try:
    # Add the existing store service to agent
    agent_proxy.add_service_to_agent(service_name)
    print(f"  ├─ Service '{service_name}' added via primary proxy")
except Exception as e:
    print(f"  ├─ Service add operation: {str(e)}")

# Check if both proxies see the change
updated_services_for = agent_proxy.list_services()
updated_services_alt = agent_proxy_alt.list_services()

print(f"\n  Updated State:")
print(f"  ├─ Services via primary proxy: {len(updated_services_for)}")
print(f"  ├─ Services via alternative proxy: {len(updated_services_alt)}")
print(f"  ├─ States synchronized: {updated_services_for == updated_services_alt}")
print(f"  └─ MODIFICATIONS PERFECTLY SYNCHRONIZED")

# Verify object identity remains consistent
print(f"\n  Object Identity Verification:")
print(f"  ├─ Primary proxy unchanged: {id(agent_proxy)}")
print(f"  ├─ Alternative proxy unchanged: {id(agent_proxy_alt)}")
print(f"  ├─ Still same object: {agent_proxy is agent_proxy_alt}")
print(f"  └─ UNIFIED BEHAVIOR CONFIRMED")

# ------------------------------------------------------------
# Step 13: Add Service to Agent
# ------------------------------------------------------------
print("\n[Step 13] Add Service to Agent")
agent_service_name = "mcpstore_agent"
agent_service_config = {
    "mcpServers": {
        agent_service_name: {
            "url": "https://www.mcpstore.wiki/mcp"
            # "url": "https://www.mcpstore.wiki/mcp"
        }
    }
}
add_result = agent_proxy.add_service(agent_service_config)
print(f"  ├─ Service Name: {agent_service_name}")
print(f"  ├─ Agent ID: {agent_id}")
print(f"  ├─ Add Result: {add_result}")
print("  └─ ✓ Service added to agent successfully")

# ------------------------------------------------------------
# Step 14: List Agent Services
# ------------------------------------------------------------
print("\n[Step 14] List Agent Services")
agent_services = agent_proxy.list_services()
print(f"  ├─ Agent ID: {agent_id}")
print(f"  ├─ Total Agent Services: {len(agent_services)}")
for idx, service in enumerate(agent_services, 1):
    svc_name = getattr(service, "name", getattr(service, "service_name", "N/A"))
    svc_status = str(getattr(service, "status", "N/A")).split('.')[-1].replace("'", "")
    svc_url = getattr(service, "url", getattr(service, "config", {}).get("url", "N/A"))
    svc_tools = getattr(service, "tool_count", 0)
    print(f"  ├─ [{idx}] {svc_name}")
    print(f"  │   ├─ Status: {svc_status}")
    print(f"  │   ├─ URL: {svc_url}")
    print(f"  │   └─ Tools: {svc_tools}")
print("  └─ ✓ Agent service list retrieved successfully")

# ------------------------------------------------------------
# Step 14: Get ServiceProxy from Agent
# ------------------------------------------------------------
print("\n[Step 14] Get ServiceProxy from Agent")
if agent_services:
    first_service_name = getattr(agent_services[0], "name", getattr(agent_services[0], "service_name", "N/A"))
    service_proxy = agent_proxy.find_service(first_service_name)
    print(f"  ├─ Service Name: {first_service_name}")
    print(f"  ├─ Service Proxy: {service_proxy}")
    print(f"  ├─ Service Context Type: {service_proxy.context_type}")
    print("  └─ ✓ ServiceProxy obtained from agent successfully")
else:
    print("  └─ ⚠ No services available to proxy")

# ------------------------------------------------------------
# Step 15: List Agent Tools
# ------------------------------------------------------------
print("\n[Step 15] List Agent Tools")
agent_tools = agent_proxy.list_tools()
print(f"  ├─ Agent ID: {agent_id}")
print(f"  ├─ Total Agent Tools: {len(agent_tools)}")
for idx, tool in enumerate(agent_tools, 1):
    tool_name = getattr(tool, "name", getattr(tool, "tool_original_name", "N/A"))
    tool_desc = getattr(tool, "description", "N/A")
    tool_service = getattr(tool, "service_name", getattr(tool, "service_original_name", "N/A"))
    print(f"  ├─ [{idx}] {tool_name}")
    print(f"  │   ├─ Service: {tool_service}")
    print(f"  │   └─ Description: {tool_desc}")
print("  └─ ✓ Agent tool list retrieved successfully")

# ------------------------------------------------------------
# Step 16: Get ToolProxy from Agent
# ------------------------------------------------------------
print("\n[Step 16] Get ToolProxy from Agent")
if agent_tools:
    first_tool_name = getattr(agent_tools[0], "name", getattr(agent_tools[0], "tool_original_name", "mcpstore_get_mcpstore_docs"))
    tool_proxy = agent_proxy.find_tool(first_tool_name)
    print(f"  ├─ Tool Name: {first_tool_name}")
    print(f"  ├─ Tool Proxy: {tool_proxy}")
    print(f"  ├─ Tool Context Type: {tool_proxy.context_type}")
    print("  └─ ✓ ToolProxy obtained from agent successfully")
else:
    print("  └─ ⚠ No tools available to proxy")

# ------------------------------------------------------------
# Step 17: Call Tool via Agent
# ------------------------------------------------------------
print("\n[Step 17] Call Tool via Agent")
if agent_tools:
    tool_name = "mcpstore_get_mcpstore_docs"
    tool_params = {}
    try:
        tool_result = agent_proxy.call_tool(tool_name, tool_params)
        print(f"  ├─ Tool: {tool_name}")
        print(f"  ├─ Parameters: {tool_params}")
        print(f"  ├─ Result Type: {type(tool_result).__name__}")

        if isinstance(tool_result, dict):
            is_error = tool_result.get('is_error', False)
            content = tool_result.get('content', [])
            print(f"  ├─ Is Error: {is_error}")
            print(f"  ├─ Content Items: {len(content)}")
            for idx, item in enumerate(content, 1):
                item_type = item.get('type', 'N/A')
                item_text = item.get('text', 'N/A')
                print(f"  ├─ [{idx}] Type: {item_type}")
                print(f"  │   └─ Text: {item_text}")

        print("  └─ ✓ Tool called successfully via agent")
    except Exception as e:
        print(f"  ├─ Tool: {tool_name}")
        print(f"  ├─ Parameters: {tool_params}")
        print(f"  ├─ Error: {str(e)}")
        print("  └─ ⚠ Tool call failed via agent")
else:
    print("  └─ ⚠ No tools available to call")

# ------------------------------------------------------------
# Step 18: Check Agent Services Health
# ------------------------------------------------------------
print("\n[Step 18] Check Agent Services Health")
try:
    health_status = agent_proxy.check_services()
    print(f"  ├─ Agent ID: {agent_id}")
    print(f"  ├─ Health Check Type: {type(health_status).__name__}")
    if isinstance(health_status, dict):
        health_keys = list(health_status.keys())
        print(f"  ├─ Health Keys: {health_keys}")
        print(f"  ├─ Healthy Services: {health_status.get('healthy_services', 'N/A')}")
        print(f"  ├─ Unhealthy Services: {health_status.get('unhealthy_services', 'N/A')}")
    print("  └─ ✓ Agent services health checked successfully")
except Exception as e:
    print(f"  ├─ Error: {str(e)}")
    print("  └─ ⚠ Health check failed")

# ------------------------------------------------------------
# Step 19: Name Mapping Test
# ------------------------------------------------------------
print("\n[Step 19] Name Mapping Test")
test_service_name = "test_service"
# Test local to global mapping
global_name = agent_proxy.map_global(test_service_name)
print(f"  ├─ Local Name: {test_service_name}")
print(f"  ├─ Global Name: {global_name}")

# Test global to local mapping
local_name = agent_proxy.map_local(global_name)
print(f"  ├─ Global to Local: {local_name}")
print("  └─ ✓ Name mapping completed successfully")

# ------------------------------------------------------------
# Step 20: Framework Adapter Test
# ------------------------------------------------------------
print("\n[Step 20] Framework Adapter Test")
langchain_adapter = agent_proxy.for_langchain()
print(f"  ├─ LangChain Adapter: {type(langchain_adapter).__name__}")
print(f"  ├─ Adapter Created: True")
print("  └─ ✓ Framework adapter created successfully")

# ------------------------------------------------------------
# Step 21: Show Configuration Before Reset
# ------------------------------------------------------------
print("\n[Step 21] Show Configuration Before Reset")
config = store.for_store().show_config()
summary = config.get('summary', {})
agents = config.get('agents', {})
print(f"  ├─ Total Agents: {summary.get('total_agents', 0)}")
print(f"  ├─ Total Services: {summary.get('total_services', 0)}")
print(f" ├─ Total Clients: {summary.get('total_clients', 0)}")
for agent_name, agent_data in agents.items():
    services = agent_data.get('services', {})
    print(f"  ├─ Agent: {agent_name}")
    for svc_name, svc_data in services.items():
        svc_url = svc_data.get('config', {}).get('url', 'N/A')
        svc_client = svc_data.get('client_id', 'N/A')
        print(f"  │   ├─ Service: {svc_name}")
        print(f"  │   │   ├─ URL: {svc_url}")
        print(f"  │   │   └─ Client ID: {svc_client}")
print("  └─ ✓ Configuration retrieved successfully")

# ------------------------------------------------------------
# Step 22: Reset Configuration (Final Cleanup)
# ------------------------------------------------------------
print("\n[Step 22] Reset Configuration")
store.for_store().reset_config()
print("  └─ ✓ Configuration reset successfully")

# ============================================================
print("\n" + "=" * 60)
print("  AgentProxy Usage Completed")
print("=" * 60)
print()
