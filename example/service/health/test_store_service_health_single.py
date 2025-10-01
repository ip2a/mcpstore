"""
测试：Store 检查单个服务健康状态
功能：测试使用 check_health() 检查单个服务的健康状态
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
print("测试：Store 检查单个服务健康状态")
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

# 2️⃣ 获取服务的 ServiceProxy
print("\n2️⃣ 获取服务的 ServiceProxy")
service_proxy = store.for_store().find_service("weather")
print(f"✅ ServiceProxy 获取成功")

# 3️⃣ 使用 check_health() 检查服务健康状态
print("\n3️⃣ 使用 check_health() 检查服务健康状态")
health_summary = service_proxy.check_health()
print(f"✅ 健康检查完成")
print(f"   返回类型: {type(health_summary)}")

# 4️⃣ 展示健康摘要的主要字段
print("\n4️⃣ 展示健康摘要的主要字段")
if isinstance(health_summary, dict):
    print(f"📊 健康摘要:")
    if 'status' in health_summary:
        print(f"   健康状态: {health_summary['status']}")
    if 'state' in health_summary:
        print(f"   生命周期状态: {health_summary['state']}")
    if 'connected' in health_summary:
        print(f"   连接状态: {health_summary['connected']}")
    if 'message' in health_summary:
        print(f"   消息: {health_summary['message']}")

# 5️⃣ 展示完整的健康摘要（JSON 格式）
print("\n5️⃣ 完整的健康摘要（JSON 格式）:")
print("-" * 60)
print(json.dumps(health_summary, indent=2, ensure_ascii=False, default=str))
print("-" * 60)

# 6️⃣ 添加第二个服务并检查
print("\n6️⃣ 添加第二个服务并检查")
service2_config = {
    "mcpServers": {
        "search": {
            "url": "https://mcpstore.wiki/mcp"
        }
    }
}
store.for_store().add_service(service2_config)
store.for_store().wait_service("search", timeout=30.0)
print(f"✅ 服务 'search' 已添加并就绪")

service2_proxy = store.for_store().find_service("search")
health2_summary = service2_proxy.check_health()
print(f"\n📊 服务 'search' 健康摘要:")
print(f"   健康状态: {health2_summary.get('status', 'N/A')}")
print(f"   生命周期状态: {health2_summary.get('state', 'N/A')}")

# 7️⃣ 对比两个服务的健康状态
print("\n7️⃣ 对比两个服务的健康状态")
print(f"   weather: {health_summary.get('status', 'N/A')}")
print(f"   search: {health2_summary.get('status', 'N/A')}")

# 8️⃣ 判断服务是否健康
print("\n8️⃣ 判断服务是否健康")
if isinstance(health_summary, dict):
    status = health_summary.get('status', '').lower()
    if 'healthy' in status or 'ok' in status:
        print(f"✅ 服务 'weather' 健康")
    else:
        print(f"⚠️ 服务 'weather' 可能存在问题")

print("\n💡 check_health() 特点:")
print("   - 检查单个服务的健康状态")
print("   - 返回健康摘要（status, state, connected）")
print("   - 比 check_services() 更详细的单服务信息")
print("   - 通过 ServiceProxy 调用")
print("   - 适合单个服务的健康检查")

print("\n💡 使用场景:")
print("   - 检查特定服务的健康状态")
print("   - 服务故障诊断")
print("   - 单服务监控")
print("   - 健康状态展示")

print("\n" + "=" * 60)
print("✅ Store 检查单个服务健康状态测试完成")
print("=" * 60)

