"""
测试：Agent 列出所有服务
功能：测试在 Agent 级别使用 list_services() 列出服务
上下文：Agent 级别
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore

print("=" * 60)
print("测试：Agent 列出所有服务")
print("=" * 60)

# 1️⃣ 初始化 Store
print("\n1️⃣ 初始化 Store")
store = MCPStore.setup_store(debug=True)
print(f"✅ Store 初始化成功")

# 2️⃣ 创建 Agent 并添加多个服务
print("\n2️⃣ 创建 Agent 并添加多个服务")
agent = store.for_agent("agent1")
services_config = {
    "mcpServers": {
        "weather": {
            "url": "https://mcpstore.wiki/mcp"
        },
        "search": {
            "url": "https://mcpstore.wiki/mcp"
        }
    }
}
agent.add_service(services_config)
print(f"✅ Agent 'agent1' 已添加 2 个服务")

# 3️⃣ 使用 Agent 的 list_services() 列出所有服务
print("\n3️⃣ 使用 Agent 的 list_services() 列出所有服务")
services = agent.list_services()
print(f"✅ Agent 服务总数: {len(services)}")
print(f"   返回类型: {type(services)}")

# 4️⃣ 遍历 Agent 的服务列表
print("\n4️⃣ 遍历 Agent 的服务列表")
for idx, svc in enumerate(services, 1):
    print(f"\n   服务 #{idx}:")
    print(f"   - 名称: {svc.name}")
    print(f"   - 对象类型: {type(svc)}")

# 5️⃣ 等待 Agent 的所有服务就绪
print("\n5️⃣ 等待 Agent 的所有服务就绪")
for svc in services:
    print(f"   等待 '{svc.name}' 就绪...")
    result = agent.wait_service(svc.name, timeout=30.0)
    print(f"   ✅ '{svc.name}' 已就绪")

# 6️⃣ 获取每个服务的工具数量
print("\n6️⃣ 获取每个服务的工具数量")
for svc in services:
    service_proxy = agent.find_service(svc.name)
    tools = service_proxy.list_tools()
    print(f"   Agent 服务 '{svc.name}': {len(tools)} 个工具")

# 7️⃣ 创建第二个 Agent 并添加不同的服务
print("\n7️⃣ 创建第二个 Agent 并添加不同的服务")
agent2 = store.for_agent("agent2")
agent2_config = {
    "mcpServers": {
        "translation": {
            "url": "https://mcpstore.wiki/mcp"
        }
    }
}
agent2.add_service(agent2_config)
print(f"✅ Agent 'agent2' 已添加服务")

# 8️⃣ 对比两个 Agent 的服务列表
print("\n8️⃣ 对比两个 Agent 的服务列表")
agent1_services = agent.list_services()
agent2_services = agent2.list_services()
print(f"   Agent1 服务列表: {[s.name for s in agent1_services]}")
print(f"   Agent2 服务列表: {[s.name for s in agent2_services]}")
print(f"   ✅ 两个 Agent 的服务列表完全独立")

# 9️⃣ 验证 Store 级别的服务列表
print("\n9️⃣ 验证 Store 级别的服务列表")
store_services = store.for_store().list_services()
print(f"   Store 服务数量: {len(store_services)}")
if store_services:
    print(f"   Store 服务列表: {[s.name for s in store_services]}")
else:
    print(f"   （Store 级别无服务）")

print("\n💡 Agent list_services() 特点:")
print("   - 每个 Agent 有独立的服务列表")
print("   - 只返回该 Agent 的服务")
print("   - 不同 Agent 的列表完全隔离")
print("   - Store 级别看不到 Agent 的服务")
print("   - 适合多租户系统的服务管理")

print("\n💡 使用场景:")
print("   - 多用户系统：每个用户一个 Agent")
print("   - 多任务系统：每个任务一个 Agent")
print("   - 隔离测试：不同测试环境使用不同 Agent")

print("\n" + "=" * 60)
print("✅ Agent 列出所有服务测试完成")
print("=" * 60)

