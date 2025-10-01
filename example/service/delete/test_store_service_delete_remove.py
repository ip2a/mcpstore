"""
测试：Store 移除服务（运行态）
功能：测试使用 remove_service() 移除服务的运行实例
上下文：Store 级别
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore

print("=" * 60)
print("测试：Store 移除服务（运行态）")
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
store.for_store().wait_service("weather", timeout=30.0)
print(f"✅ 服务 'weather' 已添加并就绪")

# 2️⃣ 验证服务存在
print("\n2️⃣ 验证服务存在")
services_before = store.for_store().list_services()
print(f"📋 移除前服务列表:")
for svc in services_before:
    print(f"   - {svc.name}")
print(f"   总计: {len(services_before)} 个服务")

# 3️⃣ 获取服务的工具
print("\n3️⃣ 获取服务的工具")
service_proxy = store.for_store().find_service("weather")
tools_before = service_proxy.list_tools()
print(f"📋 服务的工具数量: {len(tools_before)}")

# 4️⃣ 使用 remove_service() 移除服务
print("\n4️⃣ 使用 remove_service() 移除服务")
result = service_proxy.remove_service()
print(f"✅ 服务运行实例已移除")
print(f"   返回结果: {result}")

# 5️⃣ 验证服务已从运行列表中移除
print("\n5️⃣ 验证服务已从运行列表中移除")
services_after = store.for_store().list_services()
print(f"📋 移除后服务列表:")
if services_after:
    for svc in services_after:
        print(f"   - {svc.name}")
    print(f"   总计: {len(services_after)} 个服务")
else:
    print(f"   （无服务）")

# 6️⃣ 尝试查找已移除的服务
print("\n6️⃣ 尝试查找已移除的服务")
try:
    removed_service = store.for_store().find_service("weather")
    print(f"⚠️ 意外：仍然能找到服务")
except Exception as e:
    print(f"✅ 预期结果：服务不存在")
    print(f"   异常: {type(e).__name__}")

# 7️⃣ 可以重新添加服务
print("\n7️⃣ 可以重新添加服务")
store.for_store().add_service(service_config)
store.for_store().wait_service("weather", timeout=30.0)
print(f"✅ 服务已重新添加")

services_readded = store.for_store().list_services()
print(f"📋 重新添加后服务列表:")
for svc in services_readded:
    print(f"   - {svc.name}")

# 8️⃣ 添加多个服务并选择性移除
print("\n8️⃣ 添加多个服务并选择性移除")
multi_config = {
    "mcpServers": {
        "search": {"url": "https://mcpstore.wiki/mcp"},
        "translate": {"url": "https://mcpstore.wiki/mcp"}
    }
}
store.for_store().add_service(multi_config)
store.for_store().wait_service("search", timeout=30.0)
store.for_store().wait_service("translate", timeout=30.0)

all_services = store.for_store().list_services()
print(f"📋 所有服务: {[s.name for s in all_services]}")

# 只移除 search
search_proxy = store.for_store().find_service("search")
search_proxy.remove_service()
print(f"✅ 已移除 'search' 服务")

remaining_services = store.for_store().list_services()
print(f"📋 剩余服务: {[s.name for s in remaining_services]}")

print("\n💡 remove_service() 特点:")
print("   - 移除服务的运行实例")
print("   - 停止服务进程")
print("   - 从运行列表中移除")
print("   - 配置文件可能保留（取决于实现）")
print("   - 可以重新添加服务")

print("\n💡 使用场景:")
print("   - 临时停止服务")
print("   - 释放资源")
print("   - 服务不再需要")
print("   - 维护操作")
print("   - 动态服务管理")

print("\n💡 注意事项:")
print("   - 移除后服务不可用")
print("   - 正在进行的调用会失败")
print("   - 建议在低峰期操作")
print("   - 确认没有依赖后再移除")

print("\n" + "=" * 60)
print("✅ Store 移除服务测试完成")
print("=" * 60)

