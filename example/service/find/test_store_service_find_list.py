"""
测试：Store 列出所有服务
功能：测试使用 list_services() 列出所有已注册的服务
上下文：Store 级别
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore

print("=" * 60)
print("测试：Store 列出所有服务")
print("=" * 60)

# 1️⃣ 初始化 Store
print("\n1️⃣ 初始化 Store")
store = MCPStore.setup_store(debug=True)
print(f"✅ Store 初始化成功")

# 2️⃣ 添加多个服务
print("\n2️⃣ 添加多个服务")
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
store.for_store().add_service(services_config)
print(f"✅ 已添加 2 个服务")

# 3️⃣ 使用 list_services() 列出所有服务
print("\n3️⃣ 使用 list_services() 列出所有服务")
services = store.for_store().list_services()
print(f"✅ 服务总数: {len(services)}")
print(f"   返回类型: {type(services)}")

# 4️⃣ 遍历服务列表
print("\n4️⃣ 遍历服务列表")
for idx, svc in enumerate(services, 1):
    print(f"\n   服务 #{idx}:")
    print(f"   - 名称: {svc.name}")
    print(f"   - 对象类型: {type(svc)}")
    # 检查是否是 ServiceInfo 对象
    if hasattr(svc, 'name'):
        print(f"   - 有 name 属性: ✅")
    if hasattr(svc, 'config'):
        print(f"   - 有 config 属性: ✅")

# 5️⃣ 从列表中查找特定服务
print("\n5️⃣ 从列表中查找特定服务")
target_service = "weather"
found = None
for svc in services:
    if svc.name == target_service:
        found = svc
        break

if found:
    print(f"✅ 找到服务 '{target_service}'")
    print(f"   名称: {found.name}")
else:
    print(f"❌ 未找到服务 '{target_service}'")

# 6️⃣ 等待所有服务就绪
print("\n6️⃣ 等待所有服务就绪")
for svc in services:
    print(f"   等待 '{svc.name}' 就绪...")
    result = store.for_store().wait_service(svc.name, timeout=30.0)
    print(f"   ✅ '{svc.name}' 已就绪")

# 7️⃣ 获取每个服务的工具数量
print("\n7️⃣ 获取每个服务的工具数量")
for svc in services:
    service_proxy = store.for_store().find_service(svc.name)
    tools = service_proxy.list_tools()
    print(f"   服务 '{svc.name}': {len(tools)} 个工具")

print("\n💡 list_services() 特点:")
print("   - 返回 ServiceInfo 对象列表")
print("   - 包含所有已注册的服务")
print("   - 可以遍历服务进行批量操作")
print("   - ServiceInfo 包含基本信息（name, config 等）")
print("   - 需要更多操作时可以用 find_service() 获取 ServiceProxy")

print("\n" + "=" * 60)
print("✅ Store 列出所有服务测试完成")
print("=" * 60)

