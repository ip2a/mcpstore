"""
测试：Store 获取工具标签
功能：测试使用 tool_tags() 获取工具的标签
上下文：Store 级别
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore

print("=" * 60)
print("测试：Store 获取工具标签")
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

# 3️⃣ 使用 tool_tags() 获取工具标签
print("\n3️⃣ 使用 tool_tags() 获取工具标签")
tags = tool_proxy.tool_tags()
print(f"✅ 工具标签获取成功")
print(f"   返回类型: {type(tags)}")
print(f"   标签: {tags}")

# 4️⃣ 检查标签内容
print("\n4️⃣ 检查标签内容")
if tags:
    print(f"📋 工具标签:")
    if isinstance(tags, list):
        for tag in tags:
            print(f"   - {tag}")
    elif isinstance(tags, dict):
        for key, value in tags.items():
            print(f"   - {key}: {value}")
    else:
        print(f"   标签内容: {tags}")
else:
    print(f"   （无标签）")

# 5️⃣ 获取多个工具的标签
print("\n5️⃣ 获取多个工具的标签")
tools = store.for_store().list_tools()
if tools:
    print(f"📋 工具标签概览:")
    for tool in tools[:5]:
        proxy = store.for_store().find_tool(tool.name)
        tool_tags = proxy.tool_tags()
        print(f"   {tool.name}: {tool_tags if tool_tags else '（无标签）'}")
    
    if len(tools) > 5:
        print(f"   ... 还有 {len(tools) - 5} 个工具")

# 6️⃣ 使用标签进行工具分类
print("\n6️⃣ 使用标签进行工具分类")
tag_groups = {}
for tool in tools:
    proxy = store.for_store().find_tool(tool.name)
    tool_tags = proxy.tool_tags()
    
    if tool_tags:
        if isinstance(tool_tags, list):
            for tag in tool_tags:
                if tag not in tag_groups:
                    tag_groups[tag] = []
                tag_groups[tag].append(tool.name)
        elif isinstance(tool_tags, str):
            if tool_tags not in tag_groups:
                tag_groups[tool_tags] = []
            tag_groups[tool_tags].append(tool.name)

if tag_groups:
    print(f"📊 按标签分类:")
    for tag, tool_names in tag_groups.items():
        print(f"   标签 '{tag}': {len(tool_names)} 个工具")
        for name in tool_names[:3]:
            print(f"      - {name}")
        if len(tool_names) > 3:
            print(f"      ... 还有 {len(tool_names) - 3} 个")
else:
    print(f"   （暂无标签分类）")

# 7️⃣ 标签的用途
print("\n7️⃣ 标签的实际应用")
print(f"   标签可用于:")
print(f"   - 工具分类和组织")
print(f"   - 工具搜索和过滤")
print(f"   - 权限控制")
print(f"   - UI 展示分组")
print(f"   - 工具推荐")

print("\n💡 tool_tags() 特点:")
print("   - 返回工具的标签")
print("   - 可能是列表、字符串或字典")
print("   - 用于工具分类和组织")
print("   - 支持工具搜索和过滤")
print("   - 适合元数据管理")

print("\n💡 使用场景:")
print("   - 工具分类")
print("   - 标签搜索")
print("   - 权限控制")
print("   - UI 分组展示")
print("   - 工具推荐系统")

print("\n" + "=" * 60)
print("✅ Store 获取工具标签测试完成")
print("=" * 60)

