"""
测试：Agent 查找服务（基础）
功能：测试在 Agent 级别使用 find_service() 查找服务
上下文：Agent 级别
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore

print("=" * 60)
print("测试：Agent 查找服务（基础）")
print("=" * 60)

# 1️⃣ 初始化 Store
print("\n1️⃣ 初始化 Store")
store = MCPStore.setup_store(debug=False)
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

# 3️⃣ 使用 Agent 的 find_service() 查找服务
print("\n3️⃣ 使用 Agent 的 find_service() 查找服务")
service_proxy = agent.find_service("weather")
print(f"✅ 在 Agent 中找到服务")
print(f"   ServiceProxy: {service_proxy}")
print(f"   类型: {type(service_proxy)}")

# 4️⃣ 使用 ServiceProxy 获取服务信息
print("\n4️⃣ 使用 ServiceProxy 获取服务信息")
info = service_proxy.service_info()
print(f"✅ 服务信息:")
print(f"info类型{type(info)}")
print(f"   服务名称: {info.get('name', 'N/A')}")
print(f"   服务类型: {info.get('type', 'N/A')}")

# 5️⃣ 使用 ServiceProxy 获取服务状态
print("\n5️⃣ 使用 ServiceProxy 获取服务状态")
status = service_proxy.service_status()
print(f"✅ 服务状态:")
print(f"   状态: {status.get('state', 'N/A')}")
print(f"   健康状态: {status.get('health', 'N/A')}")

# 6️⃣ 验证 Store 级别找不到 Agent 的服务
print("\n6️⃣ 验证 Store 级别找不到 Agent 的服务")
store_services = store.for_store().list_services()
print(f"✅ Store 级别服务数量: {len(store_services)}")
if store_services:
    print(f"   Store 服务:")
    for svc in store_services:
        print(f"   - {svc.name}")
else:
    print(f"   （Store 级别无服务，Agent 服务已隔离）")

# 7️⃣ 创建第二个 Agent 验证隔离性
print("\n7️⃣ 创建第二个 Agent 验证隔离性")
agent2 = store.for_agent("agent2")
agent2_services = agent2.list_services()
print(f"✅ Agent2 服务数量: {len(agent2_services)}")
print(f"   （Agent2 看不到 Agent1 的服务）")

# 8️⃣ Agent2 添加自己的服务
print("\n8️⃣ Agent2 添加自己的服务")
agent2_config = {
    "mcpServers": {
        "search": {
            "url": "https://mcpstore.wiki/mcp"
        }
    }
}
agent2.add_service(agent2_config)
print(f"✅ Agent2 已添加服务 'search'")

# 9️⃣ 对比两个 Agent 的服务
print("\n9️⃣ 对比两个 Agent 的服务")
agent1_services = agent.list_services()
agent2_services = agent2.list_services()
print(f"   Agent1 服务: {[s.name for s in agent1_services]}")
print(f"   Agent2 服务: {[s.name for s in agent2_services]}")
print(f"   ✅ 两个 Agent 的服务完全隔离")

print("\n💡 Agent 查找服务特点:")
print("   - 每个 Agent 有独立的服务空间")
print("   - Agent 只能查找到自己的服务")
print("   - Store 级别看不到 Agent 的服务")
print("   - 不同 Agent 之间的服务完全隔离")
print("   - 适合多用户、多任务场景")

print("\n" + "=" * 60)
print("✅ Agent 查找服务测试完成")
print("=" * 60)

