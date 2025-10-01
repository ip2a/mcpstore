"""
测试：Agent 添加本地服务
功能：测试在 Agent 级别添加本地 MCP 服务
上下文：Agent 级别
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore

print("=" * 60)
print("测试：Agent 添加本地服务")
print("=" * 60)

# 1️⃣ 初始化 Store
print("\n1️⃣ 初始化 Store")
store = MCPStore.setup_store(debug=True)
print(f"✅ Store 初始化成功")

# 2️⃣ 创建 Agent Context
print("\n2️⃣ 创建 Agent Context")
agent = store.for_agent("agent1")
print(f"✅ Agent 'agent1' 创建成功")

# 3️⃣ 准备本地服务配置
print("\n3️⃣ 准备本地服务配置")
local_service = {
    "mcpServers": {
        "howtocook": {
            "command": "npx",
            "args": ["-y", "howtocook-mcp"]
        }
    }
}
print(f"📋 服务名称: howtocook")
print(f"📋 服务类型: 本地命令")

# 4️⃣ 在 Agent 级别添加服务
print("\n4️⃣ 在 Agent 级别添加服务")
result = agent.add_service(local_service)
print(f"✅ 服务添加成功")
print(f"   返回结果: {result}")

# 5️⃣ 验证 Agent 服务
print("\n5️⃣ 验证 Agent 服务")
agent_services = agent.list_services()
print(f"✅ Agent 服务数量: {len(agent_services)}")
for svc in agent_services:
    print(f"   - {svc.name}")

# 6️⃣ 验证 Store 级别没有该服务
print("\n6️⃣ 验证 Store 级别没有该服务")
store_services = store.for_store().list_services()
print(f"✅ Store 服务数量: {len(store_services)}")
if store_services:
    for svc in store_services:
        print(f"   - {svc.name}")
else:
    print(f"   （Store 级别无服务，Agent 服务已隔离）")

# 7️⃣ 等待 Agent 服务就绪
print("\n7️⃣ 等待 Agent 服务就绪")
wait_result = agent.wait_service("howtocook", timeout=30.0)
print(f"✅ 服务就绪: {wait_result}")

# 8️⃣ 列出 Agent 的工具
print("\n8️⃣ 列出 Agent 的工具")
tools = agent.list_tools()
print(f"✅ Agent 可用工具数量: {len(tools)}")
if tools:
    print(f"   前 5 个工具:")
    for tool in tools[:5]:
        print(f"   - {tool.name}")

print("\n💡 Agent 级别服务特点:")
print("   - 每个 Agent 有独立的服务空间")
print("   - Agent 之间的服务完全隔离")
print("   - 适合多任务、多租户场景")
print("   - Store 级别看不到 Agent 的服务")

print("\n" + "=" * 60)
print("✅ Agent 添加本地服务测试完成")
print("=" * 60)

