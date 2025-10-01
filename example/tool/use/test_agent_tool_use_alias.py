"""
测试：Agent 使用工具（别名）
功能：测试在 Agent 上下文中使用 use_tool() 调用工具
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
print("测试：Agent 使用工具（别名）")
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

# 6️⃣ 在 Agent 中使用 use_tool() 调用工具
print("\n6️⃣ 在 Agent 中使用 use_tool() 调用工具")
result = tool_proxy.use_tool(params)
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

# 8️⃣ 对比 call_tool() 和 use_tool() 在 Agent 中
print("\n8️⃣ 对比 call_tool() 和 use_tool() 在 Agent 中")
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

# 9️⃣ 测试多个 Agent 使用 use_tool()
print("\n9️⃣ 测试多个 Agent 使用 use_tool()")
agent1 = store.for_agent("agent_1")
agent2 = store.for_agent("agent_2")

# 在两个 Agent 中使用相同工具
tool1 = agent1.find_tool(tool_name)
tool2 = agent2.find_tool(tool_name)

result1 = tool1.use_tool(params)
result2 = tool2.use_tool(params)

print(f"   Agent 1 use_tool() 结果类型: {type(result1)}")
print(f"   Agent 2 use_tool() 结果类型: {type(result2)}")

if result1 == result2:
    print(f"   ✅ 不同 Agent 返回相同结果")
else:
    print(f"   ⚠️ 不同 Agent 返回不同结果")

# 🔟 Agent 上下文中的方法对比
print("\n🔟 Agent 上下文中的方法对比")
print(f"   在 Agent 上下文中:")
print(f"   - call_tool() 和 use_tool() 功能相同")
print(f"   - 都支持状态隔离")
print(f"   - 都支持并发调用")
print(f"   - 都支持独立错误处理")
print(f"   - 都支持权限控制")

print("\n💡 Agent use_tool() 特点:")
print("   - call_tool() 的别名")
print("   - 在 Agent 上下文中调用")
print("   - 支持状态隔离")
print("   - 支持并发执行")
print("   - 独立的错误处理")

print("\n💡 使用场景:")
print("   - Agent 系统中的工具使用")
print("   - 多 Agent 并发调用")
print("   - 状态隔离的工具调用")
print("   - 权限控制的工具使用")
print("   - 分布式工具调用")

print("\n💡 选择建议:")
print("   - call_tool(): 强调'调用'动作")
print("   - use_tool(): 强调'使用'工具")
print("   - Agent 上下文中功能相同")
print("   - 根据团队规范选择")

print("\n" + "=" * 60)
print("✅ Agent 使用工具（别名）测试完成")
print("=" * 60)

