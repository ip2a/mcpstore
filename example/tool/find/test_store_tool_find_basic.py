"""
测试：Store 查找工具（基础）
功能：测试使用 find_tool() 查找工具并获取 ToolProxy
上下文：Store 级别
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore

print("=" * 60)
print("测试：Store 查找工具（基础）")
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

# 2️⃣ 列出所有可用工具
print("\n2️⃣ 列出所有可用工具")
tools = store.for_store().list_tools()
print(f"✅ 可用工具数量: {len(tools)}")
if tools:
    print(f"   工具列表:")
    for tool in tools[:5]:
        print(f"   - {tool.name}")
    if len(tools) > 5:
        print(f"   ... 还有 {len(tools) - 5} 个工具")

# 3️⃣ 使用 find_tool() 查找特定工具
print("\n3️⃣ 使用 find_tool() 查找特定工具")
if tools:
    tool_name = "get_current_weather"
    tool_proxy = store.for_store().find_tool(tool_name)
    print(f"✅ 找到工具: {tool_name}")
    print(f"   ToolProxy: {tool_proxy}")
    print(f"   类型: {type(tool_proxy)}")

# 4️⃣ 验证 ToolProxy 的方法
print("\n4️⃣ 验证 ToolProxy 的可用方法")
if tools:
    methods = [m for m in dir(tool_proxy) if not m.startswith('_')]
    print(f"✅ ToolProxy 可用方法数量: {len(methods)}")
    print(f"   主要方法:")
    important_methods = [
        'tool_info', 'tool_tags', 'tool_schema',
        'call_tool', 'set_redirect', 'usage_stats', 'call_history'
    ]
    for method in important_methods:
        if method in methods:
            print(f"   - {method}()")

# 5️⃣ 使用 ToolProxy 获取工具信息
print("\n5️⃣ 使用 ToolProxy 获取工具信息")
if tools:
    info = tool_proxy.tool_info()
    print(f"✅ 工具信息:")
    if isinstance(info, dict):
        print(f"   名称: {info.get('name', 'N/A')}")
        print(f"   描述: {info.get('description', 'N/A')}")

# 6️⃣ 使用 ToolProxy 调用工具
print("\n6️⃣ 使用 ToolProxy 调用工具")
if tools:
    result = tool_proxy.call_tool({"query": "北京"})
    print(f"✅ 工具调用成功")
    print(f"   结果: {result.text_output if hasattr(result, 'text_output') else result}")

# 7️⃣ 查找多个工具
print("\n7️⃣ 查找多个工具")
if len(tools) >= 2:
    for tool in tools[:2]:
        found_tool = store.for_store().find_tool(tool.name)
        print(f"   ✅ 找到工具: {tool.name}")

# 8️⃣ 尝试查找不存在的工具
print("\n8️⃣ 尝试查找不存在的工具")
try:
    nonexistent_tool = store.for_store().find_tool("nonexistent_tool")
    print(f"⚠️ 意外：找到了不存在的工具")
except Exception as e:
    print(f"✅ 预期结果：工具不存在")
    print(f"   异常: {type(e).__name__}")

print("\n💡 find_tool() 特点:")
print("   - 查找特定名称的工具")
print("   - 返回 ToolProxy 对象")
print("   - ToolProxy 提供工具级别的操作方法")
print("   - 支持工具信息查询、调用、配置")
print("   - 工具不存在时抛出异常")

print("\n💡 ToolProxy 特点:")
print("   - 工具操作的代理对象")
print("   - 提供完整的工具管理方法")
print("   - 支持工具调用")
print("   - 支持配置（如 set_redirect）")
print("   - 支持统计查询")

print("\n💡 使用场景:")
print("   - 查找特定工具")
print("   - 获取工具详情")
print("   - 调用工具")
print("   - 配置工具行为")
print("   - 查看工具统计")

print("\n" + "=" * 60)
print("✅ Store 查找工具测试完成")
print("=" * 60)

