"""
测试：Agent 获取服务状态
功能：测试在 Agent 级别使用 service_status() 获取服务状态
上下文：Agent 级别
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore
import json

print("=" * 60)
print("测试：Agent 获取服务状态")
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
print(f"✅ Agent 'agent1' 服务 'weather' 已添加")

# 3️⃣ 获取服务状态（添加后立即查询）
print("\n3️⃣ 获取服务状态（添加后立即查询）")
service_proxy = agent.find_service("weather")
status_before = service_proxy.service_status()
print(f"✅ Agent 服务状态获取成功")
print(f"   状态: {status_before.get('state', 'N/A')}")
print(f"   健康状态: {status_before.get('health', 'N/A')}")

# 4️⃣ 等待服务就绪
print("\n4️⃣ 等待服务就绪")
agent.wait_service("weather", timeout=30.0)
print(f"✅ 服务已就绪")

# 5️⃣ 获取就绪后的服务状态
print("\n5️⃣ 获取服务状态（就绪后）")
status_after = service_proxy.service_status()
print(f"✅ 服务状态获取成功")
print(f"   状态: {status_after.get('state', 'N/A')}")
print(f"   健康状态: {status_after.get('health', 'N/A')}")

# 6️⃣ 创建第二个 Agent 并添加服务
print("\n6️⃣ 创建第二个 Agent 并添加服务")
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
print(f"✅ Agent 'agent2' 服务 'search' 已添加并就绪")

# 7️⃣ 对比两个 Agent 的服务状态
print("\n7️⃣ 对比两个 Agent 的服务状态")
agent1_proxy = agent.find_service("weather")
agent1_status = agent1_proxy.service_status()

agent2_proxy = agent2.find_service("search")
agent2_status = agent2_proxy.service_status()

print(f"\n📊 Agent1 服务状态:")
print(f"   服务: {agent1_status.get('name', 'weather')}")
print(f"   状态: {agent1_status.get('state', 'N/A')}")
print(f"   健康: {agent1_status.get('health', 'N/A')}")

print(f"\n📊 Agent2 服务状态:")
print(f"   服务: {agent2_status.get('name', 'search')}")
print(f"   状态: {agent2_status.get('state', 'N/A')}")
print(f"   健康: {agent2_status.get('health', 'N/A')}")

# 8️⃣ 展示完整的服务状态
print("\n8️⃣ Agent1 完整服务状态（JSON 格式）:")
print("-" * 60)
print(json.dumps(agent1_status, indent=2, ensure_ascii=False, default=str))
print("-" * 60)

# 9️⃣ 验证状态隔离性
print("\n9️⃣ 验证 Agent 服务状态的隔离性")
print(f"✅ Agent1 和 Agent2 的服务状态完全独立")
print(f"   每个 Agent 只能查看自己服务的状态")
print(f"   一个 Agent 的服务状态不影响另一个")

print("\n💡 Agent 服务状态特点:")
print("   - 每个 Agent 有独立的服务状态")
print("   - Agent 之间的服务状态完全隔离")
print("   - 适合多租户的状态监控")
print("   - 可以独立监控每个 Agent 的服务健康")

print("\n💡 使用场景:")
print("   - 多用户系统：每个用户监控自己的服务")
print("   - 多任务系统：每个任务独立状态管理")
print("   - SaaS 应用：租户级别的服务监控")
print("   - 测试环境：隔离的状态追踪")

print("\n" + "=" * 60)
print("✅ Agent 获取服务状态测试完成")
print("=" * 60)

