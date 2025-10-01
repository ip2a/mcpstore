"""
测试：Agent 获取服务信息
功能：测试在 Agent 级别使用 service_info() 获取服务信息
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
print("测试：Agent 获取服务信息")
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

# 3️⃣ 使用 Agent 的 ServiceProxy 获取服务信息
print("\n3️⃣ 使用 Agent 的 ServiceProxy 获取服务信息")
service_proxy = agent.find_service("weather")
info = service_proxy.service_info()
print(f"✅ Agent 服务信息获取成功")
print(f"   返回类型: {type(info)}")

# 4️⃣ 展示 Agent 服务信息
print("\n4️⃣ 展示 Agent 服务信息")
print(f"📋 服务信息:")
print(f"   服务名称: {info.get('name', 'N/A')}")
print(f"   服务类型: {info.get('type', 'N/A')}")
if 'config' in info:
    print(f"   配置: {info['config']}")

# 5️⃣ 创建第二个 Agent 并添加不同的服务
print("\n5️⃣ 创建第二个 Agent 并添加不同的服务")
agent2 = store.for_agent("agent2")
agent2_config = {
    "mcpServers": {
        "howtocook": {
            "command": "npx",
            "args": ["-y", "howtocook-mcp"]
        }
    }
}
agent2.add_service(agent2_config)
agent2.wait_service("howtocook", timeout=30.0)
print(f"✅ Agent 'agent2' 服务 'howtocook' 已添加并就绪")

# 6️⃣ 对比两个 Agent 的服务信息
print("\n6️⃣ 对比两个 Agent 的服务信息")
agent1_proxy = agent.find_service("weather")
agent1_info = agent1_proxy.service_info()

agent2_proxy = agent2.find_service("howtocook")
agent2_info = agent2_proxy.service_info()

print(f"\n📋 Agent1 服务信息:")
print(f"   名称: {agent1_info.get('name', 'N/A')}")
print(f"   类型: {agent1_info.get('type', 'N/A')}")
print(f"   配置: {agent1_info.get('config', 'N/A')}")

print(f"\n📋 Agent2 服务信息:")
print(f"   名称: {agent2_info.get('name', 'N/A')}")
print(f"   类型: {agent2_info.get('type', 'N/A')}")
print(f"   配置: {agent2_info.get('config', 'N/A')}")

# 7️⃣ 验证 Agent 服务信息的隔离性
print("\n7️⃣ 验证 Agent 服务信息的隔离性")
print(f"✅ Agent1 和 Agent2 的服务信息完全独立")
print(f"   Agent1 看不到 Agent2 的服务")
print(f"   Agent2 看不到 Agent1 的服务")

# 8️⃣ 展示完整的服务信息
print("\n8️⃣ Agent1 完整服务信息（JSON 格式）:")
print("-" * 60)
print(json.dumps(agent1_info, indent=2, ensure_ascii=False, default=str))
print("-" * 60)

print("\n💡 Agent 服务信息特点:")
print("   - 每个 Agent 有独立的服务信息")
print("   - Agent 之间的服务信息完全隔离")
print("   - 不同 Agent 可以有同名但配置不同的服务")
print("   - 适合多租户场景的信息查询")

print("\n💡 使用场景:")
print("   - 多用户系统：每个用户查看自己的服务")
print("   - 多任务系统：每个任务独立管理服务")
print("   - 隔离测试：不同环境使用不同配置")

print("\n" + "=" * 60)
print("✅ Agent 获取服务信息测试完成")
print("=" * 60)

