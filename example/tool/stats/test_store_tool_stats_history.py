"""
测试：Store 获取工具调用历史
功能：测试使用 call_history() 获取工具调用历史记录
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
print("测试：Store 获取工具调用历史")
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

# 3️⃣ 获取初始调用历史
print("\n3️⃣ 获取初始调用历史")
initial_history = tool_proxy.call_history()
print(f"✅ 工具调用历史获取成功")
print(f"   返回类型: {type(initial_history)}")
print(f"   历史记录数量: {len(initial_history) if isinstance(initial_history, list) else 'N/A'}")

# 4️⃣ 多次调用工具以生成历史记录
print("\n4️⃣ 多次调用工具以生成历史记录")
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

# 5️⃣ 获取更新后的调用历史
print("\n5️⃣ 获取更新后的调用历史")
updated_history = tool_proxy.call_history()
print(f"✅ 更新后的调用历史:")
print(f"   历史记录数量: {len(updated_history) if isinstance(updated_history, list) else 'N/A'}")

# 6️⃣ 展示历史记录的主要信息
print("\n6️⃣ 展示历史记录的主要信息")
if isinstance(updated_history, list):
    print(f"📋 调用历史详情:")
    for i, record in enumerate(updated_history, 1):
        print(f"   记录 {i}:")
        if isinstance(record, dict):
            for key, value in record.items():
                if isinstance(value, str) and len(value) > 100:
                    value_short = value[:100] + "..."
                    print(f"      {key}: {value_short}")
                else:
                    print(f"      {key}: {value}")
        else:
            print(f"      {record}")
        print()
else:
    print(f"   历史内容: {updated_history}")

# 7️⃣ 展示完整的历史记录（JSON 格式）
print("\n7️⃣ 完整的调用历史（JSON 格式）:")
print("-" * 60)
print(json.dumps(updated_history, indent=2, ensure_ascii=False, default=str))
print("-" * 60)

# 8️⃣ 对比初始和更新后的历史
print("\n8️⃣ 对比初始和更新后的历史")
initial_count = len(initial_history) if isinstance(initial_history, list) else 0
updated_count = len(updated_history) if isinstance(updated_history, list) else 0

print(f"   初始历史记录数: {initial_count}")
print(f"   更新历史记录数: {updated_count}")
print(f"   新增记录数: {updated_count - initial_count}")

if updated_count > initial_count:
    print(f"   ✅ 历史记录已更新")
else:
    print(f"   ⚠️ 历史记录未变化")

# 9️⃣ 分析历史记录模式
print("\n9️⃣ 分析历史记录模式")
if isinstance(updated_history, list) and updated_history:
    print(f"📊 历史记录分析:")
    
    # 分析记录结构
    first_record = updated_history[0]
    if isinstance(first_record, dict):
        print(f"   记录字段: {list(first_record.keys())}")
    
    # 分析时间模式
    timestamps = []
    for record in updated_history:
        if isinstance(record, dict) and 'timestamp' in record:
            timestamps.append(record['timestamp'])
    
    if timestamps:
        print(f"   时间范围: {min(timestamps)} 到 {max(timestamps)}")
        print(f"   调用频率: {len(timestamps)} 次调用")

# 🔟 获取多个工具的历史对比
print("\n🔟 获取多个工具的历史对比")
tools = store.for_store().list_tools()
if len(tools) >= 2:
    print(f"📊 工具历史对比:")
    for tool in tools[:3]:
        proxy = store.for_store().find_tool(tool.name)
        history = proxy.call_history()
        history_count = len(history) if isinstance(history, list) else 0
        print(f"   工具 {tool.name}: {history_count} 条历史记录")

# 1️⃣1️⃣ 历史记录的用途
print("\n1️⃣1️⃣ 历史记录的用途")
print(f"   调用历史用于:")
print(f"   - 调试工具调用")
print(f"   - 分析调用模式")
print(f"   - 性能问题诊断")
print(f"   - 使用行为分析")
print(f"   - 审计和合规")

print("\n💡 call_history() 特点:")
print("   - 返回工具调用历史")
print("   - 包含调用参数和结果")
print("   - 支持调试和分析")
print("   - 用于问题诊断")
print("   - 实时更新")

print("\n💡 使用场景:")
print("   - 调试工具调用")
print("   - 性能分析")
print("   - 使用行为分析")
print("   - 问题诊断")
print("   - 审计记录")

print("\n" + "=" * 60)
print("✅ Store 获取工具调用历史测试完成")
print("=" * 60)

