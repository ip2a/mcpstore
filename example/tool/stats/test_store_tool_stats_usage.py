"""
测试：Store 获取工具使用统计
功能：测试使用 usage_stats() 获取工具使用统计信息
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
print("测试：Store 获取工具使用统计")
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

# 3️⃣ 获取初始使用统计
print("\n3️⃣ 获取初始使用统计")
initial_stats = tool_proxy.usage_stats()
print(f"✅ 工具使用统计获取成功")
print(f"   返回类型: {type(initial_stats)}")
print(f"   初始统计: {initial_stats}")

# 4️⃣ 多次调用工具以生成统计数据
print("\n4️⃣ 多次调用工具以生成统计数据")
params_list = [
    {"query": "北京"},
    {"query": "上海"},
    {"query": "广州"},
    {"query": "深圳"},
    {"query": "杭州"}
]

for i, params in enumerate(params_list, 1):
    print(f"   调用 {i}: {json.dumps(params, ensure_ascii=False)}")
    try:
        result = tool_proxy.call_tool(params)
        print(f"   ✅ 调用成功")
    except Exception as e:
        print(f"   ❌ 调用失败: {e}")

# 5️⃣ 获取更新后的使用统计
print("\n5️⃣ 获取更新后的使用统计")
updated_stats = tool_proxy.usage_stats()
print(f"✅ 更新后的使用统计:")
print(f"   统计信息: {updated_stats}")

# 6️⃣ 展示统计信息的主要字段
print("\n6️⃣ 展示统计信息的主要字段")
if isinstance(updated_stats, dict):
    print(f"📋 使用统计详情:")
    for key, value in updated_stats.items():
        print(f"   {key}: {value}")
else:
    print(f"   统计内容: {updated_stats}")

# 7️⃣ 展示完整的统计信息（JSON 格式）
print("\n7️⃣ 完整的统计信息（JSON 格式）:")
print("-" * 60)
print(json.dumps(updated_stats, indent=2, ensure_ascii=False, default=str))
print("-" * 60)

# 8️⃣ 对比初始和更新后的统计
print("\n8️⃣ 对比初始和更新后的统计")
print(f"   初始统计: {initial_stats}")
print(f"   更新统计: {updated_stats}")

if initial_stats != updated_stats:
    print(f"   ✅ 统计信息已更新")
else:
    print(f"   ⚠️ 统计信息未变化")

# 9️⃣ 获取多个工具的统计对比
print("\n9️⃣ 获取多个工具的统计对比")
tools = store.for_store().list_tools()
if len(tools) >= 2:
    print(f"📊 工具统计对比:")
    for tool in tools[:3]:
        proxy = store.for_store().find_tool(tool.name)
        stats = proxy.usage_stats()
        print(f"   工具 {tool.name}: {stats}")

# 🔟 统计信息的用途
print("\n🔟 统计信息的用途")
print(f"   使用统计用于:")
print(f"   - 监控工具使用频率")
print(f"   - 分析工具性能")
print(f"   - 优化工具配置")
print(f"   - 生成使用报告")
print(f"   - 资源分配决策")

print("\n💡 usage_stats() 特点:")
print("   - 返回工具使用统计")
print("   - 包含调用次数等信息")
print("   - 支持性能分析")
print("   - 用于监控和优化")
print("   - 实时更新")

print("\n💡 使用场景:")
print("   - 工具使用监控")
print("   - 性能分析")
print("   - 使用报告生成")
print("   - 资源优化")
print("   - 决策支持")

print("\n" + "=" * 60)
print("✅ Store 获取工具使用统计测试完成")
print("=" * 60)

