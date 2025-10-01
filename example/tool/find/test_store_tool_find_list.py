"""
测试：Store 列出所有工具
功能：测试使用 list_tools() 列出所有可用工具
上下文：Store 级别
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore

print("=" * 60)
print("测试：Store 列出所有工具")
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
store.for_store().wait_service("weather", timeout=30.0)
store.for_store().wait_service("search", timeout=30.0)
print(f"✅ 已添加 2 个服务")

# 3️⃣ 使用 list_tools() 列出所有工具
print("\n3️⃣ 使用 list_tools() 列出所有工具")
tools = store.for_store().list_tools()
print(f"✅ 工具总数: {len(tools)}")
print(f"   返回类型: {type(tools)}")

# 4️⃣ 遍历工具列表
print("\n4️⃣ 遍历工具列表")
for idx, tool in enumerate(tools[:10], 1):
    print(f"\n   工具 #{idx}:")
    print(f"   - 名称: {tool.name}")
    print(f"   - 对象类型: {type(tool)}")
    if hasattr(tool, 'description'):
        desc = tool.description[:50] + "..." if len(tool.description) > 50 else tool.description
        print(f"   - 描述: {desc}")

if len(tools) > 10:
    print(f"\n   ... 还有 {len(tools) - 10} 个工具")

# 5️⃣ 按服务分组工具
print("\n5️⃣ 按服务分组工具")
services = store.for_store().list_services()
for svc in services:
    service_proxy = store.for_store().find_service(svc.name)
    service_tools = service_proxy.list_tools()
    print(f"   服务 '{svc.name}': {len(service_tools)} 个工具")
    if service_tools:
        for tool in service_tools[:3]:
            print(f"      - {tool.name}")
        if len(service_tools) > 3:
            print(f"      ... 还有 {len(service_tools) - 3} 个工具")

# 6️⃣ 从列表中查找特定工具
print("\n6️⃣ 从列表中查找特定工具")
target_tool = "get_current_weather"
found = None
for tool in tools:
    if tool.name == target_tool:
        found = tool
        break

if found:
    print(f"✅ 在列表中找到工具 '{target_tool}'")
    print(f"   名称: {found.name}")
else:
    print(f"⚠️ 未找到工具 '{target_tool}'")

# 7️⃣ 工具名称列表
print("\n7️⃣ 工具名称列表")
tool_names = [tool.name for tool in tools]
print(f"📋 所有工具名称（前10个）:")
for name in tool_names[:10]:
    print(f"   - {name}")
if len(tool_names) > 10:
    print(f"   ... 还有 {len(tool_names) - 10} 个")

# 8️⃣ 统计工具类型
print("\n8️⃣ 统计工具信息")
print(f"   总工具数: {len(tools)}")
print(f"   服务数: {len(services)}")
print(f"   平均每服务工具数: {len(tools) / len(services) if services else 0:.1f}")

print("\n💡 list_tools() 特点:")
print("   - 返回所有可用工具的列表")
print("   - 包含所有服务的工具")
print("   - 返回 ToolInfo 对象列表")
print("   - 可以遍历进行批量操作")
print("   - 适合工具发现和统计")

print("\n💡 ToolInfo vs ToolProxy:")
print("   ToolInfo:")
print("      - 工具的基本信息对象")
print("      - 包含 name, description 等属性")
print("      - 由 list_tools() 返回")
print("      - 只读信息")
print("   ToolProxy:")
print("      - 工具的操作代理对象")
print("      - 提供完整的工具方法")
print("      - 由 find_tool() 返回")
print("      - 可执行操作")

print("\n💡 使用场景:")
print("   - 发现所有可用工具")
print("   - 工具统计分析")
print("   - 批量工具操作")
print("   - 工具列表展示")
print("   - 搜索特定工具")

print("\n" + "=" * 60)
print("✅ Store 列出所有工具测试完成")
print("=" * 60)

