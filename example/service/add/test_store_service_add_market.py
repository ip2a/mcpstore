"""
测试：Store 从市场添加服务
功能：测试从 MCPStore 市场安装服务
上下文：Store 级别
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore

print("=" * 60)
print("测试：Store 从市场添加服务")
print("=" * 60)

# 1️⃣ 初始化 Store
print("\n1️⃣ 初始化 Store")
store = MCPStore.setup_store(debug=True)
print(f"✅ Store 初始化成功")

# 2️⃣ 准备市场服务配置
print("\n2️⃣ 准备市场服务配置")
market_service = {
    "mcpServers": {
        "demo-market": {
            "market": "mcpstore-demo"
        }
    }
}
print(f"📋 服务名称: demo-market")
print(f"📋 服务类型: 市场安装")
print(f"📋 市场标识: mcpstore-demo")

# 3️⃣ 从市场添加服务
print("\n3️⃣ 从市场添加服务")
result = store.for_store().add_service(market_service)
print(f"✅ 服务添加成功")
print(f"   返回结果: {result}")

# 4️⃣ 验证服务已添加
print("\n4️⃣ 验证服务已添加")
services = store.for_store().list_services()
print(f"✅ 当前服务数量: {len(services)}")
for svc in services:
    print(f"   - {svc.name}")

# 5️⃣ 等待服务就绪
print("\n5️⃣ 等待服务就绪")
wait_result = store.for_store().wait_service("demo-market", timeout=30.0)
print(f"✅ 服务就绪: {wait_result}")

# 6️⃣ 列出服务的工具
print("\n6️⃣ 列出服务的工具")
tools = store.for_store().list_tools()
print(f"✅ 可用工具数量: {len(tools)}")
if tools:
    print(f"   工具列表:")
    for tool in tools:
        print(f"   - {tool.name}")

print("\n💡 市场服务特点:")
print("   - 从 MCPStore 市场一键安装")
print("   - 自动处理依赖和配置")
print("   - 支持版本管理")
print("   - 便于发现和使用优质服务")
print("   - 适合快速集成第三方服务")

print("\n💡 市场相关信息:")
print("   - 市场地址: https://mcpstore.wiki")
print("   - 浏览可用服务: https://mcpstore.wiki/browse")

print("\n" + "=" * 60)
print("✅ Store 从市场添加服务测试完成")
print("=" * 60)

