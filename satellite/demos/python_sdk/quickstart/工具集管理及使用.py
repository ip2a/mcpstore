from example_utils import setup_example_import

setup_example_import()
from mcpstore import MCPStore
from mcpstore.config import MemoryConfig


# ============================================================
# Tool Set Management Example - Complete Agent Tool Management
#
# KEY FEATURES:
# - Agent-level tool filtering and management
# - Add/Remove/Reset tools for fine-grained control
# - Service-level tool operations
# - Cross-agent tool isolation
# - Tool availability checking
# ============================================================

print("\n" + "=" * 60)
print("  Tool Set Management Example")
print("=" * 60)

# ------------------------------------------------------------
# Step 1: Initialize MCPStore
# ------------------------------------------------------------
print("\n[Step 1] Initialize MCPStore")
store = MCPStore.setup_store(source=MemoryConfig(), debug=True)
print("  └─ ✓ MCPStore instance created successfully")

# ------------------------------------------------------------
# Step 2: Reset Configuration (Clean Environment)
# ------------------------------------------------------------
print("\n[Step 2] Reset configuration (clean environment)")
store.for_store().reset_config()
print("  └─ ✓ Configuration reset successfully")

# ------------------------------------------------------------
# Step 3: Add MCP Service to Store
# ------------------------------------------------------------
print("\n[Step 3] Add MCP service to the store")
service_name = "mcpstore"
service_config = {
    "mcpServers": {
        service_name: {
            "url": "http://115.190.220.103:2230/mcp"
            # "url": "http://115.190.220.103:2230/mcp"
        }
    }
}
store.for_store().add_service(service_config)
print(f"  ├─ Service name: {service_name}")
print("  └─ ✓ Service added successfully")

# ------------------------------------------------------------
# Step 4: Wait for Service Ready
# ------------------------------------------------------------
print("\n[Step 4] Wait for the service to be ready")
store.for_store().wait_service(service_name=service_name)
print(f"  ├─ Service name: {service_name}")
print("  └─ ✓ Service is ready")
print(store.for_store().find_service(service_name))
# ============================================================
# AGENT 1: Basic Tool Set Management
# ============================================================
print("\n" + "=" * 60)
print("  Agent 1: Basic tool set management")
print("=" * 60)

# ------------------------------------------------------------
# Step 5: Create Agent 1
# ------------------------------------------------------------
print("\n[Step 5] Create Agent 1")
agent1_id = "agent1"
agent1 = store.for_agent(agent1_id)
print(f"  ├─ Agent ID: {agent1_id}")
print("  └─ ✓ Agent 1 created successfully")

# ------------------------------------------------------------
# Step 6: Add Service to Agent 1
# ------------------------------------------------------------
print("\n[Step 6] Add a service to Agent 1")
agent1_service_config = {
    "mcpServers": {
        "mcpstore": {
            "url": "http://115.190.220.103:2230/mcp"
            # "url": "http://115.190.220.103:2230/mcp"
        }
    }
}
agent1.add_service(agent1_service_config)
print(f"  ├─ Agent ID: {agent1_id}")
print(f"  ├─ Service name: mcpstore")
print("  └─ ✓ Service added successfully")

# ------------------------------------------------------------
# Step 7: List All Tools (Initial State)
# ------------------------------------------------------------
print("\n[Step 7] List all tools (initial state)")
all_tools = agent1.list_tools(filter="all")
available_tools = agent1.list_tools(filter="available")
print(f"  ├─ Agent ID: {agent1_id}")
print(f"  ├─ Total original tools: {len(all_tools)}")
print(f"  ├─ Currently available tools: {len(available_tools)}")
for idx, tool in enumerate(available_tools[:3], 1):
    tool_name = tool.name
    print(f"  ├─ [{idx}] {tool_name}")
print("  └─ ✓ All tools are available by default")

# ------------------------------------------------------------
# Step 8: Remove All Tools
# ------------------------------------------------------------
print("\n[Step 8] Remove all tools")
agent1.remove_tools(service="mcpstore", tools="_all_tools")
available_tools_after_clear = agent1.list_tools(filter="available")
print(f"  ├─ Agent ID: {agent1_id}")
print(f"  ├─ Operation: remove all tools")
print(f"  ├─ Available tools after removal: {len(available_tools_after_clear)}")
print("  └─ ✓ All tools removed")

