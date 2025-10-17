"""
测试：Store 调用工具
功能：测试使用 call_tool() 调用工具
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
print("测试：Store 调用工具")
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

# 3️⃣ 获取工具输入模式
print("\n3️⃣ 获取工具输入模式")
schema = tool_proxy.tool_schema()
print(f"✅ 工具输入模式获取成功")
if isinstance(schema, dict) and 'properties' in schema:
    print(f"   参数: {list(schema['properties'].keys())}")
    if 'required' in schema:
        print(f"   必填: {schema['required']}")

# 4️⃣ 准备调用参数
print("\n4️⃣ 准备调用参数")
params = {
    "query": "北京"
}
print(f"   调用参数: {json.dumps(params, ensure_ascii=False)}")

# 5️⃣ 使用 call_tool() 调用工具
print("\n5️⃣ 使用 call_tool() 调用工具")
result = tool_proxy.call_tool(params)
print(f"✅ 工具调用成功")
print(f"   返回类型: {type(result)}")

# 6️⃣ 展示调用结果
print("\n6️⃣ 展示调用结果")
if isinstance(result, dict):
    print(f"📋 调用结果:")
    for key, value in result.items():
        if isinstance(value, str) and len(value) > 100:
            value_short = value[:100] + "..."
            print(f"   {key}: {value_short}")
        else:
            print(f"   {key}: {value}")
else:
    print(f"   结果: {result}")

# 7️⃣ 展示完整的调用结果（JSON 格式）
print("\n7️⃣ 完整的调用结果（JSON 格式）:")
print("-" * 60)
print(json.dumps(result, indent=2, ensure_ascii=False, default=str))
print("-" * 60)

# 8️⃣ 测试多个参数调用
print("\n8️⃣ 测试多个参数调用")
if isinstance(schema, dict) and 'properties' in schema:
    # 尝试不同的参数组合
    test_params = [
        {"query": "上海"},
        {"query": "广州"},
        {"query": "深圳"}
    ]
    
    for i, test_param in enumerate(test_params, 1):
        print(f"\n   测试 {i}: {json.dumps(test_param, ensure_ascii=False)}")
        try:
            test_result = tool_proxy.call_tool(test_param)
            print(f"   ✅ 调用成功")
            if isinstance(test_result, dict) and 'content' in test_result:
                content = test_result['content']
                content_short = content[:50] + "..." if len(content) > 50 else content
                print(f"   结果: {content_short}")
        except Exception as e:
            print(f"   ❌ 调用失败: {e}")

# 9️⃣ 错误处理测试
print("\n9️⃣ 错误处理测试")
print(f"   测试无效参数:")
try:
    invalid_params = {"invalid_param": "test"}
    invalid_result = tool_proxy.call_tool(invalid_params)
    print(f"   ⚠️ 意外成功: {invalid_result}")
except Exception as e:
    print(f"   ✅ 正确捕获错误: {e}")

print("\n💡 call_tool() 特点:")
print("   - 直接调用工具")
print("   - 支持参数传递")
print("   - 返回工具执行结果")
print("   - 自动处理错误")
print("   - 支持各种数据类型")

print("\n💡 使用场景:")
print("   - 直接工具调用")
print("   - 批量处理")
print("   - 自动化脚本")
print("   - API 接口")
print("   - 工具链调用")

print("\n" + "=" * 60)
print("✅ Store 调用工具测试完成")
print("=" * 60)

