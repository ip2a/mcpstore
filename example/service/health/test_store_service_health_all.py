"""
测试：Store 检查所有服务健康状态
功能：测试使用 check_services() 检查所有服务的健康状态
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
print("测试：Store 检查所有服务健康状态")
print("=" * 60)

# 1️⃣ 初始化 Store 并添加多个服务
print("\n1️⃣ 初始化 Store 并添加多个服务")
store = MCPStore.setup_store(debug=True)
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

# 2️⃣ 等待所有服务就绪
print("\n2️⃣ 等待所有服务就绪")
store.for_store().wait_service("weather", timeout=30.0)
store.for_store().wait_service("search", timeout=30.0)
print(f"✅ 所有服务已就绪")

# 3️⃣ 使用 check_services() 检查所有服务健康状态
print("\n3️⃣ 使用 check_services() 检查所有服务健康状态")
health_report = store.for_store().check_services()
print(f"✅ 健康检查完成")
print(f"   返回类型: {type(health_report)}")

# 4️⃣ 展示健康报告的主要字段
print("\n4️⃣ 展示健康报告的主要字段")
if isinstance(health_report, dict):
    print(f"📊 健康报告:")
    if 'total' in health_report:
        print(f"   总服务数: {health_report['total']}")
    if 'healthy' in health_report:
        print(f"   健康服务数: {health_report['healthy']}")
    if 'unhealthy' in health_report:
        print(f"   不健康服务数: {health_report['unhealthy']}")
    if 'services' in health_report:
        print(f"   服务详情数量: {len(health_report['services'])}")

# 5️⃣ 展示每个服务的健康状态
print("\n5️⃣ 展示每个服务的健康状态")
if isinstance(health_report, dict) and 'services' in health_report:
    for svc_name, svc_health in health_report['services'].items():
        print(f"\n   服务: {svc_name}")
        print(f"   - 健康状态: {svc_health.get('status', 'N/A')}")
        print(f"   - 状态: {svc_health.get('state', 'N/A')}")
        if 'last_check' in svc_health:
            print(f"   - 最后检查: {svc_health['last_check']}")

# 6️⃣ 展示完整的健康报告（JSON 格式）
print("\n6️⃣ 完整的健康报告（JSON 格式）:")
print("-" * 60)
print(json.dumps(health_report, indent=2, ensure_ascii=False, default=str))
print("-" * 60)

# 7️⃣ 列出所有服务并逐个检查
print("\n7️⃣ 列出所有服务并逐个检查")
services = store.for_store().list_services()
print(f"   服务列表: {[s.name for s in services]}")

# 8️⃣ 判断整体健康状态
print("\n8️⃣ 判断整体健康状态")
if isinstance(health_report, dict):
    total = health_report.get('total', 0)
    healthy = health_report.get('healthy', 0)
    
    if total == 0:
        print(f"⚠️ 没有服务")
    elif healthy == total:
        print(f"✅ 所有服务都健康 ({healthy}/{total})")
    else:
        unhealthy = health_report.get('unhealthy', 0)
        print(f"⚠️ 存在不健康的服务 (健康: {healthy}/{total}, 不健康: {unhealthy})")

print("\n💡 check_services() 特点:")
print("   - 检查所有已注册服务的健康状态")
print("   - 返回聚合的健康报告")
print("   - 包含总数、健康数、不健康数")
print("   - 包含每个服务的健康详情")
print("   - 适合整体健康监控")

print("\n💡 使用场景:")
print("   - 系统健康检查")
print("   - 监控面板数据源")
print("   - 定期健康巡检")
print("   - 故障诊断")

print("\n" + "=" * 60)
print("✅ Store 检查所有服务健康状态测试完成")
print("=" * 60)

