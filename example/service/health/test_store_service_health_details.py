"""
测试：Store 获取服务详细健康信息
功能：测试使用 health_details() 获取服务的详细健康信息
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
print("测试：Store 获取服务详细健康信息")
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

# 3️⃣ 使用 health_details() 获取详细健康信息
print("\n3️⃣ 使用 health_details() 获取详细健康信息")
health_details = service_proxy.health_details()
print(f"✅ 详细健康信息获取成功")
print(f"   返回类型: {type(health_details)}")

# 4️⃣ 展示健康详情的主要字段
print("\n4️⃣ 展示健康详情的主要字段")
if isinstance(health_details, dict):
    print(f"📊 健康详情:")
    common_fields = [
        'status', 'state', 'connected', 'health', 
        'last_check', 'uptime', 'errors', 'warnings',
        'tools_count', 'resources_count', 'prompts_count'
    ]
    for field in common_fields:
        if field in health_details:
            print(f"   {field}: {health_details[field]}")

# 5️⃣ 展示完整的健康详情（JSON 格式）
print("\n5️⃣ 完整的健康详情（JSON 格式）:")
print("-" * 60)
print(json.dumps(health_details, indent=2, ensure_ascii=False, default=str))
print("-" * 60)

# 6️⃣ 检查错误和警告信息
print("\n6️⃣ 检查错误和警告信息")
if isinstance(health_details, dict):
    errors = health_details.get('errors', [])
    warnings = health_details.get('warnings', [])
    
    if errors:
        print(f"❌ 错误信息 ({len(errors)} 条):")
        for idx, error in enumerate(errors[:3], 1):
            print(f"   {idx}. {error}")
    else:
        print(f"✅ 无错误信息")
    
    if warnings:
        print(f"⚠️ 警告信息 ({len(warnings)} 条):")
        for idx, warning in enumerate(warnings[:3], 1):
            print(f"   {idx}. {warning}")
    else:
        print(f"✅ 无警告信息")

# 7️⃣ 对比三种健康检查方法
print("\n7️⃣ 对比三种健康检查方法")
print(f"\n📋 check_health() - 健康摘要:")
health_summary = service_proxy.check_health()
print(f"   {health_summary}")

print(f"\n📋 service_status() - 服务状态:")
service_status = service_proxy.service_status()
print(f"   状态: {service_status.get('state', 'N/A')}")
print(f"   健康: {service_status.get('health', 'N/A')}")

print(f"\n📋 health_details() - 详细健康信息:")
print(f"   状态: {health_details.get('status', 'N/A')}")
print(f"   生命周期: {health_details.get('state', 'N/A')}")
print(f"   连接: {health_details.get('connected', 'N/A')}")
print(f"   工具数: {health_details.get('tools_count', 'N/A')}")

# 8️⃣ 使用详细信息进行诊断
print("\n8️⃣ 使用详细信息进行诊断")
if isinstance(health_details, dict):
    status = health_details.get('status', '').lower()
    connected = health_details.get('connected', False)
    tools_count = health_details.get('tools_count', 0)
    
    print(f"📊 诊断结果:")
    if 'healthy' in status and connected and tools_count > 0:
        print(f"   ✅ 服务完全健康")
        print(f"   - 状态: {status}")
        print(f"   - 连接: 正常")
        print(f"   - 工具: {tools_count} 个")
    elif connected:
        print(f"   ⚠️ 服务部分健康")
        print(f"   - 连接正常但可能存在其他问题")
    else:
        print(f"   ❌ 服务存在问题")
        print(f"   - 连接状态异常")

print("\n💡 health_details() 特点:")
print("   - 返回最详细的健康信息")
print("   - 包含错误和警告列表")
print("   - 包含工具、资源、提示的数量")
print("   - 包含运行时间、最后检查时间")
print("   - 适合深度诊断和调试")

print("\n💡 三种方法对比:")
print("   check_services():")
print("      - 所有服务的聚合健康报告")
print("      - 适合整体监控")
print("   check_health():")
print("      - 单个服务的健康摘要")
print("      - 适合快速检查")
print("   health_details():")
print("      - 单个服务的详细健康信息")
print("      - 适合深度诊断")

print("\n" + "=" * 60)
print("✅ Store 获取服务详细健康信息测试完成")
print("=" * 60)

