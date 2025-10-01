"""
测试：Store 等待服务就绪（基础）
功能：测试使用 wait_service() 等待服务达到就绪状态
上下文：Store 级别
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore
import time

print("=" * 60)
print("测试：Store 等待服务就绪（基础）")
print("=" * 60)

# 1️⃣ 初始化 Store 并添加远程服务
print("\n1️⃣ 初始化 Store 并添加远程服务")
store = MCPStore.setup_store(debug=True)
service_config = {
    "mcpServers": {
        "weather": {
            "url": "https://mcpstore.wiki/mcp"
        }
    }
}
store.for_store().add_service(service_config)
print(f"✅ 服务 'weather' 已添加")

# 2️⃣ 立即检查服务状态（可能未就绪）
print("\n2️⃣ 立即检查服务状态（添加后）")
service_proxy = store.for_store().find_service("weather")
status_before = service_proxy.service_status()
print(f"📊 当前状态: {status_before.get('state', 'N/A')}")
print(f"📊 健康状态: {status_before.get('health', 'N/A')}")

# 3️⃣ 使用 wait_service() 等待服务就绪
print("\n3️⃣ 使用 wait_service() 等待服务就绪")
print(f"⏳ 等待中...")
start_time = time.time()
result = store.for_store().wait_service("weather", timeout=30.0)
elapsed_time = time.time() - start_time
print(f"✅ 服务已就绪")
print(f"   等待结果: {result}")
print(f"   耗时: {elapsed_time:.2f} 秒")

# 4️⃣ 检查就绪后的服务状态
print("\n4️⃣ 检查就绪后的服务状态")
status_after = service_proxy.service_status()
print(f"📊 当前状态: {status_after.get('state', 'N/A')}")
print(f"📊 健康状态: {status_after.get('health', 'N/A')}")

# 5️⃣ 验证服务可用（列出工具）
print("\n5️⃣ 验证服务可用（列出工具）")
tools = service_proxy.list_tools()
print(f"✅ 可用工具数量: {len(tools)}")
if tools:
    print(f"   工具列表:")
    for tool in tools:
        print(f"   - {tool.name}")

# 6️⃣ 再次调用 wait_service（已就绪）
print("\n6️⃣ 再次调用 wait_service（已就绪的服务）")
start_time2 = time.time()
result2 = store.for_store().wait_service("weather", timeout=30.0)
elapsed_time2 = time.time() - start_time2
print(f"✅ 立即返回（已就绪）")
print(f"   等待结果: {result2}")
print(f"   耗时: {elapsed_time2:.2f} 秒")

print("\n💡 wait_service() 特点:")
print("   - 阻塞等待服务达到就绪状态")
print("   - 支持超时设置（默认 30.0 秒）")
print("   - 如果服务已就绪，立即返回")
print("   - 返回布尔值或状态信息")
print("   - 超时会抛出异常")

print("\n💡 使用场景:")
print("   - 添加服务后确保可用")
print("   - 在使用服务前等待连接")
print("   - 服务重启后等待恢复")
print("   - 批量添加服务后等待全部就绪")

print("\n" + "=" * 60)
print("✅ Store 等待服务就绪测试完成")
print("=" * 60)

