"""
测试：Store 使用工具（别名）
功能：测试使用 use_tool() 调用工具（call_tool 的别名）
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
print("测试：Store 使用工具（别名）")
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

# 4️⃣ 准备调用参数
print("\n4️⃣ 准备调用参数")
params = {
    "query": "北京"
}
print(f"   调用参数: {json.dumps(params, ensure_ascii=False)}")

# 5️⃣ 使用 use_tool() 调用工具
print("\n5️⃣ 使用 use_tool() 调用工具")
result = tool_proxy.use_tool(params)
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

# 7️⃣ 对比 call_tool() 和 use_tool()
print("\n7️⃣ 对比 call_tool() 和 use_tool()")
print(f"   使用相同参数测试两个方法:")

# 使用 call_tool()
call_result = tool_proxy.call_tool(params)
print(f"   call_tool() 结果类型: {type(call_result)}")

# 使用 use_tool()
use_result = tool_proxy.use_tool(params)
print(f"   use_tool() 结果类型: {type(use_result)}")

# 比较结果
if call_result == use_result:
    print(f"   ✅ 两个方法返回相同结果")
else:
    print(f"   ⚠️ 两个方法返回不同结果")

# 8️⃣ 测试多个工具的使用
print("\n8️⃣ 测试多个工具的使用")
tools = store.for_store().list_tools()
if len(tools) >= 2:
    for tool in tools[:2]:
        proxy = store.for_store().find_tool(tool.name)
        schema = proxy.tool_schema()
        
        print(f"\n   工具: {tool.name}")
        if isinstance(schema, dict) and 'properties' in schema:
            # 生成简单参数
            simple_params = {}
            for prop_name, prop_schema in schema['properties'].items():
                prop_type = prop_schema.get('type', 'string')
                if prop_type == 'string':
                    simple_params[prop_name] = f"test_{prop_name}"
                elif prop_type == 'number' or prop_type == 'integer':
                    simple_params[prop_name] = 1
                elif prop_type == 'boolean':
                    simple_params[prop_name] = True
            
            print(f"   参数: {json.dumps(simple_params, ensure_ascii=False)}")
            try:
                result = proxy.use_tool(simple_params)
                print(f"   ✅ 调用成功")
                if isinstance(result, dict):
                    print(f"   结果字段: {list(result.keys())}")
            except Exception as e:
                print(f"   ❌ 调用失败: {e}")

# 9️⃣ 性能对比测试
print("\n9️⃣ 性能对比测试")
import time

# 测试 call_tool() 性能
start_time = time.time()
for _ in range(3):
    tool_proxy.call_tool(params)
call_time = time.time() - start_time

# 测试 use_tool() 性能
start_time = time.time()
for _ in range(3):
    tool_proxy.use_tool(params)
use_time = time.time() - start_time

print(f"   call_tool() 3次调用耗时: {call_time:.4f}秒")
print(f"   use_tool() 3次调用耗时: {use_time:.4f}秒")
print(f"   性能差异: {abs(call_time - use_time):.4f}秒")

print("\n💡 use_tool() 特点:")
print("   - call_tool() 的别名")
print("   - 功能完全相同")
print("   - 提供更语义化的方法名")
print("   - 性能无差异")
print("   - 适合不同编程风格")

print("\n💡 使用场景:")
print("   - 语义化调用")
print("   - 代码可读性")
print("   - 团队编码规范")
print("   - 方法名偏好")
print("   - API 设计一致性")

print("\n💡 选择建议:")
print("   - call_tool(): 强调'调用'动作")
print("   - use_tool(): 强调'使用'工具")
print("   - 团队统一使用一种")
print("   - 根据上下文选择")

print("\n" + "=" * 60)
print("✅ Store 使用工具（别名）测试完成")
print("=" * 60)

