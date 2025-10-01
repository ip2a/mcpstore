"""
测试：Store 获取工具详细信息
功能：测试使用 tool_info() 获取工具的详细信息
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
print("测试：Store 获取工具详细信息")
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

# 2️⃣ 查找工具
print("\n2️⃣ 查找工具")
tool_name = "get_current_weather"
tool_proxy = store.for_store().find_tool(tool_name)
print(f"✅ 找到工具: {tool_name}")

# 3️⃣ 使用 tool_info() 获取工具详细信息
print("\n3️⃣ 使用 tool_info() 获取工具详细信息")
info = tool_proxy.tool_info()
print(f"✅ 工具信息获取成功")
print(f"   返回类型: {type(info)}")

# 4️⃣ 展示工具信息的主要字段
print("\n4️⃣ 展示工具信息的主要字段")
if isinstance(info, dict):
    print(f"📋 工具基本信息:")
    if 'name' in info:
        print(f"   名称: {info['name']}")
    if 'description' in info:
        desc = info['description']
        desc_short = desc[:80] + "..." if len(desc) > 80 else desc
        print(f"   描述: {desc_short}")
    if 'inputSchema' in info:
        print(f"   输入模式: 存在")
    if 'service' in info:
        print(f"   所属服务: {info['service']}")

# 5️⃣ 展示完整的工具信息（JSON 格式）
print("\n5️⃣ 完整的工具信息（JSON 格式）:")
print("-" * 60)
print(json.dumps(info, indent=2, ensure_ascii=False, default=str))
print("-" * 60)

# 6️⃣ 检查常见字段
print("\n6️⃣ 检查工具信息中的常见字段")
common_fields = ['name', 'description', 'inputSchema', 'service', 'tags']
for field in common_fields:
    if field in info:
        print(f"   ✅ {field}: 存在")
    else:
        print(f"   ⚠️ {field}: 未找到")

# 7️⃣ 获取多个工具的信息进行对比
print("\n7️⃣ 获取多个工具的信息进行对比")
tools = store.for_store().list_tools()
if len(tools) >= 2:
    for tool in tools[:2]:
        proxy = store.for_store().find_tool(tool.name)
        tool_info = proxy.tool_info()
        print(f"\n   工具: {tool_info.get('name', 'N/A')}")
        desc = tool_info.get('description', 'N/A')
        desc_short = desc[:60] + "..." if len(desc) > 60 else desc
        print(f"   描述: {desc_short}")

print("\n💡 tool_info() 特点:")
print("   - 返回工具的详细信息")
print("   - 包含名称、描述、输入模式")
print("   - 包含所属服务信息")
print("   - 可能包含标签和其他元数据")
print("   - 适合工具发现和文档生成")

print("\n💡 使用场景:")
print("   - 查看工具详情")
print("   - 生成工具文档")
print("   - 工具搜索和过滤")
print("   - UI 展示工具信息")
print("   - 调试工具配置")

print("\n" + "=" * 60)
print("✅ Store 获取工具详细信息测试完成")
print("=" * 60)

