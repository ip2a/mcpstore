"""
测试：Agent 添加远程服务
功能：测试在 Agent 级别添加远程 MCP 服务
上下文：Agent 级别
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore

print("=" * 60)
print("测试：Agent 添加远程服务")
print("=" * 60)

# 1️⃣ 初始化 Store
print("\n1️⃣ 初始化 Store")
store = MCPStore.setup_store(debug=True)
print(f"✅ Store 初始化成功")

# 2️⃣ 创建 Agent Context
print("\n2️⃣ 创建 Agent Context")
agent = store.for_agent("agent1")
print(f"✅ Agent 'agent1' 创建成功")

# 3️⃣ 准备远程服务配置
print("\n3️⃣ 准备远程服务配置")
remote_service = {
    "mcpServers": {
        "weather": {
            "url": "https://mcpstore.wiki/mcp"
        }
    }
}
print(f"📋 服务名称: weather")
print(f"📋 服务类型: 远程 URL")

# 4️⃣ 在 Agent 级别添加服务
print("\n4️⃣ 在 Agent 级别添加服务")
result = agent.add_service(remote_service)
print(f"✅ 服务添加成功")
print(f"   返回结果: {result}")

# 5️⃣ 验证 Agent 服务
print("\n5️⃣ 验证 Agent 服务")
agent_services = agent.list_services()
print(f"✅ Agent 服务数量: {len(agent_services)}")
for svc in agent_services:
    print(f"   - {svc.name}")

# 6️⃣ 等待 Agent 服务就绪
print("\n6️⃣ 等待 Agent 服务就绪")
wait_result = agent.wait_service("weather", timeout=30.0)
print(f"✅ 服务就绪: {wait_result}")

# 7️⃣ 列出 Agent 的工具
print("\n7️⃣ 列出 Agent 的工具")
tools = agent.list_tools()
print(f"✅ Agent 可用工具数量: {len(tools)}")
if tools:
    print(f"   工具列表:")
    for tool in tools:
        print(f"   - {tool.name}")

# 8️⃣ 测试工具调用
print("\n8️⃣ 测试工具调用")
if tools:
    tool_name = "get_current_weather"
    print(f"📞 调用工具: {tool_name}")
    result = agent.use_tool(tool_name, {"query": "北京"})
    print(f"✅ 调用成功")
    print(f"   结果: {result.text_output if hasattr(result, 'text_output') else result}")

# 9️⃣ 创建第二个 Agent 验证隔离性
print("\n9️⃣ 创建第二个 Agent 验证隔离性")
agent2 = store.for_agent("agent2")
agent2_services = agent2.list_services()
print(f"✅ Agent2 服务数量: {len(agent2_services)}")
print(f"   （Agent2 看不到 Agent1 的服务）")

print("\n💡 Agent 远程服务特点:")
print("   - 每个 Agent 独立连接远程服务")
print("   - 不同 Agent 可以连接不同的服务")
print("   - 服务状态和工具完全隔离")
print("   - 适合多用户、多任务场景")

print("\n" + "=" * 60)
print("✅ Agent 添加远程服务测试完成")
print("=" * 60)