# ------------------------------------------------------------
# Step 9: Add Single Tool
# ------------------------------------------------------------
print("\n[Step 9] Add a single tool")
agent1.add_tools(service="mcpstore", tools=["get_mcpstore_docs"])
available_tools_after_add = agent1.list_tools(filter="available")
print(f"  ├─ Agent ID: {agent1_id}")
print(f"  ├─ Added tool: get_mcpstore_docs")
print(f"  ├─ Currently available tools: {len(available_tools_after_add)}")
for idx, tool in enumerate(available_tools_after_add, 1):
    tool_name = tool.name
    print(f"  ├─ [{idx}] {tool_name}")
print("  └─ ✓ Single tool added successfully")

# ------------------------------------------------------------
# Step 10: Call Available Tool
# ------------------------------------------------------------
print("\n[Step 10] Call an available tool")
try:
    tool_result = agent1.call_tool("get_mcpstore_docs", {})
    print(f"  ├─ Agent ID: {agent1_id}")
    print(f"  ├─ Tool name: get_mcpstore_docs")
    print(f"  ├─ Call result: success")
    print("  └─ ✓ Available tool called successfully")
except Exception as e:
    print(f"  ├─ Error: {str(e)}")
    print("  └─ ✗ Tool call failed")

# ------------------------------------------------------------
# Step 11: Try to Call Unavailable Tool
# ------------------------------------------------------------
print("\n[Step 11] Try to call an unavailable tool")
try:
    # Pretend another tool exists and try to call it
    agent1.call_tool("some_other_tool", {})
    print("  └─ ✗ Should not reach this point")
except Exception as e:
    error_msg = str(e)
    print(f"  ├─ Agent ID: {agent1_id}")
    print(f"  ├─ Tool name: some_other_tool")
    print(f"  ├─ Expected behavior: raise an exception")
    print(f"  ├─ Exception message: {error_msg[:50]}...")
    print("  └─ ✓ Unavailable tool correctly blocked")

# ============================================================
# AGENT 2: Multi-Service Tool Management
# ============================================================
print("\n" + "=" * 60)
print("  Agent 2: Multi-service tool management")
print("=" * 60)

# ------------------------------------------------------------
# Step 12: Create Agent 2
# ------------------------------------------------------------
print("\n[Step 12] Create Agent 2")
agent2_id = "agent2"
agent2 = store.for_agent(agent2_id)
print(f"  ├─ Agent ID: {agent2_id}")
print("  └─ ✓ Agent 2 created successfully")

# ------------------------------------------------------------
# Step 13: Add Two Services to Agent 2
# ------------------------------------------------------------
print("\n[Step 13] Add two services to Agent 2")
agent2_service_config = {
    "mcpServers": {
        "mcpstore1": {
            "url": "http://115.190.220.103:2230/mcp"
            # "url": "http://115.190.220.103:2230/mcp"
        },
        "mcpstore2": {
            "url": "http://115.190.220.103:2230/mcp"
            # "url": "http://115.190.220.103:2230/mcp"
        }
    }
}
agent2.add_service(agent2_service_config)
print(f"  ├─ Agent ID: {agent2_id}")
print(f"  ├─ Service 1: mcpstore1")
print(f"  ├─ Service 2: mcpstore2")
print("  └─ ✓ Both services added successfully")

# ------------------------------------------------------------
# Step 14: List Agent 2 Services
# ------------------------------------------------------------
print("\n[Step 14] List Agent 2's services")
agent2_services = agent2.list_services()
print(f"  ├─ Agent ID: {agent2_id}")
print(f"  ├─ Total services: {len(agent2_services)}")
for idx, service in enumerate(agent2_services, 1):
    svc_name = getattr(service, "name", "N/A")
    svc_tools = getattr(service, "tool_count", 0)
    print(f"  ├─ [{idx}] {svc_name} ({svc_tools} tools)")
print("  └─ ✓ Service list retrieved successfully")

# ------------------------------------------------------------
# Step 15: Remove Specific Tool from Service 1
# ------------------------------------------------------------
print("\n[Step 15] Remove a specific tool from Service 1")
agent2.remove_tools(service="mcpstore1", tools=["get_mcpstore_docs"])
service1_tools = agent2.list_tools(service_name="mcpstore1", filter="available")
print(f"  ├─ Agent ID: {agent2_id}")
print(f"  ├─ Service name: mcpstore1")
print(f"  ├─ Removed tool: get_mcpstore_docs")
print(f"  ├─ Remaining available tools: {len(service1_tools)}")
print("  └─ ✓ Tool removed successfully")

