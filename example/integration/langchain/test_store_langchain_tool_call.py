"""
测试：LangChain 集成 - 工具调用
功能：测试 LangChain 工具的实际调用和使用
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
print("测试：LangChain 集成 - 工具调用")
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

# 2️⃣ 获取 LangChain 集成对象
print("\n2️⃣ 获取 LangChain 集成对象")
langchain_integration = store.for_langchain()
print(f"✅ LangChain 集成对象获取成功")

# 3️⃣ 获取 LangChain 工具列表
print("\n3️⃣ 获取 LangChain 工具列表")
langchain_tools = langchain_integration.list_tools()
print(f"✅ LangChain 工具列表获取成功")
print(f"   工具数量: {len(langchain_tools) if isinstance(langchain_tools, list) else 'N/A'}")

# 4️⃣ 选择测试工具
print("\n4️⃣ 选择测试工具")
if isinstance(langchain_tools, list) and langchain_tools:
    test_tool = langchain_tools[0]
    tool_name = getattr(test_tool, 'name', 'N/A')
    tool_desc = getattr(test_tool, 'description', 'N/A')
    print(f"   选择工具: {tool_name}")
    print(f"   工具描述: {tool_desc}")
else:
    print(f"   ❌ 无可用工具")
    exit()

# 5️⃣ 测试工具调用
print("\n5️⃣ 测试工具调用")
test_params = ["北京", "上海", "广州"]

for i, param in enumerate(test_params, 1):
    print(f"   调用 {i}: 参数='{param}'")
    try:
        result = test_tool.func(param)
        print(f"   ✅ 调用成功")
        print(f"   返回类型: {type(result)}")
        
        # 展示结果
        if isinstance(result, str):
            result_short = result[:100] + "..." if len(result) > 100 else result
            print(f"   返回结果: {result_short}")
        else:
            print(f"   返回结果: {result}")
        
        print()
    except Exception as e:
        print(f"   ❌ 调用失败: {e}")
        print()

# 6️⃣ 测试工具链调用
print("\n6️⃣ 测试工具链调用")
print(f"   模拟工具链调用:")

try:
    # 模拟工具链：天气查询 -> 结果处理
    weather_result = test_tool.func("北京")
    print(f"   1. 天气查询结果: {weather_result}")
    
    # 模拟结果处理
    if isinstance(weather_result, str):
        processed_result = f"处理后的结果: {weather_result[:50]}..."
        print(f"   2. 处理结果: {processed_result}")
    
    print(f"   ✅ 工具链调用成功")
except Exception as e:
    print(f"   ❌ 工具链调用失败: {e}")

# 7️⃣ 测试多个工具调用
print("\n7️⃣ 测试多个工具调用")
if len(langchain_tools) >= 2:
    print(f"   测试多个工具:")
    for i, tool in enumerate(langchain_tools[:2], 1):
        tool_name = getattr(tool, 'name', f'Tool_{i}')
        print(f"   工具 {i}: {tool_name}")
        
        try:
            result = tool.func("测试参数")
            print(f"   ✅ 调用成功")
            print(f"   结果类型: {type(result)}")
        except Exception as e:
            print(f"   ❌ 调用失败: {e}")
        print()

# 8️⃣ 测试工具参数验证
print("\n8️⃣ 测试工具参数验证")
print(f"   测试不同参数类型:")

test_cases = [
    ("字符串参数", "北京"),
    ("数字参数", 123),
    ("布尔参数", True),
    ("None参数", None),
    ("空字符串", ""),
]

for case_name, param in test_cases:
    print(f"   测试 {case_name}: {param}")
    try:
        result = test_tool.func(param)
        print(f"   ✅ 调用成功")
        print(f"   结果: {result}")
    except Exception as e:
        print(f"   ❌ 调用失败: {e}")
    print()

# 9️⃣ 性能测试
print("\n9️⃣ 性能测试")
import time

print(f"   测试工具调用性能:")
call_times = []
for i in range(5):
    start_time = time.time()
    try:
        result = test_tool.func("性能测试")
        end_time = time.time()
        call_time = end_time - start_time
        call_times.append(call_time)
        print(f"   调用 {i+1}: {call_time:.4f}秒")
    except Exception as e:
        print(f"   调用 {i+1}: 失败 - {e}")

if call_times:
    avg_time = sum(call_times) / len(call_times)
    print(f"   平均调用时间: {avg_time:.4f}秒")

# 🔟 LangChain 工具特性
print("\n🔟 LangChain 工具特性")
print(f"   LangChain 工具特性:")
print(f"   - 支持标准 LangChain 工具接口")
print(f"   - 支持工具链调用")
print(f"   - 支持参数验证")
print(f"   - 支持错误处理")
print(f"   - 支持性能监控")

print("\n💡 LangChain 工具调用特点:")
print("   - 标准 LangChain 工具接口")
print("   - 支持工具链调用")
print("   - 自动参数处理")
print("   - 统一错误处理")
print("   - 性能优化")

print("\n💡 使用场景:")
print("   - LangChain 工具链")
print("   - 工具链构建")
print("   - 自动化流程")
print("   - 工具组合")
print("   - 工作流自动化")

print("\n" + "=" * 60)
print("✅ LangChain 集成 - 工具调用测试完成")
print("=" * 60)

