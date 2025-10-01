"""
测试：Store 完全删除服务
功能：测试使用 delete_service() 完全删除服务（包括配置和缓存）
上下文：Store 级别
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore

print("=" * 60)
print("测试：Store 完全删除服务")
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

# 2️⃣ 获取服务信息和配置
print("\n2️⃣ 获取服务信息和配置")
service_proxy = store.for_store().find_service("weather")
info_before = service_proxy.service_info()
print(f"📋 删除前服务信息:")
print(f"   名称: {info_before.get('name', 'N/A')}")
print(f"   类型: {info_before.get('type', 'N/A')}")
print(f"   配置: {info_before.get('config', 'N/A')}")

# 3️⃣ 验证服务列表
print("\n3️⃣ 验证服务列表")
services_before = store.for_store().list_services()
print(f"📋 删除前服务数量: {len(services_before)}")
for svc in services_before:
    print(f"   - {svc.name}")

# 4️⃣ 使用 delete_service() 完全删除服务
print("\n4️⃣ 使用 delete_service() 完全删除服务")
result = service_proxy.delete_service()
print(f"✅ 服务已完全删除")
print(f"   返回结果: {result}")

# 5️⃣ 验证服务已完全删除
print("\n5️⃣ 验证服务已完全删除")
services_after = store.for_store().list_services()
print(f"📋 删除后服务数量: {len(services_after)}")
if services_after:
    for svc in services_after:
        print(f"   - {svc.name}")
else:
    print(f"   （无服务）")

# 6️⃣ 尝试查找已删除的服务
print("\n6️⃣ 尝试查找已删除的服务")
try:
    deleted_service = store.for_store().find_service("weather")
    print(f"⚠️ 意外：仍然能找到服务")
except Exception as e:
    print(f"✅ 预期结果：服务不存在")
    print(f"   异常: {type(e).__name__}")

# 7️⃣ 可以重新添加同名服务
print("\n7️⃣ 可以重新添加同名服务")
store.for_store().add_service(service_config)
store.for_store().wait_service("weather", timeout=30.0)
print(f"✅ 同名服务已重新添加")

new_service = store.for_store().find_service("weather")
new_info = new_service.service_info()
print(f"📋 重新添加的服务信息:")
print(f"   名称: {new_info.get('name', 'N/A')}")
print(f"   类型: {new_info.get('type', 'N/A')}")

# 8️⃣ 对比 remove 和 delete
print("\n8️⃣ remove_service() vs delete_service()")
print(f"\n   remove_service():")
print(f"   - 移除运行实例")
print(f"   - 配置可能保留")
print(f"   - 缓存可能保留")
print(f"   - 可以快速恢复")
print(f"\n   delete_service():")
print(f"   - 完全删除服务")
print(f"   - 删除配置文件")
print(f"   - 删除所有缓存")
print(f"   - 彻底清除")

# 9️⃣ 批量删除示例
print("\n9️⃣ 批量删除示例")
# 添加多个服务
multi_config = {
    "mcpServers": {
        "service1": {"url": "https://mcpstore.wiki/mcp"},
        "service2": {"url": "https://mcpstore.wiki/mcp"}
    }
}
store.for_store().add_service(multi_config)
store.for_store().wait_service("service1", timeout=30.0)
store.for_store().wait_service("service2", timeout=30.0)

all_services = store.for_store().list_services()
print(f"📋 添加后所有服务: {[s.name for s in all_services]}")

# 批量删除
for svc in all_services:
    if svc.name.startswith("service"):
        service = store.for_store().find_service(svc.name)
        service.delete_service()
        print(f"   ✅ 已删除: {svc.name}")

final_services = store.for_store().list_services()
print(f"📋 批量删除后剩余服务: {[s.name for s in final_services]}")

print("\n💡 delete_service() 特点:")
print("   - 完全删除服务")
print("   - 删除运行实例")
print("   - 删除配置文件")
print("   - 删除所有缓存")
print("   - 彻底清理，不可恢复")

print("\n💡 使用场景:")
print("   - 永久移除服务")
print("   - 清理不需要的服务")
print("   - 释放所有资源")
print("   - 配置清理")
print("   - 环境清理")

print("\n💡 注意事项:")
print("   - 操作不可逆")
print("   - 确认服务不再需要")
print("   - 备份重要配置")
print("   - 检查服务依赖")
print("   - 谨慎使用")

print("\n" + "=" * 60)
print("✅ Store 完全删除服务测试完成")
print("=" * 60)

