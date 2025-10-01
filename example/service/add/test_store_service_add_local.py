"""
测试：Store 添加本地服务
功能：测试在 Store 级别添加本地 MCP 服务
上下文：Store 级别
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore

print("=" * 60)
print("测试：Store 添加本地服务")
print("=" * 60)

# 1️⃣ 初始化 Store
print("\n1️⃣ 初始化 Store")
store = MCPStore.setup_store(debug=True)
print(f"✅ Store 初始化成功")

# 2️⃣ 准备本地服务配置
print("\n2️⃣ 准备本地服务配置")
local_service = {
    "mcpServers": {
        "howtocook": {
            "command": "npx",
            "args": ["-y", "howtocook-mcp"]
        }
    }
}
print(f"📋 服务名称: howtocook")
print(f"📋 服务类型: 本地命令")
print(f"📋 命令: npx -y howtocook-mcp")

# 3️⃣ 添加服务
print("\n3️⃣ 添加服务")
result = store.for_store().add_service(local_service)
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
wait_result = store.for_store().wait_service("howtocook", timeout=30.0)
print(f"✅ 服务就绪: {wait_result}")

# 6️⃣ 列出服务的工具
print("\n6️⃣ 列出服务的工具")
tools = store.for_store().list_tools()
print(f"✅ 可用工具数量: {len(tools)}")
if tools:
    print(f"   前 5 个工具:")
    for tool in tools[:5]:
        print(f"   - {tool.name}")
    if len(tools) > 5:
        print(f"   ... 还有 {len(tools) - 5} 个工具")

print("\n💡 本地服务特点:")
print("   - 使用本地命令启动（如 npx, python 等）")
print("   - 需要本地环境支持（如 Node.js, Python 等）")
print("   - 启动时间较快")
print("   - 适合开发和测试")

print("\n" + "=" * 60)
print("✅ Store 添加本地服务测试完成")
print("=" * 60)

