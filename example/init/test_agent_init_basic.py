"""
测试：Agent 基础初始化
功能：测试 Agent 级别的上下文初始化
上下文：Agent 级别
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent))

from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore

print("=" * 60)
print("测试：Agent 基础初始化")
print("=" * 60)

# 1️⃣ 初始化 Store
print("\n1️⃣ 初始化 Store")
store = MCPStore.setup_store(debug=True)
print(f"✅ Store 初始化成功: {store}")

# 2️⃣ 创建 Agent Context
print("\n2️⃣ 创建 Agent Context")
agent_id = "agent1"
agent_context = store.for_agent(agent_id)
print(f"✅ Agent Context 创建成功")
print(f"   Agent ID: {agent_id}")
print(f"   Context: {agent_context}")
print(f"   类型: {type(agent_context)}")

# 3️⃣ 创建多个 Agent Context
print("\n3️⃣ 创建多个 Agent Context")
agent_ids = ["agent1", "agent2", "agent3"]
agents = {}
for aid in agent_ids:
    agents[aid] = store.for_agent(aid)
    print(f"✅ Agent '{aid}' Context 创建成功")

# 4️⃣ 验证 Agent 隔离性（初始状态）
print("\n4️⃣ 验证 Agent 隔离性")
for aid in agent_ids:
    services = agents[aid].list_services()
    print(f"   Agent '{aid}' 服务数量: {len(services)}")

print("\n💡 Agent 特性说明:")
print("   - 每个 Agent 有独立的服务空间")
print("   - Agent 之间的服务和工具完全隔离")
print("   - 适合多租户、多任务场景")

print("\n" + "=" * 60)
print("✅ Agent 基础初始化测试完成")
print("=" * 60)

