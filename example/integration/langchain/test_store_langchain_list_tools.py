"""
测试：LangChain 集成 - 列出工具
功能：测试使用 for_langchain().list_tools() 获取 LangChain 兼容的工具列表
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
print("测试：LangChain 集成 - 列出工具")
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
print(f"   集成对象类型: {type(langchain_integration)}")

# 3️⃣ 使用 list_tools() 获取 LangChain 兼容的工具列表
print("\n3️⃣ 使用 list_tools() 获取 LangChain 兼容的工具列表")
langchain_tools = langchain_integration.list_tools()
print(f"✅ LangChain 工具列表获取成功")
print(f"   返回类型: {type(langchain_tools)}")
print(f"   工具数量: {len(langchain_tools) if isinstance(langchain_tools, list) else 'N/A'}")

# 4️⃣ 展示 LangChain 工具列表
print("\n4️⃣ 展示 LangChain 工具列表")
if isinstance(langchain_tools, list):
    print(f"📋 LangChain 工具列表:")
    for i, tool in enumerate(langchain_tools, 1):
        print(f"   工具 {i}: {tool}")
        if hasattr(tool, 'name'):
            print(f"     名称: {tool.name}")
        if hasattr(tool, 'description'):
            desc = tool.description
            desc_short = desc[:80] + "..." if len(desc) > 80 else desc
            print(f"     描述: {desc_short}")
        if hasattr(tool, 'func'):
            print(f"     函数: {tool.func}")
        print()
else:
    print(f"   工具列表: {langchain_tools}")

# 5️⃣ 展示完整的工具列表（JSON 格式）
print("\n5️⃣ 完整的工具列表（JSON 格式）:")
print("-" * 60)
try:
    # 尝试序列化工具对象
    tools_data = []
    for tool in langchain_tools:
        tool_data = {
            'name': getattr(tool, 'name', 'N/A'),
            'description': getattr(tool, 'description', 'N/A'),
            'func': str(getattr(tool, 'func', 'N/A')),
            'type': type(tool).__name__
        }
        tools_data.append(tool_data)
    
    print(json.dumps(tools_data, indent=2, ensure_ascii=False, default=str))
except Exception as e:
    print(f"   序列化失败: {e}")
    print(f"   原始数据: {langchain_tools}")
print("-" * 60)

# 6️⃣ 对比原生工具和 LangChain 工具
print("\n6️⃣ 对比原生工具和 LangChain 工具")
native_tools = store.for_store().list_tools()
print(f"   原生工具数量: {len(native_tools)}")
print(f"   LangChain 工具数量: {len(langchain_tools) if isinstance(langchain_tools, list) else 'N/A'}")

if len(native_tools) == len(langchain_tools):
    print(f"   ✅ 工具数量一致")
else:
    print(f"   ⚠️ 工具数量不一致")

# 7️⃣ 测试 LangChain 工具调用
print("\n7️⃣ 测试 LangChain 工具调用")
if isinstance(langchain_tools, list) and langchain_tools:
    test_tool = langchain_tools[0]
    print(f"   测试工具: {getattr(test_tool, 'name', 'N/A')}")
    
    try:
        # 尝试调用工具
        if hasattr(test_tool, 'func'):
            result = test_tool.func("北京")
            print(f"   ✅ 工具调用成功")
            print(f"   返回类型: {type(result)}")
            print(f"   返回结果: {result}")
        else:
            print(f"   ⚠️ 工具无 func 属性")
    except Exception as e:
        print(f"   ❌ 工具调用失败: {e}")

# 8️⃣ 分析 LangChain 工具特性
print("\n8️⃣ 分析 LangChain 工具特性")
if isinstance(langchain_tools, list) and langchain_tools:
    print(f"📊 LangChain 工具特性分析:")
    
    # 分析工具属性
    tool_attrs = set()
    for tool in langchain_tools:
        tool_attrs.update(dir(tool))
    
    print(f"   工具属性: {sorted(tool_attrs)}")
    
    # 分析工具类型
    tool_types = {}
    for tool in langchain_tools:
        tool_type = type(tool).__name__
        tool_types[tool_type] = tool_types.get(tool_type, 0) + 1
    
    print(f"   工具类型分布: {tool_types}")

# 9️⃣ LangChain 集成的用途
print("\n9️⃣ LangChain 集成的用途")
print(f"   LangChain 集成用于:")
print(f"   - 将 MCPStore 工具转换为 LangChain 工具")
print(f"   - 支持 LangChain 工具链")
print(f"   - 提供统一的工具接口")
print(f"   - 支持 LangChain 生态系统")
print(f"   - 简化工具集成")

print("\n💡 for_langchain().list_tools() 特点:")
print("   - 返回 LangChain 兼容的工具列表")
print("   - 支持 LangChain 工具链")
print("   - 提供统一的工具接口")
print("   - 支持工具调用")
print("   - 自动转换工具格式")

print("\n💡 使用场景:")
print("   - LangChain 工具链集成")
print("   - 工具格式转换")
print("   - 统一工具接口")
print("   - 生态系统集成")
print("   - 工具链构建")

print("\n" + "=" * 60)
print("✅ LangChain 集成 - 列出工具测试完成")
print("=" * 60)

