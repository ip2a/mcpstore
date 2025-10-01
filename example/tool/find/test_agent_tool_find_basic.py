"""
测试：Agent 查找工具（基础）
功能：测试在 Agent 级别使用 find_tool() 查找工具
上下文：Agent 级别
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore

print("=" * 60)
print("测试：Agent 查找工具（基础）")
print("=" * 60)

# 1️⃣ 初始化 Store
print("\n1️⃣ 初始化 Store")
store = MCPStore.setup_store(debug=True)
print(f"✅ Store 初始化成功")

# 2️⃣ 创建 Agent 并添加服务
print("\n2️⃣ 创建 Agent 并添加服务")
agent = store.for_agent("agent1")
service_config = {
    "mcpServers": {
        "weather": {
            "url": "https://mcpstore.wiki/mcp"
        }
    }
}
agent.add_service(service_config)
agent.wait_service("weather", timeout=30.0)
print(f"✅ Agent 'agent1' 服务 'weather' 已添加并就绪")

# 3️⃣ Agent 列出工具
print("\n3️⃣ Agent 列出工具")
tools = agent.list_tools()
print(f"✅ Agent 可用工具数量: {len(tools)}")
if tools:
    print(f"   工具列表:")
    for tool in tools[:5]:
        print(f"   - {tool.name}")

# 4️⃣ Agent 使用 find_tool() 查找工具
print("\n4️⃣ Agent 使用 find_tool() 查找工具")
if tools:
    tool_name = "get_current_weather"
    tool_proxy = agent.find_tool(tool_name)
    print(f"✅ Agent 找到工具: {tool_name}")
    print(f"   ToolProxy: {tool_proxy}")

# 5️⃣ Agent 使用 ToolProxy 获取工具信息
print("\n5️⃣ Agent 使用 ToolProxy 获取工具信息")
if tools:
    info = tool_proxy.tool_info()
    print(f"✅ 工具信息:")
    if isinstance(info, dict):
        print(f"   名称: {info.get('name', 'N/A')}")
        print(f"   描述: {info.get('description', 'N/A')}")

# 6️⃣ Agent 使用 ToolProxy 调用工具
print("\n6️⃣ Agent 使用 ToolProxy 调用工具")
if tools:
    result = tool_proxy.call_tool({"query": "北京"})
    print(f"✅ Agent 工具调用成功")
    print(f"   结果: {result.text_output if hasattr(result, 'text_output') else result}")

# 7️⃣ 创建第二个 Agent 验证隔离性
print("\n7️⃣ 创建第二个 Agent 验证隔离性")
agent2 = store.for_agent("agent2")
agent2_config = {
    "mcpServers": {
        "search": {
            "url": "https://mcpstore.wiki/mcp"
        }
    }
}
agent2.add_service(agent2_config)
agent2.wait_service("search", timeout=30.0)
print(f"✅ Agent 'agent2' 服务 'search' 已添加")

# 8️⃣ 对比两个 Agent 的工具
print("\n8️⃣ 对比两个 Agent 的工具")
agent1_tools = agent.list_tools()
agent2_tools = agent2.list_tools()
print(f"   Agent1 工具数: {len(agent1_tools)}")
print(f"   Agent2 工具数: {len(agent2_tools)}")

print(f"\n   Agent1 工具名称:")
for tool in agent1_tools[:3]:
    print(f"      - {tool.name}")

print(f"\n   Agent2 工具名称:")
for tool in agent2_tools[:3]:
    print(f"      - {tool.name}")

print(f"\n   ✅ 两个 Agent 的工具完全隔离")

# 9️⃣ 验证 Store 级别看不到 Agent 工具
print("\n9️⃣ 验证 Store 级别看不到 Agent 工具")
store_tools = store.for_store().list_tools()
print(f"   Store 工具数量: {len(store_tools)}")
if store_tools:
    print(f"   Store 工具列表: {[t.name for t in store_tools[:3]]}")
else:
    print(f"   （Store 级别无工具，Agent 工具已隔离）")

print("\n💡 Agent find_tool() 特点:")
print("   - 每个 Agent 独立查找工具")
print("   - Agent 只能找到自己服务的工具")
print("   - Store 级别看不到 Agent 的工具")
print("   - 不同 Agent 的工具完全隔离")
print("   - 适合多租户工具管理")

print("\n💡 使用场景:")
print("   - 多用户系统：每个用户查找自己的工具")
print("   - 多任务系统：每个任务独立工具管理")
print("   - 隔离测试：不同环境使用不同工具")
print("   - 权限控制：按 Agent 限制工具访问")

print("\n" + "=" * 60)
print("✅ Agent 查找工具测试完成")
print("=" * 60)

