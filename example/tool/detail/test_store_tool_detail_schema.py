"""
测试：Store 获取工具输入模式
功能：测试使用 tool_schema() 获取工具的输入参数模式
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
print("测试：Store 获取工具输入模式")
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

# 3️⃣ 使用 tool_schema() 获取工具输入模式
print("\n3️⃣ 使用 tool_schema() 获取工具输入模式")
schema = tool_proxy.tool_schema()
print(f"✅ 工具输入模式获取成功")
print(f"   返回类型: {type(schema)}")

# 4️⃣ 展示输入模式的主要结构
print("\n4️⃣ 展示输入模式的主要结构")
if isinstance(schema, dict):
    print(f"📋 输入模式结构:")
    if 'type' in schema:
        print(f"   类型: {schema['type']}")
    if 'properties' in schema:
        print(f"   属性数量: {len(schema['properties'])}")
        print(f"   属性列表:")
        for prop_name, prop_schema in schema['properties'].items():
            prop_type = prop_schema.get('type', 'N/A')
            prop_desc = prop_schema.get('description', 'N/A')
            desc_short = prop_desc[:40] + "..." if len(prop_desc) > 40 else prop_desc
            print(f"      - {prop_name} ({prop_type}): {desc_short}")
    if 'required' in schema:
        print(f"   必填字段: {schema['required']}")

# 5️⃣ 展示完整的输入模式（JSON 格式）
print("\n5️⃣ 完整的输入模式（JSON 格式）:")
print("-" * 60)
print(json.dumps(schema, indent=2, ensure_ascii=False, default=str))
print("-" * 60)

# 6️⃣ 解析模式以生成调用示例
print("\n6️⃣ 根据模式生成调用示例")
if isinstance(schema, dict) and 'properties' in schema:
    example_params = {}
    for prop_name, prop_schema in schema['properties'].items():
        prop_type = prop_schema.get('type', 'string')
        if prop_type == 'string':
            example_params[prop_name] = f"<{prop_name}>"
        elif prop_type == 'number' or prop_type == 'integer':
            example_params[prop_name] = 0
        elif prop_type == 'boolean':
            example_params[prop_name] = False
        elif prop_type == 'array':
            example_params[prop_name] = []
        elif prop_type == 'object':
            example_params[prop_name] = {}
    
    print(f"📝 调用示例:")
    print(f"   tool_proxy.call_tool({json.dumps(example_params, ensure_ascii=False)})")

# 7️⃣ 获取多个工具的模式
print("\n7️⃣ 获取多个工具的模式对比")
tools = store.for_store().list_tools()
if len(tools) >= 2:
    for tool in tools[:2]:
        proxy = store.for_store().find_tool(tool.name)
        tool_schema = proxy.tool_schema()
        
        print(f"\n   工具: {tool.name}")
        if isinstance(tool_schema, dict):
            if 'properties' in tool_schema:
                print(f"   参数数量: {len(tool_schema['properties'])}")
                print(f"   参数名称: {list(tool_schema['properties'].keys())}")
            if 'required' in tool_schema:
                print(f"   必填参数: {tool_schema['required']}")

# 8️⃣ 模式的用途
print("\n8️⃣ 输入模式的实际应用")
print(f"   输入模式用于:")
print(f"   - 参数验证")
print(f"   - 生成调用代码")
print(f"   - UI 表单生成")
print(f"   - 文档生成")
print(f"   - 类型检查")

print("\n💡 tool_schema() 特点:")
print("   - 返回工具的输入参数模式")
print("   - 通常是 JSON Schema 格式")
print("   - 包含参数类型、描述、必填信息")
print("   - 用于参数验证和文档生成")
print("   - 支持复杂的嵌套结构")

print("\n💡 使用场景:")
print("   - 参数验证")
print("   - 动态 UI 生成")
print("   - 代码生成")
print("   - 文档自动生成")
print("   - 类型安全调用")

print("\n" + "=" * 60)
print("✅ Store 获取工具输入模式测试完成")
print("=" * 60)

