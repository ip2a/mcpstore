"""
测试：Agent 设置工具重定向
功能：测试在 Agent 上下文中使用 set_redirect() 设置工具重定向行为
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
print("测试：Agent 设置工具重定向")
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

# 4️⃣ 检查初始重定向状态
print("\n4️⃣ 检查初始重定向状态")
initial_redirect = tool_proxy.set_redirect()
print(f"✅ 获取初始重定向状态: {initial_redirect}")

# 5️⃣ 设置重定向为 True
print("\n5️⃣ 设置重定向为 True")
tool_proxy.set_redirect(True)
redirect_status = tool_proxy.set_redirect()
print(f"✅ 重定向已设置为: {redirect_status}")

# 6️⃣ 测试重定向行为
print("\n6️⃣ 测试重定向行为")
params = {"query": "北京"}
print(f"   调用参数: {json.dumps(params, ensure_ascii=False)}")

# 调用工具并观察行为
result = tool_proxy.call_tool(params)
print(f"✅ 工具调用完成")
print(f"   返回类型: {type(result)}")

# 7️⃣ 设置重定向为 False
print("\n7️⃣ 设置重定向为 False")
tool_proxy.set_redirect(False)
redirect_status = tool_proxy.set_redirect()
print(f"✅ 重定向已设置为: {redirect_status}")

# 8️⃣ 测试非重定向行为
print("\n8️⃣ 测试非重定向行为")
result2 = tool_proxy.call_tool(params)
print(f"✅ 工具调用完成")
print(f"   返回类型: {type(result2)}")

# 9️⃣ 对比 Store 和 Agent 重定向设置
print("\n9️⃣ 对比 Store 和 Agent 重定向设置")
print(f"   测试不同上下文中的重定向设置:")

# Store 上下文
store_tool = store.for_store().find_tool(tool_name)
store_tool.set_redirect(True)
store_redirect = store_tool.set_redirect()
print(f"   Store 重定向状态: {store_redirect}")

# Agent 上下文
agent_redirect = tool_proxy.set_redirect()
print(f"   Agent 重定向状态: {agent_redirect}")

# 比较状态
if store_redirect == agent_redirect:
    print(f"   ✅ Store 和 Agent 重定向状态相同")
else:
    print(f"   ⚠️ Store 和 Agent 重定向状态不同")

# 🔟 测试多个 Agent 的重定向隔离
print("\n🔟 测试多个 Agent 的重定向隔离")
agent1 = store.for_agent("agent_1")
agent2 = store.for_agent("agent_2")

# 在两个 Agent 中设置不同重定向状态
tool1 = agent1.find_tool(tool_name)
tool2 = agent2.find_tool(tool_name)

tool1.set_redirect(True)
tool2.set_redirect(False)

redirect1 = tool1.set_redirect()
redirect2 = tool2.set_redirect()

print(f"   Agent 1 重定向状态: {redirect1}")
print(f"   Agent 2 重定向状态: {redirect2}")

if redirect1 != redirect2:
    print(f"   ✅ 不同 Agent 重定向状态独立")
else:
    print(f"   ⚠️ 不同 Agent 重定向状态相同")

# 1️⃣1️⃣ Agent 重定向特性
print("\n1️⃣1️⃣ Agent 重定向特性")
print(f"   Agent 重定向特点:")
print(f"   - 独立的重定向设置")
print(f"   - 不影响其他 Agent")
print(f"   - 支持并发配置")
print(f"   - 状态隔离")
print(f"   - 可配置权限控制")

print("\n💡 Agent set_redirect() 特点:")
print("   - 在 Agent 上下文中设置")
print("   - 支持状态隔离")
print("   - 支持并发配置")
print("   - 独立的错误处理")
print("   - 可配置权限")

print("\n💡 使用场景:")
print("   - 多 Agent 系统")
print("   - 并发重定向配置")
print("   - 状态隔离")
print("   - 权限控制")
print("   - 分布式工具配置")

print("\n" + "=" * 60)
print("✅ Agent 设置工具重定向测试完成")
print("=" * 60)

