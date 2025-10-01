"""
测试：Store 获取服务状态
功能：测试使用 service_status() 获取服务的实时运行状态
上下文：Store 级别
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore
import json

print("=" * 60)
print("测试：Store 获取服务状态")
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

# 2️⃣ 立即获取服务状态（未就绪状态）
print("\n2️⃣ 获取服务状态（添加后立即查询）")
service_proxy = store.for_store().find_service("weather")
status_before = service_proxy.service_status()
print(f"✅ 服务状态获取成功")
print(f"   状态: {status_before.get('state', 'N/A')}")
print(f"   健康状态: {status_before.get('health', 'N/A')}")

# 3️⃣ 等待服务就绪
print("\n3️⃣ 等待服务就绪")
store.for_store().wait_service("weather", timeout=30.0)
print(f"✅ 服务已就绪")

# 4️⃣ 获取就绪后的服务状态
print("\n4️⃣ 获取服务状态（就绪后）")
status_after = service_proxy.service_status()
print(f"✅ 服务状态获取成功")
print(f"   返回类型: {type(status_after)}")

# 5️⃣ 展示服务状态的主要字段
print("\n5️⃣ 展示服务状态的主要字段")
print(f"📊 运行状态:")
if 'state' in status_after:
    print(f"   生命周期状态: {status_after['state']}")
if 'health' in status_after:
    print(f"   健康状态: {status_after['health']}")
if 'connected' in status_after:
    print(f"   连接状态: {status_after['connected']}")
if 'last_check' in status_after:
    print(f"   最后检查时间: {status_after['last_check']}")

# 6️⃣ 展示完整的服务状态（JSON 格式）
print("\n6️⃣ 完整的服务状态（JSON 格式）:")
print("-" * 60)
print(json.dumps(status_after, indent=2, ensure_ascii=False, default=str))
print("-" * 60)

# 7️⃣ 检查服务状态中的常见字段
print("\n7️⃣ 检查服务状态中的常见字段")
status_fields = ['state', 'health', 'connected', 'last_check', 'uptime', 'errors']
for field in status_fields:
    if field in status_after:
        print(f"   ✅ {field}: {status_after[field]}")
    else:
        print(f"   ⚠️ {field}: 未找到")

# 8️⃣ 对比信息和状态的区别
print("\n8️⃣ 对比 service_info() 和 service_status() 的区别")
info = service_proxy.service_info()
status = service_proxy.service_status()
print(f"\n📋 service_info() 主要字段:")
print(f"   {', '.join([k for k in info.keys()][:5])}...")
print(f"\n📊 service_status() 主要字段:")
print(f"   {', '.join([k for k in status.keys()][:5])}...")

print("\n💡 service_status() 特点:")
print("   - 返回服务的实时运行状态")
print("   - 包含生命周期状态（state）")
print("   - 包含健康状态（health）")
print("   - 包含连接状态和最后检查时间")
print("   - 动态信息，会随时间变化")

print("\n💡 service_info() vs service_status():")
print("   service_info():")
print("      - 静态配置信息")
print("      - 服务名称、类型、配置")
print("      - 不会频繁变化")
print("   service_status():")
print("      - 动态运行状态")
print("      - 生命周期、健康状态")
print("      - 实时更新")

print("\n💡 使用场景:")
print("   - 监控服务运行状态")
print("   - 检查服务是否健康")
print("   - 调试连接问题")
print("   - 实时状态展示")

print("\n" + "=" * 60)
print("✅ Store 获取服务状态测试完成")
print("=" * 60)