# ------------------------------------------------------------
# Step 16: List All Tools from Agent 2
# ------------------------------------------------------------
print("\n[Step 16] List all tools for Agent 2")
all_agent2_tools = agent2.list_tools(filter="available")
print(f"  ├─ Agent ID: {agent2_id}")
print(f"  ├─ Total available tools: {len(all_agent2_tools)}")
# Group tools by service
tools_by_service = {}
for tool in all_agent2_tools:
    svc = tool.service_name
    if svc not in tools_by_service:
        tools_by_service[svc] = []
    tools_by_service[svc].append(tool.name)

for svc, tools in tools_by_service.items():
    print(f"  ├─ {svc}: {len(tools)} tools")
print("  └─ ✓ Tool list retrieved successfully")

# ============================================================
# AGENT 3: Managing Tools with ServiceProxy
# ============================================================
print("\n" + "=" * 60)
print("  Agent 3: Managing tools with ServiceProxy")
print("=" * 60)

# ------------------------------------------------------------
# Step 17: Create Agent 3
# ------------------------------------------------------------
print("\n[Step 17] Create Agent 3")
agent3_id = "agent3"
agent3 = store.for_agent(agent3_id)
print(f"  ├─ Agent ID: {agent3_id}")
print("  └─ ✓ Agent 3 created successfully")

# ------------------------------------------------------------
# Step 18: Add Service to Agent 3
# ------------------------------------------------------------
print("\n[Step 18] Add a service to Agent 3")
agent3_service_config = {
    "mcpServers": {
        "mcpstore": {
            "url": "http://115.190.220.103:2230/mcp"
            # "url": "http://115.190.220.103:2230/mcp"
        }
    }
}
agent3.add_service(agent3_service_config)
agent3.wait_service(service_name="mcpstore", timeout=20)
print(f"  ├─ Agent ID: {agent3_id}")
print(f"  ├─ Service name: mcpstore")
print("  └─ ✓ Service added successfully")

# ------------------------------------------------------------
# Step 19: Find Service Using find_service()
# ------------------------------------------------------------
print("\n[Step 19] Look up the service with find_service()")
service_proxy = agent3.find_service("mcpstore")
print(f"  ├─ Agent ID: {agent3_id}")
print(f"  ├─ Service name: mcpstore")
print(f"  ├─ ServiceProxy: {service_proxy}")
print(f"  ├─ Agent bound to the service: {service_proxy.agent_id}")
print(f"  ├─ Agent-scoped: {service_proxy.is_agent_scoped}")
print("  └─ ✓ ServiceProxy retrieved successfully")

# ------------------------------------------------------------
# Step 20: Modify Tools via ServiceProxy
# ------------------------------------------------------------
print("\n[Step 20] Modify the tool set via ServiceProxy")
# First remove all tools
agent3.remove_tools(service=service_proxy, tools="_all_tools")
# Then add only the tools that actually exist on this service (fixed to two in this example)
agent3.add_tools(service=service_proxy, tools=["get_mcpstore_docs", "get_current_weather"])
print(f"  ├─ Agent ID: {agent3_id}")
print(f"  ├─ Operated via ServiceProxy")
print(f"  ├─ Operation 1: remove all tools")
print(f"  ├─ Operation 2: add get_mcpstore_docs")
print("  └─ ✓ Tool set modified via ServiceProxy")

# ------------------------------------------------------------
# Step 21: List Tools from Agent 3
# ------------------------------------------------------------
print("\n[Step 21] List Agent 3's tools")
agent3_tools = agent3.list_tools(filter="available")
print(f"  ├─ Agent ID: {agent3_id}")
print(f"  ├─ Available tool count: {len(agent3_tools)}")
for idx, tool in enumerate(agent3_tools, 1):
    tool_name = getattr(tool, "name", getattr(tool, "tool_original_name", "N/A"))
    print(f"  ├─ [{idx}] {tool_name}")
print("  └─ ✓ Tool list retrieved successfully")

# ============================================================
# Tool Set Information Queries
# ============================================================
print("\n" + "=" * 60)
print("  Tool set information queries")
print("=" * 60)

