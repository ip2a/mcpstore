"""
测试：Agent 调用工具
功能：测试在 Agent 上下文中使用 call_tool() 调用工具
上下文：Agent 级别
"""

import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from utils.import_helper import setup_import_path
setup_import_path()

from mcpstore import MCPStore
import json

print("=" * 60)
print("测试：Agent 调用工具")
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

# 2️⃣ 创建 Agent 上下文
print("\n2️⃣ 创建 Agent 上下文")
agent_context = store.for_agent("test_agent")
print(f"✅ Agent 上下文创建成功: test_agent")

# 3️⃣ 在 Agent 中查找工具
print("\n3️⃣ 在 Agent 中查找工具")
tool_name = "get_current_weather"
tool_proxy = agent_context.find_tool(tool_name)
print(f"✅ 在 Agent 中找到工具: {tool_name}")

# 4️⃣ 获取工具输入模式
print("\n4️⃣ 获取工具输入模式")
schema = tool_proxy.tool_schema()
print(f"✅ 工具输入模式获取成功")

# 5️⃣ 准备调用参数
print("\n5️⃣ 准备调用参数")
params = {
    "query": "北京"
}
print(f"   调用参数: {json.dumps(params, ensure_ascii=False)}")

# 6️⃣ 在 Agent 中使用 call_tool() 调用工具
print("\n6️⃣ 在 Agent 中使用 call_tool() 调用工具")
result = tool_proxy.call_tool(params)
print(f"✅ Agent 工具调用成功")
print(f"   返回类型: {type(result)}")

# 7️⃣ 展示调用结果
print("\n7️⃣ 展示调用结果")
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

# 8️⃣ 对比 Store 和 Agent 调用
print("\n8️⃣ 对比 Store 和 Agent 调用")
print(f"   使用相同参数测试不同上下文:")

# Store 上下文调用
store_tool = store.for_store().find_tool(tool_name)
store_result = store_tool.call_tool(params)
print(f"   Store 调用结果类型: {type(store_result)}")

# Agent 上下文调用
agent_result = tool_proxy.call_tool(params)
print(f"   Agent 调用结果类型: {type(agent_result)}")

# 比较结果
if store_result == agent_result:
    print(f"   ✅ Store 和 Agent 返回相同结果")
else:
    print(f"   ⚠️ Store 和 Agent 返回不同结果")

# 9️⃣ 测试多个 Agent 的隔离性
print("\n9️⃣ 测试多个 Agent 的隔离性")
agent1 = store.for_agent("agent_1")
agent2 = store.for_agent("agent_2")

# 在两个 Agent 中调用相同工具
tool1 = agent1.find_tool(tool_name)
tool2 = agent2.find_tool(tool_name)

result1 = tool1.call_tool(params)
result2 = tool2.call_tool(params)

print(f"   Agent 1 调用结果类型: {type(result1)}")
print(f"   Agent 2 调用结果类型: {type(result2)}")

if result1 == result2:
    print(f"   ✅ 不同 Agent 返回相同结果")
else:
    print(f"   ⚠️ 不同 Agent 返回不同结果")

# 🔟 Agent 上下文特性
print("\n🔟 Agent 上下文特性")
print(f"   Agent 上下文特点:")
print(f"   - 独立的工具调用环境")
print(f"   - 隔离的状态管理")
print(f"   - 支持并发调用")
print(f"   - 独立的错误处理")
print(f"   - 可配置的权限控制")

print("\n💡 Agent call_tool() 特点:")
print("   - 在 Agent 上下文中调用")
print("   - 支持状态隔离")
print("   - 支持并发执行")
print("   - 独立的错误处理")
print("   - 可配置权限")

print("\n💡 使用场景:")
print("   - 多 Agent 系统")
print("   - 并发工具调用")
print("   - 状态隔离")
print("   - 权限控制")
print("   - 分布式处理")

print("\n" + "=" * 60)
print("✅ Agent 调用工具测试完成")
print("=" * 60)

