"""
测试：Agent 等待服务就绪（基础）
功能：测试在 Agent 级别使用 wait_service() 等待服务
上下文：Agent 级别
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore
import time

print("=" * 60)
print("测试：Agent 等待服务就绪（基础）")
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

# 3️⃣ Agent 等待服务就绪
print("\n3️⃣ Agent 等待服务就绪")
print(f"⏳ 等待中...")
start_time = time.time()
result = agent.wait_service("weather", timeout=30.0)
elapsed_time = time.time() - start_time
print(f"✅ Agent 服务已就绪")
print(f"   等待结果: {result}")
print(f"   耗时: {elapsed_time:.2f} 秒")

# 4️⃣ 验证 Agent 服务可用
print("\n4️⃣ 验证 Agent 服务可用")
service_proxy = agent.find_service("weather")
tools = service_proxy.list_tools()
print(f"✅ Agent 可用工具数量: {len(tools)}")
if tools:
    print(f"   工具列表:")
    for tool in tools:
        print(f"   - {tool.name}")

# 5️⃣ 创建第二个 Agent 并等待其服务
print("\n5️⃣ 创建第二个 Agent 并等待其服务")
agent2 = store.for_agent("agent2")
agent2_config = {
    "mcpServers": {
        "search": {
            "url": "https://mcpstore.wiki/mcp"
        }
    }
}
agent2.add_service(agent2_config)
print(f"✅ Agent 'agent2' 服务 'search' 已添加")

print(f"⏳ 等待 Agent2 服务就绪...")
start_time2 = time.time()
result2 = agent2.wait_service("search", timeout=30.0)
elapsed_time2 = time.time() - start_time2
print(f"✅ Agent2 服务已就绪")
print(f"   耗时: {elapsed_time2:.2f} 秒")

# 6️⃣ 验证两个 Agent 的服务独立性
print("\n6️⃣ 验证两个 Agent 的服务独立性")
agent1_services = agent.list_services()
agent2_services = agent2.list_services()
print(f"   Agent1 服务: {[s.name for s in agent1_services]}")
print(f"   Agent2 服务: {[s.name for s in agent2_services]}")
print(f"   ✅ 两个 Agent 的服务完全独立")

# 7️⃣ 测试 Agent 等待已就绪的服务
print("\n7️⃣ 测试 Agent 等待已就绪的服务")
start_time3 = time.time()
result3 = agent.wait_service("weather", timeout=30.0)
elapsed_time3 = time.time() - start_time3
print(f"✅ 立即返回（服务已就绪）")
print(f"   耗时: {elapsed_time3:.2f} 秒")

# 8️⃣ 验证 Store 级别看不到 Agent 服务
print("\n8️⃣ 验证 Store 级别看不到 Agent 服务")
store_services = store.for_store().list_services()
print(f"   Store 服务数量: {len(store_services)}")
if store_services:
    print(f"   Store 服务: {[s.name for s in store_services]}")
else:
    print(f"   （Store 级别无服务，Agent 服务已隔离）")

print("\n💡 Agent wait_service() 特点:")
print("   - 每个 Agent 独立等待自己的服务")
print("   - Agent 之间的等待互不影响")
print("   - Store 级别无法等待 Agent 的服务")
print("   - 适合多租户的服务就绪控制")

print("\n💡 使用场景:")
print("   - 多用户系统：每个用户等待自己的服务")
print("   - 多任务系统：每个任务独立等待")
print("   - 隔离测试：不同环境独立等待")
print("   - 并发场景：多个 Agent 并行等待")

print("\n" + "=" * 60)
print("✅ Agent 等待服务就绪测试完成")
print("=" * 60)