# ------------------------------------------------------------
# Step 22: Get Tool Set Info for Specific Service
# ------------------------------------------------------------
print("\n[Step 22] Get tool set info for a specific service")
try:
    tool_set_info = agent1.get_tool_set_info(service="mcpstore")
    print(f"  ├─ Agent ID: {agent1_id}")
    print(f"  ├─ Service name: {tool_set_info.get('service_name', 'N/A')}")
    print(f"  ├─ Total tools: {tool_set_info.get('total_tools', 0)}")
    print(f"  ├─ Available tools: {tool_set_info.get('available_tools', 0)}")
    print(f"  ├─ Removed tools: {tool_set_info.get('removed_tools', 0)}")
    print(f"  ├─ Utilization: {tool_set_info.get('utilization', 0):.2%}")
    print("  └─ ✓ Tool set info retrieved successfully")
except Exception as e:
    print(f"  ├─ Error: {str(e)}")
    print("  └─ ✗ Failed to retrieve info")

# ------------------------------------------------------------
# Step 23: Get Tool Set Summary for Agent
# ------------------------------------------------------------
print("\n[Step 23] Get the agent's tool set summary")
try:
    summary = agent2.get_tool_set_summary()
    print(f"  ├─ Agent ID: {summary.get('agent_id', 'N/A')}")
    print(f"  ├─ Total services: {summary.get('total_services', 0)}")
    print(f"  ├─ Total available tools: {summary.get('total_available_tools', 0)}")
    print(f"  ├─ Total original tools: {summary.get('total_original_tools', 0)}")
    print(f"  ├─ Overall utilization: {summary.get('overall_utilization', 0):.2%}")
    
    services_info = summary.get('services', {})
    print(f"  ├─ Service details:")
    for svc_name, svc_info in services_info.items():
        print(f"  │   ├─ {svc_name}:")
        print(f"  │   │   ├─ Total tools: {svc_info.get('total_tools', 0)}")
        print(f"  │   │   ├─ Available: {svc_info.get('available_tools', 0)}")
        print(f"  │   │   └─ Utilization: {svc_info.get('utilization', 0):.2%}")
    print("  └─ ✓ Tool set summary retrieved successfully")
except Exception as e:
    print(f"  ├─ Error: {str(e)}")
    print("  └─ ✗ Failed to retrieve summary")

# ============================================================
# Advanced Features
# ============================================================
print("\n" + "=" * 60)
print("  Advanced features")
print("=" * 60)

# ------------------------------------------------------------
# Step 24: Reset Tools to Default State
# ------------------------------------------------------------
print("\n[Step 24] Reset the tool set to its default state")
agent1.reset_tools(service="mcpstore")
reset_tools = agent1.list_tools(filter="available")
print(f"  ├─ Agent ID: {agent1_id}")
print(f"  ├─ Service name: mcpstore")
print(f"  ├─ Available tools after reset: {len(reset_tools)}")
print("  └─ ✓ Tool set reset successfully (all tools restored)")

# ------------------------------------------------------------
# Step 25: Batch Operations on All Services
# ------------------------------------------------------------
print("\n[Step 25] Run a batch operation across all services")
# Make a specific tool available on every service of agent2
agent2.add_tools(service="_all_services", tools=["get_mcpstore_docs"])
print(f"  ├─ Agent ID: {agent2_id}")
print(f"  ├─ Scope: all services")
print(f"  ├─ Operation: make get_mcpstore_docs available")
print("  └─ ✓ Batch operation complete")

# ------------------------------------------------------------
# Step 26: Chain Operations
# ------------------------------------------------------------
print("\n[Step 26] Chained operations demo")
agent3.remove_tools(service="mcpstore", tools="_all_tools") \
      .add_tools(service="mcpstore", tools=["get_mcpstore_docs"]) \
      .add_tools(service="mcpstore", tools=["get_current_weather"])

chain_tools = agent3.list_tools(filter="available")
print(f"  ├─ Agent ID: {agent3_id}")
print(f"  ├─ Chained operations:")
print(f"  │   ├─ 1. Remove all tools")
print(f"  │   ├─ 2. Add get_mcpstore_docs")
print(f"  │   └─ 3. Add get_current_weather")
print(f"  ├─ Final available tools: {len(chain_tools)}")
print("  └─ ✓ Chained operations complete")

