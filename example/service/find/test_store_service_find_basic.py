"""
测试：Store 查找服务（基础）
功能：测试使用 find_service() 查找服务并获取 ServiceProxy
上下文：Store 级别
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore

print("=" * 60)
print("测试：Store 查找服务（基础）")
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

# 2️⃣ 使用 find_service() 查找服务
print("\n2️⃣ 使用 find_service() 查找服务")
service_proxy = store.for_store().find_service("weather")
print(f"✅ 找到服务")
print(f"   ServiceProxy: {service_proxy}")
print(f"   类型: {type(service_proxy)}")

# 3️⃣ 验证 ServiceProxy 的方法
print("\n3️⃣ 验证 ServiceProxy 的可用方法")
methods = [m for m in dir(service_proxy) if not m.startswith('_')]
print(f"✅ ServiceProxy 可用方法数量: {len(methods)}")
print(f"   主要方法:")
important_methods = [
    'service_info', 'service_status', 'check_health', 'health_details',
    'update_config', 'patch_config', 'restart_service', 'refresh_content',
    'remove_service', 'delete_service', 'list_tools', 'tools_stats'
]
for method in important_methods:
    if method in methods:
        print(f"   - {method}()")

# 4️⃣ 使用 ServiceProxy 获取服务信息
print("\n4️⃣ 使用 ServiceProxy 获取服务信息")
info = service_proxy.service_info()
print(f"✅ 服务信息:")
print(f"   服务名称: {info.get('name', 'N/A')}")
print(f"   服务类型: {info.get('type', 'N/A')}")
if 'config' in info:
    print(f"   配置: {info['config']}")

# 5️⃣ 使用 ServiceProxy 获取服务状态
print("\n5️⃣ 使用 ServiceProxy 获取服务状态")
status = service_proxy.service_status()
print(f"✅ 服务状态:")
print(f"   状态: {status.get('state', 'N/A')}")
print(f"   健康状态: {status.get('health', 'N/A')}")

# 6️⃣ 使用 ServiceProxy 列出工具
print("\n6️⃣ 使用 ServiceProxy 列出工具")
tools = service_proxy.list_tools()
print(f"✅ 服务工具数量: {len(tools)}")
if tools:
    print(f"   工具列表:")
    for tool in tools:
        print(f"   - {tool.name}")

print("\n💡 ServiceProxy 特点:")
print("   - find_service() 返回 ServiceProxy 对象")
print("   - ServiceProxy 提供服务级别的操作方法")
print("   - 可以获取服务信息、状态、健康检查")
print("   - 可以管理服务配置和生命周期")
print("   - 可以列出服务的工具和统计")

print("\n" + "=" * 60)
print("✅ Store 查找服务测试完成")
print("=" * 60)

