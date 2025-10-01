"""
测试：Store 获取服务工具统计
功能：测试使用 tools_stats() 获取服务中所有工具的统计信息
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
print("测试：Store 获取服务工具统计")
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

# 2️⃣ 获取服务代理
print("\n2️⃣ 获取服务代理")
service_proxy = store.for_store().find_service("weather")
print(f"✅ 找到服务: weather")

# 3️⃣ 获取初始工具统计
print("\n3️⃣ 获取初始工具统计")
initial_stats = service_proxy.tools_stats()
print(f"✅ 服务工具统计获取成功")
print(f"   返回类型: {type(initial_stats)}")
print(f"   初始统计: {initial_stats}")

# 4️⃣ 多次调用不同工具以生成统计数据
print("\n4️⃣ 多次调用不同工具以生成统计数据")
tools = store.for_store().list_tools()
if tools:
    print(f"   可用工具: {[tool.name for tool in tools]}")
    
    # 调用每个工具几次
    for tool in tools[:3]:  # 限制前3个工具
        proxy = store.for_store().find_tool(tool.name)
        schema = proxy.tool_schema()
        
        # 生成简单参数
        if isinstance(schema, dict) and 'properties' in schema:
            simple_params = {}
            for prop_name, prop_schema in schema['properties'].items():
                prop_type = prop_schema.get('type', 'string')
                if prop_type == 'string':
                    simple_params[prop_name] = f"test_{prop_name}"
                elif prop_type == 'number' or prop_type == 'integer':
                    simple_params[prop_name] = 1
                elif prop_type == 'boolean':
                    simple_params[prop_name] = True
            
            print(f"   调用工具 {tool.name}: {json.dumps(simple_params, ensure_ascii=False)}")
            try:
                result = proxy.call_tool(simple_params)
                print(f"   ✅ 调用成功")
            except Exception as e:
                print(f"   ❌ 调用失败: {e}")

# 5️⃣ 获取更新后的工具统计
print("\n5️⃣ 获取更新后的工具统计")
updated_stats = service_proxy.tools_stats()
print(f"✅ 更新后的工具统计:")
print(f"   统计信息: {updated_stats}")

# 6️⃣ 展示统计信息的主要字段
print("\n6️⃣ 展示统计信息的主要字段")
if isinstance(updated_stats, dict):
    print(f"📋 服务工具统计详情:")
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

# 9️⃣ 分析统计信息结构
print("\n9️⃣ 分析统计信息结构")
if isinstance(updated_stats, dict):
    print(f"📊 统计信息分析:")
    
    # 分析统计字段
    print(f"   统计字段: {list(updated_stats.keys())}")
    
    # 分析工具数量
    if 'total_tools' in updated_stats:
        print(f"   总工具数: {updated_stats['total_tools']}")
    
    # 分析调用统计
    if 'total_calls' in updated_stats:
        print(f"   总调用数: {updated_stats['total_calls']}")
    
    # 分析工具详情
    if 'tools' in updated_stats:
        tools_detail = updated_stats['tools']
        if isinstance(tools_detail, dict):
            print(f"   工具详情数量: {len(tools_detail)}")
            for tool_name, tool_stats in tools_detail.items():
                print(f"     工具 {tool_name}: {tool_stats}")

# 🔟 对比单个工具统计和服务统计
print("\n🔟 对比单个工具统计和服务统计")
if tools:
    tool_name = tools[0].name
    tool_proxy = store.for_store().find_tool(tool_name)
    tool_stats = tool_proxy.usage_stats()
    
    print(f"   单个工具 {tool_name} 统计: {tool_stats}")
    print(f"   服务整体统计: {updated_stats}")
    
    # 分析关系
    if isinstance(tool_stats, dict) and isinstance(updated_stats, dict):
        print(f"   统计关系分析:")
        for key in tool_stats.keys():
            if key in updated_stats:
                print(f"     {key}: 工具={tool_stats[key]}, 服务={updated_stats[key]}")

# 1️⃣1️⃣ 工具统计的用途
print("\n1️⃣1️⃣ 工具统计的用途")
print(f"   服务工具统计用于:")
print(f"   - 监控服务整体使用情况")
print(f"   - 分析工具使用分布")
print(f"   - 优化服务配置")
print(f"   - 生成服务报告")
print(f"   - 资源分配决策")

print("\n💡 tools_stats() 特点:")
print("   - 返回服务中所有工具的统计")
print("   - 包含整体和详细统计")
print("   - 支持服务级监控")
print("   - 用于性能分析")
print("   - 实时更新")

print("\n💡 使用场景:")
print("   - 服务监控")
print("   - 工具使用分析")
print("   - 性能优化")
print("   - 服务报告")
print("   - 资源管理")

print("\n" + "=" * 60)
print("✅ Store 获取服务工具统计测试完成")
print("=" * 60)