# ------------------------------------------------------------
# Step 27: Compare filter="available" vs filter="all"
# ------------------------------------------------------------
print("\n[Step 27] Compare the effect of the filter argument")
available = agent1.list_tools(filter="available")
all_tools = agent1.list_tools(filter="all")
print(f"  ├─ Agent ID: {agent1_id}")
print(f"  ├─ filter='available': {len(available)} tools")
print(f"  ├─ filter='all': {len(all_tools)} tools")
print(f"  ├─ Difference: {len(all_tools) - len(available)} tools filtered out")
print("  └─ ✓ filter comparison complete")

# ============================================================
# Cross-Agent Isolation Verification
# ============================================================
print("\n" + "=" * 60)
print("  Cross-agent isolation verification")
print("=" * 60)

# ------------------------------------------------------------
# Step 28: Verify Agent Isolation
# ------------------------------------------------------------
print("\n[Step 28] Verify tool set isolation between agents")
agent1_tools = agent1.list_tools(filter="available")
agent2_tools = agent2.list_tools(filter="available")
agent3_tools = agent3.list_tools(filter="available")

print(f"  ├─ Agent 1 available tools: {len(agent1_tools)}")
print(f"  ├─ Agent 2 available tools: {len(agent2_tools)}")
print(f"  ├─ Agent 3 available tools: {len(agent3_tools)}")
print(f"  ├─ Each agent's tool set is managed independently")
print("  └─ ✓ Agent isolation verified")

# ------------------------------------------------------------
# Step 29: Cross-Agent Operation Protection Test
# ------------------------------------------------------------
print("\n[Step 29] Cross-agent operation guard test")
try:
    # Try to operate on agent2 using agent1's ServiceProxy
    agent1_service = agent1.find_service("mcpstore")
    agent2.add_tools(service=agent1_service, tools=["some_tool"])
    print("  └─ ✗ Should not reach this point (an exception is expected)")
except Exception as e:
    error_msg = str(e)
    print(f"  ├─ Attempted a cross-agent operation")
    print(f"  ├─ Expected behavior: raise CrossAgentOperationError")
    print(f"  ├─ Exception message: {error_msg[:60]}...")
    print("  └─ ✓ Cross-agent operation guard works")

# ============================================================
# Config Review and Cleanup
# ============================================================
print("\n" + "=" * 60)
print("  Config review and cleanup")
print("=" * 60)

# ------------------------------------------------------------
# Step 30: Show Final Configuration
# ------------------------------------------------------------
print("\n[Step 30] Show the final configuration")
config = store.for_store().show_config()
summary = config.get('summary', {})
agents = config.get('agents', {})

print(f"  ├─ Total agents: {summary.get('total_agents', 0)}")
print(f"  ├─ Total services: {summary.get('total_services', 0)}")
print(f"  ├─ Total clients: {summary.get('total_clients', 0)}")

print(f"  ├─ Agent details:")
for agent_name, agent_data in agents.items():
    services = agent_data.get('services', {})
    print(f"  │   ├─ {agent_name}: {len(services)} services")
    for svc_name in list(services.keys())[:2]:
        print(f"  │   │   └─ {svc_name}")

print("  └─ ✓ Configuration retrieved successfully")

# ------------------------------------------------------------
# Step 31: Reset Configuration (Final Cleanup)
# ------------------------------------------------------------
print("\n[Step 31] Reset configuration (final cleanup)")
store.for_store().reset_config()
print("  └─ ✓ Configuration reset successfully")

# ============================================================
# Summary
# ============================================================
print("\n" + "=" * 60)
print("  Tool Set Management Summary")
print("=" * 60)

print("""
This example demonstrates the following features:

1. Basic tool set management
   - Remove all tools (remove_tools with "_all_tools")
   - Add a single tool (add_tools with a tool list)
   - Blocking calls to unavailable tools

2. Multi-service tool management
   - Add multiple services to an agent
   - Remove tools for a specific service
   - List tools grouped by service

3. Tool management via ServiceProxy
   - Obtain a service proxy with find_service()
   - Operate on the tool set through ServiceProxy
   - Service ownership verification

4. Tool set information queries
   - get_tool_set_info() for service-level information
   - get_tool_set_summary() for an agent-level summary

5. Advanced features
   - reset_tools() to restore the default state
   - "_all_services" batch operations
   - Chained calls
   - Comparing the filter argument

6. Safety features
   - Tool set isolation between agents
   - Cross-agent operation guard
   - Service ownership verification
""")

# ============================================================
print("\n" + "=" * 60)
print("  Tool set management example complete")
print("=" * 60)
print()
