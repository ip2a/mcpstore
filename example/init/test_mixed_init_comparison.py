"""
测试：Store vs Agent 对比
功能：对比 Store 级别和 Agent 级别的区别
上下文：混合模式
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent))

from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore

print("=" * 60)
print("测试：Store vs Agent 对比")
print("=" * 60)

# 1️⃣ 初始化 Store
print("\n1️⃣ 初始化 Store")
store = MCPStore.setup_store(debug=True)
print(f"✅ Store 初始化成功")

# 2️⃣ 准备测试数据（远程服务配置）
print("\n2️⃣ 准备测试数据")
demo_service = {
    "mcpServers": {
        "weather": {
            "url": "https://mcpstore.wiki/mcp"
        }
    }
}
print(f"✅ 测试服务配置: weather (远程服务)")

# 3️⃣ Store 级别添加服务
print("\n3️⃣ Store 级别添加服务")
store.for_store().add_service(demo_service)
print(f"✅ Store 级别服务已添加")

# 4️⃣ Agent 级别添加服务
print("\n4️⃣ Agent 级别添加服务")
agent1 = store.for_agent("agent1")
agent2 = store.for_agent("agent2")

# Agent1 添加相同的服务
agent1.add_service(demo_service)
print(f"✅ Agent1 服务已添加")

# Agent2 添加不同的服务
agent2_service = {
    "mcpServers": {
        "search": {
            "url": "https://mcpstore.wiki/mcp"
        }
    }
}
agent2.add_service(agent2_service)
print(f"✅ Agent2 服务已添加（不同服务）")

# 5️⃣ 对比服务列表
print("\n5️⃣ 对比服务列表")
print("─" * 60)

store_services = store.for_store().list_services()
print(f"🌐 Store 级别服务: {[s.name for s in store_services]}")

agent1_services = agent1.list_services()
print(f"🤖 Agent1 服务: {[s.name for s in agent1_services]}")

agent2_services = agent2.list_services()
print(f"🤖 Agent2 服务: {[s.name for s in agent2_services]}")

print("─" * 60)

# 6️⃣ 特性对比表
print("\n6️⃣ Store vs Agent 特性对比")
print("─" * 60)
print(f"{'特性':<20} | {'Store 级别':<20} | {'Agent 级别':<20}")
print("─" * 60)
print(f"{'访问范围':<20} | {'全局共享':<20} | {'独立隔离':<20}")
print(f"{'服务空间':<20} | {'单一命名空间':<20} | {'每个Agent独立':<20}")
print(f"{'工具可见性':<20} | {'所有工具':<20} | {'Agent工具':<20}")
print(f"{'配置共享':<20} | {'是':<20} | {'否':<20}")
print(f"{'适用场景':<20} | {'简单应用':<20} | {'多任务/多租户':<20}")
print("─" * 60)

# 7️⃣ 使用建议
print("\n💡 使用建议:")
print("   📌 Store 级别:")
print("      - 适合单一应用场景")
print("      - 所有功能共享同一套服务")
print("      - 配置简单，管理方便")
print()
print("   📌 Agent 级别:")
print("      - 适合多任务场景")
print("      - 每个任务有独立的服务集")
print("      - 完全隔离，互不干扰")
print("      - 支持多租户应用")

print("\n" + "=" * 60)
print("✅ Store vs Agent 对比测试完成")
print("=" * 60)

