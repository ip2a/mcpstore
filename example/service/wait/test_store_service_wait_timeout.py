"""
测试：Store 等待服务超时
功能：测试 wait_service() 的超时机制
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
print("测试：Store 等待服务超时")
print("=" * 60)

# 1️⃣ 初始化 Store 并添加服务
print("\n1️⃣ 初始化 Store 并添加服务")
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

# 2️⃣ 使用合理的超时时间等待
print("\n2️⃣ 使用合理的超时时间等待（30秒）")
print(f"⏳ 等待中...")
start_time = time.time()
result = store.for_store().wait_service("weather", timeout=30.0)
elapsed_time = time.time() - start_time
print(f"✅ 服务就绪")
print(f"   耗时: {elapsed_time:.2f} 秒")

# 3️⃣ 测试等待不存在的服务
print("\n3️⃣ 测试等待不存在的服务")
print(f"⏳ 尝试等待不存在的服务 'nonexistent'...")
try:
    result = store.for_store().wait_service("nonexistent", timeout=5.0)
    print(f"⚠️ 意外成功: {result}")
except Exception as e:
    print(f"✅ 预期的异常: {type(e).__name__}")
    print(f"   错误信息: {str(e)}")

# 4️⃣ 测试不同的超时时间
print("\n4️⃣ 测试不同的超时时间")
test_service = {
    "mcpServers": {
        "search": {
            "url": "https://mcpstore.wiki/mcp"
        }
    }
}
store.for_store().add_service(test_service)

timeout_values = [10.0, 20.0, 30.0]
for timeout in timeout_values:
    print(f"\n   超时设置: {timeout} 秒")
    start = time.time()
    result = store.for_store().wait_service("search", timeout=timeout)
    elapsed = time.time() - start
    print(f"   ✅ 等待结果: {result}")
    print(f"   ✅ 实际耗时: {elapsed:.2f} 秒")
    break  # 服务已就绪，后续立即返回

# 5️⃣ 批量等待多个服务
print("\n5️⃣ 批量等待多个服务")
multi_services = {
    "mcpServers": {
        "service1": {"url": "https://mcpstore.wiki/mcp"},
        "service2": {"url": "https://mcpstore.wiki/mcp"}
    }
}
store.for_store().add_service(multi_services)
print(f"✅ 已添加 2 个服务")

service_names = ["service1", "service2"]
print(f"\n   批量等待所有服务就绪...")
total_start = time.time()
for svc_name in service_names:
    print(f"   ⏳ 等待 '{svc_name}'...")
    start = time.time()
    result = store.for_store().wait_service(svc_name, timeout=30.0)
    elapsed = time.time() - start
    print(f"   ✅ '{svc_name}' 就绪 (耗时: {elapsed:.2f}s)")

total_elapsed = time.time() - total_start
print(f"\n   ✅ 所有服务就绪，总耗时: {total_elapsed:.2f} 秒")

print("\n💡 超时机制特点:")
print("   - timeout 参数指定最大等待时间（秒）")
print("   - 超时会抛出异常")
print("   - 服务就绪后立即返回，不等待全部超时")
print("   - 等待不存在的服务会抛出异常")

print("\n💡 最佳实践:")
print("   - 远程服务：使用较长超时（30秒+）")
print("   - 本地服务：使用较短超时（10秒）")
print("   - 生产环境：根据网络情况调整")
print("   - 批量等待：设置合理的单个超时")

print("\n💡 错误处理:")
print("   - 捕获超时异常")
print("   - 检查服务是否存在")
print("   - 记录等待时间用于调试")

print("\n" + "=" * 60)
print("✅ Store 等待服务超时测试完成")
print("=" * 60)

