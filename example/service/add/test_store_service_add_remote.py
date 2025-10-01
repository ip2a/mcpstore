"""
测试：Store 添加远程服务
功能：测试在 Store 级别添加远程 MCP 服务
上下文：Store 级别
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore

print("=" * 60)
print("测试：Store 添加远程服务")
print("=" * 60)

# 1️⃣ 初始化 Store
print("\n1️⃣ 初始化 Store")
store = MCPStore.setup_store(debug=True)
print(f"✅ Store 初始化成功")

# 2️⃣ 准备远程服务配置
print("\n2️⃣ 准备远程服务配置")
remote_service = {
    "mcpServers": {
        "weather": {
            "url": "https://mcpstore.wiki/mcp"
        }
    }
}
print(f"📋 服务名称: weather")
print(f"📋 服务类型: 远程 URL")
print(f"📋 URL: https://mcpstore.wiki/mcp")

# 3️⃣ 添加服务
print("\n3️⃣ 添加服务")
result = store.for_store().add_service(remote_service)
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
wait_result = store.for_store().wait_service("weather", timeout=30.0)
print(f"✅ 服务就绪: {wait_result}")

# 6️⃣ 列出服务的工具
print("\n6️⃣ 列出服务的工具")
tools = store.for_store().list_tools()
print(f"✅ 可用工具数量: {len(tools)}")
if tools:
    print(f"   工具列表:")
    for tool in tools:
        print(f"   - {tool.name}")

# 7️⃣ 测试工具调用
print("\n7️⃣ 测试工具调用")
if tools:
    tool_name = "get_current_weather"
    print(f"📞 调用工具: {tool_name}")
    result = store.for_store().use_tool(tool_name, {"query": "北京"})
    print(f"✅ 调用成功")
    print(f"   结果: {result.text_output if hasattr(result, 'text_output') else result}")

print("\n💡 远程服务特点:")
print("   - 通过 URL 连接到远程服务")
print("   - 不需要本地环境依赖")
print("   - 连接速度取决于网络")
print("   - 适合生产环境")

print("\n" + "=" * 60)
print("✅ Store 添加远程服务测试完成")
print("=" * 60)

