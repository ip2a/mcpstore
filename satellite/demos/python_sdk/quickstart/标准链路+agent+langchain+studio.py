from example_utils import setup_example_import, ExampleConfig

setup_example_import()
import sys
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[2]
WIKI_STUDIO_SCRIPT = PROJECT_ROOT / "wiki" / "mcp_service_wiki_studio.py"
from mcpstore import MCPStore
from mcpstore.config import MemoryConfig
from langchain.agents import create_agent
from langchain_core.messages import HumanMessage
from langchain_openai import ChatOpenAI


# ============================================================
# Standard Workflow Example - MCPStore with Enhancements
# Agent + LangChain + Studio
# ============================================================

print("\n" + "=" * 60)
print("  MCPStore Standard Workflow Example (Agent + LangChain + Studio)")
print("=" * 60)

# ------------------------------------------------------------
# Step 1: Initialize MCPStore
# ------------------------------------------------------------
print("\n[Step 1] Initialize MCPStore")
agent_name = "demo_agent"
agent_ctx = store.for_agent(agent_name)
store = MCPStore.setup_store(source=MemoryConfig(), debug=True)
print(f"  ├─ Agent Name: {agent_name}")
print("  └─ ✓ MCPStore instance created successfully")

# ------------------------------------------------------------
# Step 2: Reset Configuration (Show Before/After)
# ------------------------------------------------------------
print("\n[Step 2] Reset Configuration (Show Before/After)")
config_before_reset = store.for_agent(agent_name).show_config()
store.for_store().reset_config()
config_after_reset = store.for_agent(agent_name).show_config()
print("  ├─ Config before reset =>")
print(config_before_reset)
print("  ├─ Config after reset  =>")
print(config_after_reset)
print("  └─ ✓ Configuration reset and shown (before/after)")

# ------------------------------------------------------------
# Step 3: Add MCP Service (Studio Mode)
# ------------------------------------------------------------
print("\n[Step 3] Add MCP Service (Studio Mode)")
service_name = "mcpstore"
service_config = {
    "mcpServers": {
        service_name: {
            "command": sys.executable,
            "args": [str(WIKI_STUDIO_SCRIPT)],
            "env": {}
        }
    }
}
store.for_agent(agent_name).add_service(service_config)
print(f"  ├─ Service Name: {service_name}")
print(f"  ├─ Agent Name: {agent_name}")
print(f"  ├─ Service Type: STDIO (Studio Mode)")
print(f"  ├─ Command: {service_config['mcpServers'][service_name]['command']}")
print(f"  ├─ Args: {service_config['mcpServers'][service_name]['args']}")
print("  └─ ✓ Service added successfully")

# ------------------------------------------------------------
# Step 3.5: Show Configuration After Add Service (Compare with After Reset)
# ------------------------------------------------------------
print("\n[Step 3.5] Show Configuration After Add Service (Compare with After Reset)")
config_after_add_service = store.for_agent(agent_name).show_config()
print("  ├─ Config after reset (from Step 2) =>")
print(config_after_reset)
print("  ├─ Config after add service        =>")
print(config_after_add_service)
print("  └─ ✓ Configuration comparison shown (reset vs add service)")

# ------------------------------------------------------------
# Step 4: Wait for Service Ready
# ------------------------------------------------------------
print("\n[Step 4] Wait for Service Ready")
store.for_agent(agent_name).wait_service(service_name=service_name, timeout=20)
print(f"  ├─ Service Name: {service_name}")
print("  └─ ✓ Service is ready")

# ------------------------------------------------------------
# Step 5: List All Services
# ------------------------------------------------------------
print("\n[Step 5] List All Services")
services = store.for_agent(agent_name).list_services()
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
tools = store.for_agent(agent_name).list_tools()
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
# Step 7: Setup LangChain Agent and Call Tools
# ------------------------------------------------------------
print("\n[Step 7] Setup LangChain Agent and Call Tools")

# 7.1: Load LangChain tools
langchain_tools = agent_ctx.for_langchain().list_tools()
print(f"  ├─ Total LangChain Tools: {len(langchain_tools)}")
for idx, tool in enumerate(langchain_tools, 1):
    tool_desc = tool.description
    tool_desc_display = tool_desc[:80] + "..." if len(tool_desc) > 80 else tool_desc
    print(f"  │   ├─ [{idx}] {tool.name}")
    print(f"  │   │   └─ Description: {tool_desc_display}")

# 7.2: Initialize LLM
llm_config = ExampleConfig.get_llm_config()
llm = ChatOpenAI(
    temperature=0,
    model=llm_config["model"],
    api_key=llm_config["api_key"],
    base_url=llm_config["api_base"],
)
print(f"  ├─ LLM Model: {llm_config['model']}")
print(f"  ├─ API Base: {llm_config['api_base']}")

# 7.3: Build Agent
agent_graph = create_agent(
    model=llm,
    tools=langchain_tools,
    system_prompt="You are a helpful assistant that can call tools when needed."
)
print("  ├─ Agent created successfully")

# 7.4: Execute query
user_query = "Get mcpstore documentation URL"
inputs = {"messages": [HumanMessage(content=user_query)]}
print(f"  ├─ User Query: {user_query}")
print("  └─ Executing Agent...")

final_state = agent_graph.invoke(inputs)
final_messages = final_state.get("messages", [])
final_answer = final_messages[-1].content if final_messages else "(No result)"
print(f"  └─ ✓ Agent execution completed")
print(f"  └─ Result: {final_answer}")

# ------------------------------------------------------------
# Step 8: Reset Configuration (Final Cleanup - Show Before/After)
# ------------------------------------------------------------
print("\n[Step 8] Reset Configuration (Final Cleanup - Show Before/After)")
config_before_final_reset = store.for_agent(agent_name).show_config()
store.for_store().reset_config()
config_after_final_reset = store.for_agent(agent_name).show_config()
print("  ├─ Config before final reset =>")
print(config_before_final_reset)
print("  ├─ Config after final reset  =>")
print(config_after_final_reset)
print("  └─ ✓ Configuration reset and shown (before/after)")

# ============================================================
print("\n" + "=" * 60)
print("  Standard Workflow Completed (Agent + LangChain + Studio)")
print("=" * 60)
print()
